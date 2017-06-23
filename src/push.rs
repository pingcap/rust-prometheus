// Copyright 2014 The Prometheus Authors
// Copyright 2016 PingCAP, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// See the License for the specific language governing permissions and
// limitations under the License.

use std::str;
use std::collections::HashMap;
use std::net::TcpStream;
use std::time;
use std::io::{Write, BufRead, BufWriter, BufReader};

use url::Url;

use proto;
use registry::Registry;
use metrics::Collector;
use errors::{Result, Error};
use encoder::{Encoder, TextEncoder};

/// `push_metrics` pushes all gathered metrics to the Pushgateway specified by
/// url, using the provided job name and the (optional) further grouping labels
/// (the grouping map may be nil). See the Pushgateway documentation for
/// detailed implications of the job and other grouping labels. Neither the job
/// name nor any grouping label value may contain a "/". The metrics pushed must
/// not contain a job label of their own nor any of the grouping labels.
///
/// You can use just host:port or ip:port as url, in which case 'http://' is
/// added automatically. You can also include the schema in the URL. However, do
/// not include the '/metrics/jobs/...' part.
///
/// Note that all previously pushed metrics with the same job and other grouping
/// labels will be replaced with the metrics pushed by this call. (It uses HTTP
/// method 'PUT' to push to the Pushgateway.)
pub fn push_metrics(job: &str,
                    grouping: HashMap<String, String>,
                    url: &str,
                    mfs: Vec<proto::MetricFamily>)
                    -> Result<()> {
    push(job, grouping, url, mfs, "PUT")
}

/// `push_add_metrics` works like `push_metrics`, but only previously pushed
/// metrics with the same name (and the same job and other grouping labels) will
/// be replaced. (It uses HTTP method 'POST' to push to the Pushgateway.)
pub fn push_add_metrics(job: &str,
                        grouping: HashMap<String, String>,
                        url: &str,
                        mfs: Vec<proto::MetricFamily>)
                        -> Result<()> {
    push(job, grouping, url, mfs, "POST")
}

// pub for tests
pub const LABEL_NAME_JOB: &'static str = "job";

fn push(job: &str,
          grouping: HashMap<String, String>,
          url: &str,
          mfs: Vec<proto::MetricFamily>,
          method: &str)
          -> Result<()> {

    // Suppress clippy warning needless_pass_by_value.
    let grouping = grouping;
    let mfs = mfs;

    let mut push_url = if url.contains("://") {
        url.to_owned()
    } else {
        format!("http://{}", url)
    };

    if push_url.ends_with('/') {
        push_url.pop();
    }

    let mut url_components = Vec::new();
    if job.contains('/') {
        return Err(Error::Msg(format!("job contains '/': {}", job)));
    }

    // TODO: escape job
    url_components.push(job.to_owned());

    for (ln, lv) in &grouping {
        // TODO: check label name
        if lv.contains('/') {
            return Err(Error::Msg(format!("value of grouping label {} contains '/': {}", ln, lv)));
        }
        url_components.push(ln.to_owned());
        url_components.push(lv.to_owned());
    }

    push_url = format!("{}/metrics/job/{}", push_url, url_components.join("/"));

    // Check for pre-existing grouping labels:
    for mf in &mfs {
        for m in mf.get_metric() {
            for lp in m.get_label() {
                if lp.get_name() == LABEL_NAME_JOB {
                    return Err(Error::Msg(format!("pushed metric {} already contains a \
                                                   job label",
                                                  mf.get_name())));
                }
                if grouping.contains_key(lp.get_name()) {
                    return Err(Error::Msg(format!("pushed metric {} already contains \
                                                   grouping label {}",
                                                  mf.get_name(),
                                                  lp.get_name())));
                }
            }
        }
    }

    let push_url: Url = Url::parse(&push_url).map_err(|e| Error::Msg(format!("{:?}", e)))?;
    let stream: TcpStream = TcpStream::connect(&push_url)?;
    stream.set_write_timeout(Some(time::Duration::from_secs(5)))?;
    stream.set_read_timeout(Some(time::Duration::from_secs(5)))?;

    {
        let mut buf = Vec::with_capacity(200);
        let encoder = TextEncoder::new();
        encoder.encode(&mfs, &mut buf)?;

        let mut bw = BufWriter::new(&stream);
        bw.write_fmt(format_args!("{} {} HTTP/1.1\r\n", method, push_url.path()))?;
        bw.write_fmt(format_args!("Host: {}\r\n", push_url.host_str().unwrap()))?;
        bw.write_fmt(format_args!("Content-Type: {}\r\n", encoder.format_type()))?;
        bw.write_fmt(format_args!("Content-Length: {}\r\n", buf.len()))?;
        bw.write(b"\r\n")?;
        bw.write(&buf)?;
        bw.flush()?;
    }

    let resp = {
        let mut buf = String::with_capacity(200);
        let mut br = BufReader::new(&stream);
        loop {
            let num = br.read_line(&mut buf)?;
            if num == 2 { // "\r\n" end of headers.
                // Pushgateway's responses have no body.
                break
            }
        }
        buf
    };
    let status_code = parse_resp(&resp)?;
    if status_code != "202" {
        return Err(Error::Msg(format!("unexpected status code {} while pushing to {}", status_code, push_url)));
    }

    Ok(())
}

