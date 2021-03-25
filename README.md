# Prometheus Rust client library

[![Build Status](https://travis-ci.org/tikv/rust-prometheus.svg?branch=master)](https://travis-ci.org/pingcap/rust-prometheus)
[![docs.rs](https://docs.rs/prometheus/badge.svg)](https://docs.rs/prometheus)
[![crates.io](http://meritbadge.herokuapp.com/prometheus)](https://crates.io/crates/prometheus)

This is the [Rust](https://www.rust-lang.org) client library for
[Prometheus](http://prometheus.io). The main data structures and APIs are ported
from [Go client](https://github.com/prometheus/client_golang).

## Documentation

Find the latest documentation at <https://docs.rs/prometheus>.

## Advanced

### Crate features

This crate provides several optional components which can be enabled via [Cargo `[features]`](https://doc.rust-lang.org/cargo/reference/features.html):

- `protobuf`: Enable [Protocol Buffers](https://developers.google.com/protocol-buffers/) (aka Protobuf)  based exposition format. (Enabled by default)

> Notice: Since version 2.0, Prometheus no longer supports the Protobuf-based format. You can read about the reasoning behind this change in [this document](https://github.com/OpenObservability/OpenMetrics/blob/master/legacy/markdown/protobuf_vs_text.md).

- `nightly`: Enable nightly only features.

- `process`: Enable [process metrics](https://prometheus.io/docs/instrumenting/writing_clientlibs/#process-metrics) support.

- `push`: Enable [push metrics](https://prometheus.io/docs/instrumenting/pushing/) support.

### Static Metric

When using a `MetricVec` with label values known at compile time
prometheus-static-metric reduces the overhead of retrieving the concrete
`Metric` from a `MetricVec`.

See [static-metric](./static-metric) directory for details.

## Thanks

- [brian-brazil](https://github.com/brian-brazil)
- [ccmtaylor](https://github.com/ccmtaylor)
- [kamalmarhubi](https://github.com/kamalmarhubi)
- [lucab](https://github.com/lucab)
- [koushiro](https://github.com/koushiro)
