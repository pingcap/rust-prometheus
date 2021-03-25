// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

//! Non-generated version of required structures provided by the protobuf.
//! This version is used when the `protobuf` feature is turned off.

#![allow(missing_docs)]

#[derive(PartialEq, Clone, Default, Debug)]
pub struct LabelPair {
    pub name: Option<String>,
    pub value: Option<String>,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct Gauge {
    pub value: Option<f64>,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct Counter {
    pub value: Option<f64>,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct Quantile {
    pub quantile: Option<f64>,
    pub value: Option<f64>,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct Summary {
    pub sample_count: Option<u64>,
    pub sample_sum: Option<f64>,
    pub quantile: Vec<Quantile>,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct Untyped {
    pub value: Option<f64>,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct Histogram {
    pub sample_count: Option<u64>,
    pub sample_sum: Option<f64>,
    pub bucket: Vec<Bucket>,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct Bucket {
    pub cumulative_count: Option<u64>,
    pub upper_bound: Option<f64>,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct Metric {
    // message fields
    pub label: Vec<LabelPair>,
    pub gauge: Option<Gauge>,
    pub counter: Option<Counter>,
    pub summary: Option<Summary>,
    pub untyped: Option<Untyped>,
    pub histogram: Option<Histogram>,
    pub timestamp_ms: Option<i64>,
}

#[derive(Clone, PartialEq, Eq, Debug, Hash, Copy)]
pub enum MetricType {
    Counter = 0,
    Gauge = 1,
    Summary = 2,
    Untyped = 3,
    Histogram = 4,
}

impl MetricType {
    pub fn from_i32(value: i32) -> Option<MetricType> {
        match value {
            0 => Some(Self::Counter),
            1 => Some(Self::Gauge),
            2 => Some(Self::Summary),
            3 => Some(Self::Untyped),
            4 => Some(Self::Histogram),
            _ => None,
        }
    }
}

impl Default for MetricType {
    fn default() -> Self {
        MetricType::Counter
    }
}

impl From<MetricType> for i32 {
    fn from(value: MetricType) -> Self {
        value as i32
    }
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct MetricFamily {
    pub name: Option<String>,
    pub help: Option<String>,
    pub r#type: Option<i32>,
    pub metric: Vec<Metric>,
}
