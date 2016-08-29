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

use std::io::Write;

use errors::{Result, Error};
use proto::MetricFamily;
use proto::{self, MetricType};
use histogram::BUCKET_LABEL;

pub trait Encoder {
    /// `encode` converts a slice of MetricFamily proto messages into target
    /// format and writes the resulting lines to `writer`. It returns the number
    /// of bytes written and any error encountered. This function does not
    /// perform checks on the content of the metric and label names,
    /// i.e. invalid metric or label names will result in invalid text format
    /// output.
    fn encode(&self, &[MetricFamily], &mut Write) -> Result<()>;

    /// `format_type` returns target format.
    fn format_type(&self) -> &str;
}

pub type Format = &'static str;

pub const TEXT_FORMAT: Format = "text/plain; version=0.0.4";

const POSITIVE_INF: &'static str = "+Inf";

/// Implementation of an `Encoder` that converts a `MetricFamily` proto message
/// into text format.
#[derive(Debug, Default)]
pub struct TextEncoder;

impl TextEncoder {
    pub fn new() -> TextEncoder {
        TextEncoder
    }
}

impl Encoder for TextEncoder {
    fn encode(&self, metric_familys: &[MetricFamily], writer: &mut Write) -> Result<()> {
        for mf in metric_familys {
            if mf.get_metric().is_empty() {
                return Err(Error::Msg("MetricFamily has no metrics".to_owned()));
            }

            let name = mf.get_name();
            if name.is_empty() {
                return Err(Error::Msg("MetricFamily has no name".to_owned()));
            }

            let help = mf.get_help();
            if !help.is_empty() {
                try!(write!(writer, "# HELP {} {}\n", name, escape_string(help, false)));
            }

            let metric_type = mf.get_field_type();
            let lowercase_type = format!("{:?}", metric_type).to_lowercase();
            try!(write!(writer, "# TYPE {} {}\n", name, lowercase_type));

            for m in mf.get_metric() {
                match metric_type {
                    MetricType::COUNTER => {
                        try!(write_sample(name, m, "", "", m.get_counter().get_value(), writer));
                    }
                    MetricType::GAUGE => {
                        try!(write_sample(name, m, "", "", m.get_gauge().get_value(), writer));
                    }
                    MetricType::HISTOGRAM => {
                        let h = m.get_histogram();

                        let mut inf_seen = false;
                        for b in h.get_bucket() {
                            let upper_bound = b.get_upper_bound();
                            try!(write_sample(&format!("{}_bucket", name),
                                              m,
                                              BUCKET_LABEL,
                                              &format!("{}", upper_bound),
                                              b.get_cumulative_count() as f64,
                                              writer));
                            if upper_bound.is_sign_positive() && upper_bound.is_infinite() {
                                inf_seen = true;
                            }
                        }
                        if !inf_seen {
                            try!(write_sample(&format!("{}_bucket", name),
                                              m,
                                              BUCKET_LABEL,
                                              POSITIVE_INF,
                                              h.get_sample_count() as f64,
                                              writer));
                        }

                        try!(write_sample(&format!("{}_sum", name),
                                          m,
                                          "",
                                          "",
                                          h.get_sample_sum(),
                                          writer));

                        try!(write_sample(&format!("{}_count", name),
                                          m,
                                          "",
                                          "",
                                          h.get_sample_count() as f64,
                                          writer));
                    }
                    MetricType::SUMMARY | MetricType::UNTYPED => {
                        unimplemented!();
                    }
                }
            }
        }

        Ok(())
    }

    fn format_type(&self) -> &str {
        TEXT_FORMAT
    }
}

/// `write_sample` writes a single sample in text format to `writer`, given the
/// metric name, the metric proto message itself, optionally an additional label
/// name and value (use empty strings if not required), and the value.
/// The function returns the number of bytes written and any error encountered.
fn write_sample(name: &str,
                mc: &proto::Metric,
                additional_label_name: &str,
                additional_label_value: &str,
                value: f64,
                writer: &mut Write)
                -> Result<()> {
    try!(writer.write_all(name.as_bytes()));

    try!(label_pairs_to_text(mc.get_label(),
                             additional_label_name,
                             additional_label_value,
                             writer));

    try!(write!(writer, " {}", value));

    let timestamp = mc.get_timestamp_ms();
    if timestamp != 0 {
        try!(write!(writer, " {}", timestamp));
    }

    try!(writer.write_all(b"\n"));

    Ok(())
}

