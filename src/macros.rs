// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

/// Create labels with specified name-value pairs.
///
/// # Examples
///
/// ```
/// # use std::collections::HashMap;
/// # use prometheus::labels;
/// # fn main() {
/// let labels = labels!{
///     "test" => "hello",
///     "foo" => "bar",
/// };
/// assert_eq!(labels.len(), 2);
/// assert!(labels.get("test").is_some());
/// assert_eq!(*(labels.get("test").unwrap()), "hello");
///
/// let labels: HashMap<&str, &str> = labels!{};
/// assert!(labels.is_empty());
/// # }
/// ```
#[macro_export]
macro_rules! labels {
    ( $( $ KEY : expr => $ VALUE : expr ),* $(,)? ) => {
        {
            use std::collections::HashMap;

            let mut lbs = HashMap::new();
            $(
                lbs.insert($KEY, $VALUE);
            )*

            lbs
        }
    };
}

/// Create an [`Opts`].
///
/// # Examples
///
/// ```
/// # use prometheus::{labels, opts};
/// # fn main() {
/// let name = "test_opts";
/// let help = "test opts help";
///
/// let opts = opts!(name, help);
/// assert_eq!(opts.name, name);
/// assert_eq!(opts.help, help);
///
/// let opts = opts!(name, help, labels!{"test" => "hello", "foo" => "bar",});
/// assert_eq!(opts.const_labels.len(), 2);
/// assert!(opts.const_labels.get("foo").is_some());
/// assert_eq!(opts.const_labels.get("foo").unwrap(), "bar");
///
/// let opts = opts!(name,
///                  help,
///                  labels!{"test" => "hello", "foo" => "bar",},
///                  labels!{"ans" => "42",});
/// assert_eq!(opts.const_labels.len(), 3);
/// assert!(opts.const_labels.get("ans").is_some());
/// assert_eq!(opts.const_labels.get("ans").unwrap(), "42");
/// # }
/// ```
#[macro_export]
macro_rules! opts {
    ( $ NAME : expr , $ HELP : expr $ ( , $ CONST_LABELS : expr ) * ) => {
        {
            use std::collections::HashMap;

            let opts = $crate::Opts::new($NAME, $HELP);
            let lbs = HashMap::<String, String>::new();
            $(
                let mut lbs = lbs;
                lbs.extend($CONST_LABELS.iter().map(|(k, v)| ((*k).into(), (*v).into())));
            )*

            opts.const_labels(lbs)
        }
    }
}

/// Create a [`HistogramOpts`].
///
/// # Examples
///
/// ```
/// # use prometheus::linear_buckets;
/// # use prometheus::{histogram_opts, labels};
/// # fn main() {
/// let name = "test_histogram_opts";
/// let help = "test opts help";
///
/// let opts = histogram_opts!(name, help);
/// assert_eq!(opts.common_opts.name, name);
/// assert_eq!(opts.common_opts.help, help);
///
/// let opts = histogram_opts!(name, help, linear_buckets(1.0, 0.5, 4).unwrap());
/// assert_eq!(opts.common_opts.name, name);
/// assert_eq!(opts.common_opts.help, help);
/// assert_eq!(opts.buckets.len(), 4);
///
/// let opts = histogram_opts!(name,
///                            help,
///                            vec![1.0, 2.0],
///                            labels!{"key".to_string() => "value".to_string(),});
/// assert_eq!(opts.common_opts.name, name);
/// assert_eq!(opts.common_opts.help, help);
/// assert_eq!(opts.buckets.len(), 2);
/// assert!(opts.common_opts.const_labels.get("key").is_some());
/// assert_eq!(opts.common_opts.const_labels.get("key").unwrap(), "value");
/// # }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! histogram_opts {
    ($NAME:expr, $HELP:expr) => {{
        $crate::HistogramOpts::new($NAME, $HELP)
    }};

    ($NAME:expr, $HELP:expr, $BUCKETS:expr) => {{
        let hopts = $crate::histogram_opts!($NAME, $HELP);
        hopts.buckets($BUCKETS)
    }};

    ($NAME:expr, $HELP:expr, $BUCKETS:expr, $CONST_LABELS:expr) => {{
        let hopts = $crate::histogram_opts!($NAME, $HELP, $BUCKETS);
        hopts.const_labels($CONST_LABELS)
    }};
}