fn parse_resp(resp: &str) -> Result<&str> {
    let status_line = resp.lines().next().ok_or(Error::Msg("empty response".to_owned()))?;
    status_line.split(' ').nth(1).ok_or(Error::Msg("empty status code".to_owned()))
}

fn push_from_collector(job: &str,
                       grouping: HashMap<String, String>,
                       url: &str,
                       collectors: Vec<Box<Collector>>,
                       method: &str)
                       -> Result<()> {
    let registry = Registry::new();
    for bc in collectors {
        registry.register(bc)?;
    }

    let mfs = registry.gather();
    push(job, grouping, url, mfs, method)
}

/// `push_collector` push metrics collected from the provided collectors. It is
/// a convenient way to push only a few metrics.
pub fn push_collector(job: &str,
                      grouping: HashMap<String, String>,
                      url: &str,
                      collectors: Vec<Box<Collector>>)
                      -> Result<()> {
    push_from_collector(job, grouping, url, collectors, "PUT")
}

/// `push_add_collector` works like `push_add_metrics`, it collects from the
/// provided collectors. It is a convenient way to push only a few metrics.
pub fn push_add_collector(job: &str,
                          grouping: HashMap<String, String>,
                          url: &str,
                          collectors: Vec<Box<Collector>>)
                          -> Result<()> {
    push_from_collector(job, grouping, url, collectors, "POST")
}

// pub for tests
pub const DEFAULT_GROUP_LABEL_PAIR: (&'static str, &'static str) = ("instance", "unknown");

/// `hostname_grouping_key` returns a label map with the only entry
/// {instance="<hostname>"}. This can be conveniently used as the grouping
/// parameter if metrics should be pushed with the hostname as label. The
/// returned map is created upon each call so that the caller is free to add more
/// labels to the map.
///
/// Note: This function returns `instance = "unknown"` in Windows.
#[cfg(not(target_os = "windows"))]
pub fn hostname_grouping_key() -> HashMap<String, String> {
    use libc;

    // Host names are limited to 255 bytes.
    //   ref: http://pubs.opengroup.org/onlinepubs/7908799/xns/gethostname.html
    let max_len = 256;
    let mut name = vec![0u8; max_len];
    match unsafe {
        libc::gethostname(name.as_mut_ptr() as *mut libc::c_char,
                          max_len as libc::size_t)
    } {
        0 => {
            let last_char = name.iter().position(|byte| *byte == 0).unwrap_or(max_len);
            labels!{
                DEFAULT_GROUP_LABEL_PAIR.0.to_owned() => str::from_utf8(&name[..last_char])
                                            .unwrap_or(DEFAULT_GROUP_LABEL_PAIR.1).to_owned(),
            }
        }
        _ => {
            labels!{DEFAULT_GROUP_LABEL_PAIR.0.to_owned() => DEFAULT_GROUP_LABEL_PAIR.1.to_owned(),}
        }
    }
}

#[cfg(target_os = "windows")]
pub fn hostname_grouping_key() -> HashMap<String, String> {
    labels!{DEFAULT_GROUP_LABEL_PAIR.0.to_owned() => DEFAULT_GROUP_LABEL_PAIR.1.to_owned(),}
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use protobuf::RepeatedField;
    use proto;

    use super::*;

    #[test]
    fn test_hostname_grouping_key() {
        let map = hostname_grouping_key();
        assert!(!map.is_empty());
    }

    #[test]
    fn test_push_bad_label_name() {
        let table = vec![// Error message: "pushed metric {} already contains a job label"
                         (LABEL_NAME_JOB, "job label"),
                         // Error message: "pushed metric {} already contains grouping label {}"
                         (DEFAULT_GROUP_LABEL_PAIR.0, "grouping label")];

        for case in table {
            let mut l = proto::LabelPair::new();
            l.set_name(case.0.to_owned());
            let mut m = proto::Metric::new();
            m.set_label(RepeatedField::from_vec(vec![l]));
            let mut mf = proto::MetricFamily::new();
            mf.set_metric(RepeatedField::from_vec(vec![m]));
            let res = push_metrics("test", hostname_grouping_key(), "mockurl", vec![mf]);
            assert!(res.unwrap_err().description().contains(case.1));
        }
    }
}