/// `label_pairs_to_text` converts a slice of `LabelPair` proto messages plus
/// the explicitly given additional label pair into text formatted as required
/// by the text format and writes it to `writer`. An empty slice in combination
/// with an empty string `additional_label_name` results in nothing being
/// written. Otherwise, the label pairs are written, escaped as required by the
/// text format, and enclosed in '{...}'. The function returns the number of
/// bytes written and any error encountered.
fn label_pairs_to_text(pairs: &[proto::LabelPair],
                       additional_label_name: &str,
                       additional_label_value: &str,
                       writer: &mut Write)
                       -> Result<()> {
    if pairs.is_empty() && additional_label_name.is_empty() {
        return Ok(());
    }

    let mut separator = "{";
    for lp in pairs {
        try!(write!(writer,
                    "{}{}=\"{}\"",
                    separator,
                    lp.get_name(),
                    escape_string(lp.get_value(), true)));

        separator = ",";
    }

    if !additional_label_name.is_empty() {
        try!(write!(writer,
                    "{}{}=\"{}\"",
                    separator,
                    additional_label_name,
                    escape_string(additional_label_value, true)));
    }

    try!(writer.write_all(b"}"));

    Ok(())
}

/// `escape_string` replaces '\' by '\\', new line character by '\n', and - if
/// `include_double_quote` is true - '"' by '\"'.
pub fn escape_string(v: &str, include_double_quote: bool) -> String {
    let mut escaped = String::with_capacity(v.len() * 2);

    for c in v.chars() {
        match c {
            '\\' | '\n' => {
                escaped.extend(c.escape_default());
            }
            '"' if include_double_quote => {
                escaped.extend(c.escape_default());
            }
            _ => {
                escaped.push(c);
            }
        }
    }

    escaped.shrink_to_fit();

    escaped
}

#[cfg(test)]
mod tests {
    use counter::Counter;
    use gauge::Gauge;
    use metrics::{Opts, Collector};
    use histogram::{Histogram, HistogramOpts};

    use super::*;

    #[test]
    fn test_ecape_string() {
        assert_eq!(r"\\", escape_string("\\", false));
        assert_eq!(r"a\\", escape_string("a\\", false));
        assert_eq!(r"\n", escape_string("\n", false));
        assert_eq!(r"a\n", escape_string("a\n", false));
        assert_eq!(r"\\n", escape_string("\\n", false));

        assert_eq!(r##"\\n\""##, escape_string("\\n\"", true));
        assert_eq!(r##"\\\n\""##, escape_string("\\\n\"", true));
        assert_eq!(r##"\\\\n\""##, escape_string("\\\\n\"", true));
        assert_eq!(r##"\"\\n\""##, escape_string("\"\\n\"", true));
    }

    #[test]
    fn test_text_encoder() {
        let counter_opts =
            Opts::new("test_counter", "test help").const_label("a", "1").const_label("b", "2");
        let counter = Counter::with_opts(counter_opts).unwrap();
        counter.inc();

        let mf = counter.collect();
        let mut writer = Vec::<u8>::new();
        let encoder = TextEncoder::new();
        let txt = encoder.encode(&[mf], &mut writer);
        assert!(txt.is_ok());

        let counter_ans = r##"# HELP test_counter test help
# TYPE test_counter counter
test_counter{a="1",b="2"} 1
"##;
        assert_eq!(counter_ans.as_bytes(), writer.as_slice());

        let gauge_opts =
            Opts::new("test_gauge", "test help").const_label("a", "1").const_label("b", "2");
        let gauge = Gauge::with_opts(gauge_opts).unwrap();
        gauge.inc();
        gauge.set(42.0);

        let mf = gauge.collect();
        writer.clear();
        let txt = encoder.encode(&[mf], &mut writer);
        assert!(txt.is_ok());

        let gauge_ans = r##"# HELP test_gauge test help
# TYPE test_gauge gauge
test_gauge{a="1",b="2"} 42
"##;
        assert_eq!(gauge_ans.as_bytes(), writer.as_slice());
    }

    #[test]
    fn test_text_encoder_histogram() {
        let opts = HistogramOpts::new("test_histogram", "test help").const_label("a", "1");
        let histogram = Histogram::with_opts(opts).unwrap();
        histogram.observe(0.25);

        let mf = histogram.collect();
        let mut writer = Vec::<u8>::new();
        let encoder = TextEncoder::new();
        let res = encoder.encode(&[mf], &mut writer);
        assert!(res.is_ok());

        let ans = r##"# HELP test_histogram test help
# TYPE test_histogram histogram
test_histogram_bucket{a="1",le="0.005"} 0
test_histogram_bucket{a="1",le="0.01"} 0
test_histogram_bucket{a="1",le="0.025"} 0
test_histogram_bucket{a="1",le="0.05"} 0
test_histogram_bucket{a="1",le="0.1"} 0
test_histogram_bucket{a="1",le="0.25"} 1
test_histogram_bucket{a="1",le="0.5"} 1
test_histogram_bucket{a="1",le="1"} 1
test_histogram_bucket{a="1",le="2.5"} 1
test_histogram_bucket{a="1",le="5"} 1
test_histogram_bucket{a="1",le="10"} 1
test_histogram_bucket{a="1",le="+Inf"} 1
test_histogram_sum{a="1"} 0.25
test_histogram_count{a="1"} 1
"##;
        assert_eq!(ans.as_bytes(), writer.as_slice());
    }
}
