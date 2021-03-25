// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

/*!
This crate provides staticly built metrics to your Prometheus application.

This is useful since it reduces the amount of branching and processing needed at runtime to collect metrics.

```rust
use prometheus::{self, IntCounter, TextEncoder, Encoder};

use lazy_static::lazy_static;
use prometheus::register_int_counter;

lazy_static! {
    static ref HIGH_FIVE_COUNTER: IntCounter =
        register_int_counter!("highfives", "Number of high fives recieved").unwrap();
}

HIGH_FIVE_COUNTER.inc();
assert_eq!(HIGH_FIVE_COUNTER.get(), 1);
```

Is it reccomended that you consult the [`prometheus` documentation for more information.](https://docs.rs/prometheus/)
*/
mod auto_flush_builder;
mod auto_flush_from;
mod builder;
mod parser;
mod register_macro;
mod util;

use proc_macro::TokenStream;

use self::builder::TokensBuilder;
use self::parser::StaticMetricMacroBody;
use self::register_macro::RegisterMethodInvoking;
use crate::auto_flush_from::AutoFlushFromDef;
use auto_flush_builder::AutoFlushTokensBuilder;

/// Build static metrics.
#[proc_macro]
pub fn make_static_metric(input: TokenStream) -> TokenStream {
    let body: StaticMetricMacroBody = syn::parse(input).unwrap();
    TokensBuilder::build(body).into()
}

/// Build auto flush able static metrics.
/// refer to https://github.com/tikv/rust-prometheus/tree/master/static-metric for more info.
#[proc_macro]
pub fn make_auto_flush_static_metric(input: TokenStream) -> TokenStream {
    let body: StaticMetricMacroBody = syn::parse(input).unwrap();
    AutoFlushTokensBuilder::build(body).into()
}

/// Instantiate a auto flush able static metric struct from a HistogramVec or CounterVec.
#[proc_macro]
pub fn auto_flush_from(input: TokenStream) -> TokenStream {
    let def: AutoFlushFromDef = syn::parse(input).unwrap();
    def.auto_flush_from()
}

/// Register a `CounterVec` and create static metrics from it.
#[proc_macro]
pub fn register_static_counter_vec(input: TokenStream) -> TokenStream {
    register_static_vec("counter", input)
}

/// Register a `IntCounterVec` and create static metrics from it.
#[proc_macro]
pub fn register_static_int_counter_vec(input: TokenStream) -> TokenStream {
    register_static_vec("int_counter", input)
}

/// Register a `GaugeVec` and create static metrics from it.
#[proc_macro]
pub fn register_static_gauge_vec(input: TokenStream) -> TokenStream {
    register_static_vec("gauge", input)
}

/// Register a `IntGaugeVec` and create static metrics from it.
#[proc_macro]
pub fn register_static_int_gauge_vec(input: TokenStream) -> TokenStream {
    register_static_vec("int_gauge", input)
}

/// Register a `HistogramVec` and create static metrics from it.
#[proc_macro]
pub fn register_static_histogram_vec(input: TokenStream) -> TokenStream {
    register_static_vec("histogram", input)
}

/// Register a `IntHistogramVec` and create static metrics from it.
#[proc_macro]
pub fn register_static_int_histogram_vec(input: TokenStream) -> TokenStream {
    register_static_vec("int_histogram", input)
}

/// Procedural macro handler for `register_static_xxx_vec!`.
fn register_static_vec(register_type: &str, input: TokenStream) -> TokenStream {
    let invoking: RegisterMethodInvoking = syn::parse(input).unwrap();
    invoking.into_tokens(register_type).into()
}