/// Create a [`Counter`] and registers to default registry.
///
/// # Examples
///
/// ```
/// # use prometheus::{opts, register_counter};
/// # fn main() {
/// let opts = opts!("test_macro_counter_1", "help");
/// let res1 = register_counter!(opts);
/// assert!(res1.is_ok());
///
/// let res2 = register_counter!("test_macro_counter_2", "help");
/// assert!(res2.is_ok());
/// # }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! register_counter {
    (@of_type $TYPE:ident, $OPTS:expr) => {{
        let counter = $crate::$TYPE::with_opts($OPTS).unwrap();
        $crate::register(Box::new(counter.clone())).map(|_| counter)
    }};

    ($OPTS:expr) => {{
        $crate::register_counter!(@of_type Counter, $OPTS)
    }};

    ($NAME:expr, $HELP:expr) => {{
        $crate::register_counter!(opts!($NAME, $HELP))
    }};
}

/// Create an [`IntCounter`] and registers to default registry.
///
/// View docs of `register_counter` for examples.
#[macro_export(local_inner_macros)]
macro_rules! register_int_counter {
    ($OPTS:expr) => {{
        $crate::register_counter!(@of_type IntCounter, $OPTS)
    }};

    ($NAME:expr, $HELP:expr) => {{
        $crate::register_int_counter!(opts!($NAME, $HELP))
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! __register_counter_vec {
    ($TYPE:ident, $OPTS:expr, $LABELS_NAMES:expr) => {{
        let counter_vec = $crate::$TYPE::new($OPTS, $LABELS_NAMES).unwrap();
        $crate::register(Box::new(counter_vec.clone())).map(|_| counter_vec)
    }};
}

/// Create a [`CounterVec`] and registers to default registry.
///
/// # Examples
///
/// ```
/// # use prometheus::{opts, register_counter_vec};
/// # fn main() {
/// let opts = opts!("test_macro_counter_vec_1", "help");
/// let counter_vec = register_counter_vec!(opts, &["a", "b"]);
/// assert!(counter_vec.is_ok());
///
/// let counter_vec = register_counter_vec!("test_macro_counter_vec_2", "help", &["a", "b"]);
/// assert!(counter_vec.is_ok());
/// # }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! register_counter_vec {
    ($OPTS:expr, $LABELS_NAMES:expr) => {{
        $crate::__register_counter_vec!(CounterVec, $OPTS, $LABELS_NAMES)
    }};

    ($NAME:expr, $HELP:expr, $LABELS_NAMES:expr) => {{
        $crate::register_counter_vec!(opts!($NAME, $HELP), $LABELS_NAMES)
    }};
}

/// Create an [`IntCounterVec`] and registers to default registry.
///
/// View docs of `register_counter_vec` for examples.
#[macro_export(local_inner_macros)]
macro_rules! register_int_counter_vec {
    ($OPTS:expr, $LABELS_NAMES:expr) => {{
        $crate::__register_counter_vec!(IntCounterVec, $OPTS, $LABELS_NAMES)
    }};

    ($NAME:expr, $HELP:expr, $LABELS_NAMES:expr) => {{
        $crate::register_int_counter_vec!(opts!($NAME, $HELP), $LABELS_NAMES)
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! __register_gauge {
    ($TYPE:ident, $OPTS:expr) => {{
        let gauge = $crate::$TYPE::with_opts($OPTS).unwrap();
        $crate::register(Box::new(gauge.clone())).map(|_| gauge)
    }};
}

/// Create a [`Gauge`] and registers to default registry.
///
/// # Examples
///
/// ```
/// # use prometheus::{opts, register_gauge};
/// # fn main() {
/// let opts = opts!("test_macro_gauge", "help");
/// let res1 = register_gauge!(opts);
/// assert!(res1.is_ok());
///
/// let res2 = register_gauge!("test_macro_gauge_2", "help");
/// assert!(res2.is_ok());
/// # }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! register_gauge {
    ($OPTS:expr) => {{
        $crate::__register_gauge!(Gauge, $OPTS)
    }};

    ($NAME:expr, $HELP:expr) => {{
        $crate::register_gauge!(opts!($NAME, $HELP))
    }};
}

/// Create an [`IntGauge`] and registers to default registry.
///
/// View docs of `register_gauge` for examples.
#[macro_export(local_inner_macros)]
macro_rules! register_int_gauge {
    ($OPTS:expr) => {{
        $crate::__register_gauge!(IntGauge, $OPTS)
    }};

    ($NAME:expr, $HELP:expr) => {{
        $crate::register_int_gauge!(opts!($NAME, $HELP))
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! __register_gauge_vec {
    ($TYPE:ident, $OPTS:expr, $LABELS_NAMES:expr) => {{
        let gauge_vec = $crate::$TYPE::new($OPTS, $LABELS_NAMES).unwrap();
        $crate::register(Box::new(gauge_vec.clone())).map(|_| gauge_vec)
    }};
}

/// Create a [`GaugeVec`] and registers to default registry.
///
/// # Examples
///
/// ```
/// # use prometheus::{opts, register_gauge_vec};
/// # fn main() {
/// let opts = opts!("test_macro_gauge_vec_1", "help");
/// let gauge_vec = register_gauge_vec!(opts, &["a", "b"]);
/// assert!(gauge_vec.is_ok());
///
/// let gauge_vec = register_gauge_vec!("test_macro_gauge_vec_2", "help", &["a", "b"]);
/// assert!(gauge_vec.is_ok());
/// # }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! register_gauge_vec {
    ($OPTS:expr, $LABELS_NAMES:expr) => {{
        $crate::__register_gauge_vec!(GaugeVec, $OPTS, $LABELS_NAMES)
    }};

    ($NAME:expr, $HELP:expr, $LABELS_NAMES:expr) => {{
        $crate::register_gauge_vec!(opts!($NAME, $HELP), $LABELS_NAMES)
    }};
}

/// Create an [`IntGaugeVec`] and registers to default registry.
///
/// View docs of `register_gauge_vec` for examples.
#[macro_export(local_inner_macros)]
macro_rules! register_int_gauge_vec {
    ($OPTS:expr, $LABELS_NAMES:expr) => {{
        $crate::__register_gauge_vec!(IntGaugeVec, $OPTS, $LABELS_NAMES)
    }};

    ($NAME:expr, $HELP:expr, $LABELS_NAMES:expr) => {{
        $crate::register_int_gauge_vec!(opts!($NAME, $HELP), $LABELS_NAMES)
    }};
}

/// Create a [`Histogram`] and registers to default registry.
///
/// # Examples
///
/// ```
/// # use prometheus::{histogram_opts, register_histogram};
/// # fn main() {
/// let opts = histogram_opts!("test_macro_histogram", "help");
/// let res1 = register_histogram!(opts);
/// assert!(res1.is_ok());
///
/// let res2 = register_histogram!("test_macro_histogram_2", "help");
/// assert!(res2.is_ok());
///
/// let res3 = register_histogram!("test_macro_histogram_4",
///                                 "help",
///                                 vec![1.0, 2.0]);
/// assert!(res3.is_ok());
/// # }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! register_histogram {
    ($NAME:expr, $HELP:expr) => {
        $crate::register_histogram!(histogram_opts!($NAME, $HELP))
    };

    ($NAME:expr, $HELP:expr, $BUCKETS:expr) => {
        $crate::register_histogram!(histogram_opts!($NAME, $HELP, $BUCKETS))
    };

    ($HOPTS:expr) => {{
        let histogram = $crate::Histogram::with_opts($HOPTS).unwrap();
        $crate::register(Box::new(histogram.clone())).map(|_| histogram)
    }};
}

/// Create a [`HistogramVec`] and registers to default registry.
///
/// # Examples
///
/// ```
/// # use prometheus::{histogram_opts, register_histogram_vec};
/// # fn main() {
/// let opts = histogram_opts!("test_macro_histogram_vec_1", "help");
/// let histogram_vec = register_histogram_vec!(opts, &["a", "b"]);
/// assert!(histogram_vec.is_ok());
///
/// let histogram_vec =
///     register_histogram_vec!("test_macro_histogram_vec_2", "help", &["a", "b"]);
/// assert!(histogram_vec.is_ok());
///
/// let histogram_vec = register_histogram_vec!("test_macro_histogram_vec_3",
///                                             "help",
///                                             &["test_label"],
///                                             vec![0.0, 1.0, 2.0]);
/// assert!(histogram_vec.is_ok());
/// # }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! register_histogram_vec {
    ($HOPTS:expr, $LABELS_NAMES:expr) => {{
        let histogram_vec = $crate::HistogramVec::new($HOPTS, $LABELS_NAMES).unwrap();
        $crate::register(Box::new(histogram_vec.clone())).map(|_| histogram_vec)
    }};

    ($NAME:expr, $HELP:expr, $LABELS_NAMES:expr) => {{
        $crate::register_histogram_vec!(histogram_opts!($NAME, $HELP), $LABELS_NAMES)
    }};

    ($NAME:expr, $HELP:expr, $LABELS_NAMES:expr, $BUCKETS:expr) => {{
        $crate::register_histogram_vec!(histogram_opts!($NAME, $HELP, $BUCKETS), $LABELS_NAMES)
    }};
}
