// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::borrow::Cow;
use std::io::Write;

use crate::errors::Result;
use crate::histogram::BUCKET_LABEL;
use crate::proto::{self, MetricFamily, MetricType};

use super::{check_metric_family, Encoder};

/// The text format of metric family.
pub const TEXT_FORMAT: &str = "text/plain; version=0.0.4";

const POSITIVE_INF: &str = "+Inf";
const QUANTILE: &str = "quantile";

/// An implementation of an [`Encoder`] that converts a [`MetricFamily`] proto message
/// into text format.
#[derive(Debug, Default)]
pub struct TextEncoder;

impl TextEncoder {
    /// Create a new text encoder.
    pub fn new() -> TextEncoder {
        TextEncoder
    }
}

impl Encoder for TextEncoder {
    fn encode<W: Write>(&self, metric_families: &[MetricFamily], writer: &mut W) -> Result<()> {
        for mf in metric_families {
            // Fail-fast checks.
            check_metric_family(mf)?;

            // Write `# HELP` header.
            let name = match &mf.name {
                Some(v) => &**v,
                None => "",
            };
            if let Some(help) = &mf.help {
                writer.write_all(b"# HELP ")?;
                writer.write_all(name.as_bytes())?;
                writer.write_all(b" ")?;
                writer.write_all(escape_string(help, false).as_bytes())?;
                writer.write_all(b"\n")?;
            }

            // Write `# TYPE` header.
            let metric_type = mf
                .r#type
                .map(MetricType::from_i32)
                .flatten()
                .unwrap_or_default();
            let lowercase_type = format!("{:?}", metric_type).to_lowercase();
            writer.write_all(b"# TYPE ")?;
            writer.write_all(name.as_bytes())?;
            writer.write_all(b" ")?;
            writer.write_all(lowercase_type.as_bytes())?;
            writer.write_all(b"\n")?;

            for m in &mf.metric {
                match metric_type {
                    MetricType::Counter => {
                        let value = match &m.counter {
                            Some(v) => v.value.unwrap_or_default(),
                            None => 0.0,
                        };
                        write_sample(writer, &name, None, m, None, value)?;
                    }
                    MetricType::Gauge => {
                        let value = match &m.gauge {
                            Some(v) => v.value.unwrap_or_default(),
                            None => 0.0,
                        };
                        write_sample(writer, &name, None, m, None, value)?;
                    }
                    MetricType::Histogram => {
                        if let Some(h) = &m.histogram {
                            let mut inf_seen = false;
                            for b in &h.bucket {
                                let upper_bound = b.upper_bound.unwrap_or_default();
                                write_sample(
                                    writer,
                                    &name,
                                    Some("_bucket"),
                                    m,
                                    Some((BUCKET_LABEL, &upper_bound.to_string())),
                                    b.cumulative_count.unwrap_or_default() as f64,
                                )?;
                                if upper_bound.is_sign_positive() && upper_bound.is_infinite() {
                                    inf_seen = true;
                                }
                            }
                            if !inf_seen {
                                write_sample(
                                    writer,
                                    &name,
                                    Some("_bucket"),
                                    m,
                                    Some((BUCKET_LABEL, POSITIVE_INF)),
                                    h.sample_count.unwrap_or_default() as f64,
                                )?;
                            }

                            write_sample(
                                writer,
                                &name,
                                Some("_sum"),
                                m,
                                None,
                                h.sample_sum.unwrap_or_default(),
                            )?;

                            write_sample(
                                writer,
                                &name,
                                Some("_count"),
                                m,
                                None,
                                h.sample_count.unwrap_or_default() as f64,
                            )?;
                        }
                    }
                    MetricType::Summary => {
                        if let Some(s) = &m.summary {
                            for q in &s.quantile {
                                write_sample(
                                    writer,
                                    &name,
                                    None,
                                    m,
                                    Some((QUANTILE, &q.quantile.unwrap_or_default().to_string())),
                                    q.value.unwrap_or_default(),
                                )?;
                            }

                            write_sample(
                                writer,
                                &name,
                                Some("_sum"),
                                m,
                                None,
                                s.sample_sum.unwrap_or_default(),
                            )?;

                            write_sample(
                                writer,
                                &name,
                                Some("_count"),
                                m,
                                None,
                                s.sample_count.unwrap_or_default() as f64,
                            )?;
                        }
                    }
                    MetricType::Untyped => {
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
/// metric name, an optional metric name postfix, the metric proto message
/// itself, optionally an additional label name and value (use empty strings if
/// not required), and the value. The function returns the number of bytes
/// written and any error encountered.
fn write_sample(
    writer: &mut dyn Write,
    name: &str,
    name_postfix: Option<&str>,
    mc: &proto::Metric,
    additional_label: Option<(&str, &str)>,
    value: f64,
) -> Result<()> {
    writer.write_all(name.as_bytes())?;
    if let Some(postfix) = name_postfix {
        writer.write_all(postfix.as_bytes())?;
    }

    label_pairs_to_text(&*mc.label, additional_label, writer)?;

    writer.write_all(b" ")?;
    writer.write_all(value.to_string().as_bytes())?;

    if let Some(timestamp) = mc.timestamp_ms {
        writer.write_all(b" ")?;
        writer.write_all(timestamp.to_string().as_bytes())?;
    }

    writer.write_all(b"\n")?;

    Ok(())
}

/// `label_pairs_to_text` converts a slice of `LabelPair` proto messages plus
/// the explicitly given additional label pair into text formatted as required
/// by the text format and writes it to `writer`. An empty slice in combination
/// with an empty string `additional_label_name` results in nothing being
/// written. Otherwise, the label pairs are written, escaped as required by the
/// text format, and enclosed in '{...}'. The function returns the number of
/// bytes written and any error encountered.
fn label_pairs_to_text(
    pairs: &[proto::LabelPair],
    additional_label: Option<(&str, &str)>,
    writer: &mut dyn Write,
) -> Result<()> {
    if pairs.is_empty() && additional_label.is_none() {
        return Ok(());
    }

    let mut separator = b"{";
    for lp in pairs {
        let name = match &lp.name {
            Some(v) => &**v,
            None => "",
        };
        let value = match &lp.value {
            Some(v) => &**v,
            None => "",
        };
        writer.write_all(separator)?;
        writer.write_all(name.as_bytes())?;
        writer.write_all(b"=\"")?;
        writer.write_all(escape_string(value, true).as_bytes())?;
        writer.write_all(b"\"")?;

        separator = b",";
    }

    if let Some((name, value)) = additional_label {
        writer.write_all(separator)?;
        writer.write_all(name.as_bytes())?;
        writer.write_all(b"=\"")?;
        writer.write_all(escape_string(value, true).as_bytes())?;
        writer.write_all(b"\"")?;
    }

    writer.write_all(b"}")?;

    Ok(())
}

fn find_first_occurence(v: &str, include_double_quote: bool) -> Option<usize> {
    if include_double_quote {
        memchr::memchr3(b'\\', b'\n', b'\"', v.as_bytes())
    } else {
        memchr::memchr2(b'\\', b'\n', v.as_bytes())
    }
}

/// `escape_string` replaces `\` by `\\`, new line character by `\n`, and `"` by `\"` if
/// `include_double_quote` is true.
///
/// Implementation adapted from
/// https://lise-henry.github.io/articles/optimising_strings.html
fn escape_string(v: &str, include_double_quote: bool) -> Cow<'_, str> {
    let first_occurence = find_first_occurence(v, include_double_quote);

    if let Some(first) = first_occurence {
        let mut escaped = String::with_capacity(v.len() * 2);
        escaped.push_str(&v[0..first]);
        let remainder = v[first..].chars();

        for c in remainder {
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
        escaped.into()
    } else {
        // The input string does not contain any characters that would need to
        // be escaped. Return it as it is.
        v.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::counter::Counter;
    use crate::gauge::Gauge;
    use crate::histogram::{Histogram, HistogramOpts};
    use crate::metrics::{Collector, Opts};

    #[test]
    fn test_escape_string() {
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
        let counter_opts = Opts::new("test_counter", "test help")
            .const_label("a", "1")
            .const_label("b", "2");
        let counter = Counter::with_opts(counter_opts).unwrap();
        counter.inc();

        let mf = counter.collect();
        let mut writer = Vec::<u8>::new();
        let encoder = TextEncoder::new();
        let txt = encoder.encode(&mf, &mut writer);
        assert!(txt.is_ok());

        let counter_ans = r##"# HELP test_counter test help
# TYPE test_counter counter
test_counter{a="1",b="2"} 1
"##;
        assert_eq!(counter_ans.as_bytes(), writer.as_slice());

        let gauge_opts = Opts::new("test_gauge", "test help")
            .const_label("a", "1")
            .const_label("b", "2");
        let gauge = Gauge::with_opts(gauge_opts).unwrap();
        gauge.inc();
        gauge.set(42.0);

        let mf = gauge.collect();
        writer.clear();
        let txt = encoder.encode(&mf, &mut writer);
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
        let res = encoder.encode(&mf, &mut writer);
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

    #[test]
    fn test_text_encoder_summary() {
        use crate::proto::{Metric, Quantile, Summary};
        use std::str;

        let mut metric_family = MetricFamily::default();
        metric_family.name = Some("test_summary".to_string());
        metric_family.help = Some("This is a test summary statistic".to_string());
        metric_family.r#type = Some(MetricType::Summary.into());

        let mut summary = Summary::default();
        summary.sample_count = Some(5.0 as u64);
        summary.sample_sum = Some(15.0);

        let mut quantile1 = Quantile::default();
        quantile1.quantile = Some(50.0);
        quantile1.value = Some(3.0);

        let mut quantile2 = Quantile::default();
        quantile2.quantile = Some(100.0);
        quantile2.value = Some(5.0);

        summary.quantile = vec![quantile1, quantile2];

        let mut metric = Metric::default();
        metric.summary = Some(summary);
        metric_family.metric = vec![metric];

        let mut writer = Vec::<u8>::new();
        let encoder = TextEncoder::new();
        let res = encoder.encode(&vec![metric_family], &mut writer);
        assert!(res.is_ok());

        let ans = r##"# HELP test_summary This is a test summary statistic
# TYPE test_summary summary
test_summary{quantile="50"} 3
test_summary{quantile="100"} 5
test_summary_sum 15
test_summary_count 5
"##;
        assert_eq!(ans, str::from_utf8(writer.as_slice()).unwrap());
    }
}
