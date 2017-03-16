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

/// Create labes with specify name-value pairs.
///
/// # Examples
///
/// ```
/// # #[macro_use] extern crate prometheus;
/// # use std::collections::HashMap;
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
    () => {
        {
            use std::collections::HashMap;

            HashMap::new()
        }
    };

    ( $ ( $ KEY : expr => $ VALUE : expr , ) + ) => {
        {
            use std::collections::HashMap;

            let mut lbs = HashMap::new();
            $(
                lbs.insert($KEY, $VALUE);
            )+

            lbs
        }
    }
}

/// Create an `Opts`.
///
/// # Examples
///
/// ```
/// # #[macro_use] extern crate prometheus;
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

/// Create a `HistogramOpts`
///
/// # Examples
///
/// ```
/// # #[macro_use] extern crate prometheus;
/// # use prometheus::linear_buckets;
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
#[macro_export]
macro_rules! histogram_opts {
    ( $ NAME : expr , $ HELP : expr ) => {
        {
            $crate::HistogramOpts::new($NAME, $HELP)
        }
    };

    ( $ NAME : expr , $ HELP : expr , $ BUCKETS : expr ) => {
        {
            let hopts = histogram_opts!($NAME, $HELP);
            hopts.buckets($BUCKETS)
        }
    };

    ( $ NAME : expr , $ HELP : expr , $ BUCKETS : expr , $ CONST_LABELS : expr ) => {
        {
            let hopts = histogram_opts!($NAME, $HELP, $BUCKETS);
            hopts.const_labels($CONST_LABELS)
        }
    };
}

/// Create a `Counter` and register to default registry.
///
/// # Examples
///
/// ```
/// # #[macro_use] extern crate prometheus;
/// # fn main() {
/// let opts = opts!("test_macro_counter_1", "help");
/// let res1 = register_counter!(opts);
/// assert!(res1.is_ok());
///
/// let res2 = register_counter!("test_macro_counter_2", "help");
/// assert!(res2.is_ok());
/// # }
/// ```
#[macro_export]
macro_rules! register_counter {
    ( $ NAME : expr , $ HELP : expr ) => {
        register_counter!(opts!($NAME, $HELP))
    };

    ( $ OPTS : expr ) => {
        {
            let counter = $crate::Counter::with_opts($OPTS).unwrap();
            $crate::register(Box::new(counter.clone())).map(|_| counter)
        }
    }
}

/// Create a `CounterVec` and register to default registry.
///
/// # Examples
///
/// ```
/// # #[macro_use] extern crate prometheus;
/// # fn main() {
/// let opts = opts!("test_macro_counter_vec_1", "help");
/// let counter_vec = register_counter_vec!(opts, &["a", "b"]);
/// assert!(counter_vec.is_ok());
///
/// let counter_vec = register_counter_vec!("test_macro_counter_vec_2", "help", &["a", "b"]);
/// assert!(counter_vec.is_ok());
/// # }
/// ```
#[macro_export]
macro_rules! register_counter_vec {
    ( $ OPTS : expr , $ LABELS_NAMES : expr ) => {
        {
            let counter_vec = $crate::CounterVec::new($OPTS, $LABELS_NAMES).unwrap();
            $crate::register(Box::new(counter_vec.clone())).map(|_| counter_vec)
        }
    };

    ( $ NAME : expr , $ HELP : expr , $ LABELS_NAMES : expr ) => {
        {
            register_counter_vec!(opts!($NAME, $HELP), $LABELS_NAMES)
        }
    };
}

/// Create a `Gauge` and register to default registry.
///
/// # Examples
///
/// ```
/// # #[macro_use] extern crate prometheus;
/// # fn main() {
/// let opts = opts!("test_macro_gauge", "help");
/// let res1 = register_gauge!(opts);
/// assert!(res1.is_ok());
///
/// let res2 = register_gauge!("test_macro_gauge_2", "help");
/// assert!(res2.is_ok());
/// # }
/// ```
#[macro_export]
macro_rules! register_gauge {
    ( $ NAME : expr , $ HELP : expr ) => {
        register_gauge!(opts!($NAME, $HELP))
    };

    ( $ OPTS : expr ) => {
        {
            let gauge = $crate::Gauge::with_opts($OPTS).unwrap();
            $crate::register(Box::new(gauge.clone())).map(|_| gauge)
        }
    }
}

/// Create a `GaugeVec` and register to default registry.
///
/// # Examples
///
/// ```
/// # #[macro_use] extern crate prometheus;
/// # fn main() {
/// let opts = opts!("test_macro_gauge_vec_1", "help");
/// let gauge_vec = register_gauge_vec!(opts, &["a", "b"]);
/// assert!(gauge_vec.is_ok());
///
/// let gauge_vec = register_gauge_vec!("test_macro_gauge_vec_2", "help", &["a", "b"]);
/// assert!(gauge_vec.is_ok());
/// # }
/// ```
#[macro_export]
macro_rules! register_gauge_vec {
    ( $ OPTS : expr , $ LABELS_NAMES : expr ) => {
        {
            let gauge_vec = $crate::GaugeVec::new($OPTS, $LABELS_NAMES).unwrap();
            $crate::register(Box::new(gauge_vec.clone())).map(|_| gauge_vec)
        }
    };

    ( $ NAME : expr , $ HELP : expr , $ LABELS_NAMES : expr ) => {
        {
            register_gauge_vec!(opts!($NAME, $HELP), $LABELS_NAMES)
        }
    };
}

/// Create a `Histogram` and register to default registry.
///
/// # Examples
///
/// ```
/// # #[macro_use] extern crate prometheus;
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
#[macro_export]
macro_rules! register_histogram {
    ( $ NAME : expr , $ HELP : expr ) => {
        register_histogram!(histogram_opts!($NAME, $HELP))
    };

    ( $ NAME : expr , $ HELP : expr , $ BUCKETS : expr ) => {
        register_histogram!(histogram_opts!($NAME, $HELP, $BUCKETS))
    };

    ( $ HOPTS : expr ) => {
        {
            let histogram = $crate::Histogram::with_opts($HOPTS).unwrap();
            $crate::register(Box::new(histogram.clone())).map(|_| histogram)
        }
    }
}

/// Create a `HistogramVec` and register to default registry.
///
/// # Examples
///
/// ```
/// # #[macro_use] extern crate prometheus;
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
#[macro_export]
macro_rules! register_histogram_vec {
    ( $ HOPTS : expr , $ LABELS_NAMES : expr ) => {
        {
            let histogram_vec = $crate::HistogramVec::new($HOPTS, $LABELS_NAMES).unwrap();
            $crate::register(Box::new(histogram_vec.clone())).map(|_| histogram_vec)
        }
    };

    ( $ NAME : expr , $ HELP : expr , $ LABELS_NAMES : expr ) => {
        {
            register_histogram_vec!(histogram_opts!($NAME, $HELP), $LABELS_NAMES)
        }
    };

    ( $ NAME : expr , $ HELP : expr , $ LABELS_NAMES : expr , $ BUCKETS : expr) => {
        {
            register_histogram_vec!(histogram_opts!($NAME, $HELP, $BUCKETS), $LABELS_NAMES)
        }
    };
}
