# Prometheus Rust client library

[![Build Status](https://travis-ci.org/pingcap/rust-prometheus.svg?branch=master)](https://travis-ci.org/pingcap/rust-prometheus)
[![docs.rs](https://docs.rs/prometheus/badge.svg?version=0.2)](https://docs.rs/prometheus)
[![crates.io](http://meritbadge.herokuapp.com/prometheus)](https://crates.io/crates/prometheus)

This is the [Rust](https://www.rust-lang.org) client library for [Prometheus](http://prometheus.io).
The main Structures and APIs are ported from [Go client](https://github.com/prometheus/client_golang).

## Usage

+ Add this to your `Cargo.toml`:

    ```toml
    [dependencies]
    prometheus = "0.2"
    ```

+ Add this to your crate in `lib.rs`:

    ```rust
    extern crate prometheus
    ```

## Example

```rust
use prometheus::{Opts, Registry, Counter, TextEncoder, Encoder};

// Create a Counter.
let counter_opts = Opts::new("test_counter", "test counter help");
let counter = Counter::with_opts(counter_opts).unwrap();

// Create a Registry and register Counter. 
let r = Registry::new();
r.register(Box::new(counter.clone())).unwrap();

// Inc.
counter.inc();

// Gather the metrics.
let mut buffer = vec![];
let encoder = TextEncoder::new();
let metric_familys = r.gather();
encoder.encode(&metric_familys, &mut buffer).unwrap();

// Output to the standard output.
println!("{}", String::from_utf8(buffer).unwrap());
```

[More Examples](./examples)
