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

#[macro_export]
macro_rules! opts {
    ( $ NAME : expr , $ HELP : expr $ ( , $ LABELS : expr ) * ) => {
        {
            use std::collections::HashMap;

            let opts = $crate::Opts::new($NAME, $HELP);
            let lbs = HashMap::<String, String>::new();
            $(
                let mut lbs = lbs;
                lbs.extend($LABELS.iter().map(|(k, v)| ((*k).into(), (*v).into())));
            )*

            opts.const_labels(lbs)
        }
    }
}

#[macro_export]
macro_rules! histogram_opts {
    ( $ NAME : expr , $ HELP : expr , [ $ ( $ BUCKETS : expr ) , * ] ) => {
        {
            let his_opts = $crate::HistogramOpts::new($NAME, $HELP);

            let buckets = Vec::new();
            $(
                let mut buckets = buckets;
                buckets.extend($BUCKETS);
            )*;

            his_opts.buckets(buckets)
        }
    };

    ( $ NAME : expr , $ HELP : expr , $ LABELS : expr , [ $ ( $ BUCKETS : expr ) , + ] ) => {
        {
            use std::collections::HashMap;
            use std::iter::FromIterator;

            let his_opts = histogram_opts!($NAME, $HELP, [ $( $BUCKETS ), + ]);

            his_opts.const_labels(
                HashMap::from_iter($LABELS.iter().map(|(k, v)| ((*k).into(), (*v).into()))))
        }
    };

    ( $ NAME : expr , $ HELP : expr $ ( , $ LABELS : expr ) * ) => {
        {
            let opts = opts!($NAME, $HELP $(, $LABELS ) *);

            $crate::HistogramOpts::from(opts)
        }
    }
}

#[macro_export]
macro_rules! register_counter {
    ( $ NAME : expr , $ HELP : expr $ ( , $ LABELS : expr ) * ) => {
        register_counter!(opts!($NAME, $HELP $(, $LABELS)*))
    };

    ( $ OPTS : expr ) => {
        {
            let counter = $crate::Counter::with_opts($OPTS).unwrap();
            $crate::register(Box::new(counter.clone())).map(|_| counter)
        }
    }
}

#[macro_export]
macro_rules! register_gauge {
    ( $ NAME : expr , $ HELP : expr $ ( , $ LABELS : expr ) * ) => {
        register_gauge!(opts!($NAME, $HELP $(, $LABELS)*))
    };

    ( $ OPTS : expr ) => {
        {
            let gauge = $crate::Gauge::with_opts($OPTS).unwrap();
            $crate::register(Box::new(gauge.clone())).map(|_| gauge)
        }
    }
}

#[macro_export]
macro_rules! register_histogram {
    ( $ NAME : expr , $ HELP : expr ) => {
        register_histogram!(histogram_opts!($NAME, $HELP))
    };

    ( $ NAME : expr , $ HELP : expr , $ LABELS : expr ) => {
        register_histogram!(histogram_opts!($NAME, $HELP, $LABELS))
    };

    ( $ NAME : expr , $ HELP : expr , $ LABELS : expr , [ $ ( $ BUCKETS : expr ) , + ] ) => {
        register_histogram!(
            histogram_opts!($NAME, $HELP, $LABELS, [ $($BUCKETS), + ]))
    };

    ( $ OPTS : expr ) => {
        {
            let histogram = $crate::Histogram::with_opts($OPTS).unwrap();
            $crate::register(Box::new(histogram.clone())).map(|_| histogram)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use histogram::{linear_buckets, exponential_buckets};

    #[test]
    fn test_macro_labels() {
        let labels = labels!{
            "test" => "hello",
            "foo" => "bar",
        };
        assert_eq!(labels.len(), 2);
        assert!(labels.get("test").is_some());
        assert_eq!(*(labels.get("test").unwrap()), "hello");

        let labels: HashMap<&str, &str> = labels!{};
        assert!(labels.is_empty());
    }

    #[test]
    fn test_macro_opts() {
        let name = "test_opts";
        let help = "test opts help";

        let opts = opts!(name, help);
        assert_eq!(opts.name, name);
        assert_eq!(opts.help, help);

        let opts = opts!(name, help, labels!{"test" => "hello", "foo" => "bar",});
        assert_eq!(opts.const_labels.len(), 2);
        assert!(opts.const_labels.get("foo").is_some());
        assert_eq!(opts.const_labels.get("foo").unwrap(), "bar");

        let opts = opts!(name,
                         help,
                         labels!{"test" => "hello", "foo" => "bar",},
                         labels!{"ans" => "42",});
        assert_eq!(opts.const_labels.len(), 3);
        assert!(opts.const_labels.get("ans").is_some());
        assert_eq!(opts.const_labels.get("ans").unwrap(), "42");
    }

    #[test]
    fn test_macro_counter() {
        let opts = opts!("test_macro_counter_1",
                         "help",
                         labels!{"test" => "hello", "foo" => "bar",});

        let res1 = register_counter!(opts);
        assert!(res1.is_ok());

        let res2 = register_counter!("test_macro_counter_2", "help");
        assert!(res2.is_ok());

        let res3 = register_counter!("test_macro_counter_3", "help", labels!{ "a" => "b",});
        assert!(res3.is_ok());
    }

    #[test]
    fn test_macro_gauge() {
        let opts = opts!("test_macro_gauge",
                         "help",
                         labels!{"test" => "hello", "foo" => "bar",});

        let res1 = register_gauge!(opts);
        assert!(res1.is_ok());

        let res2 = register_gauge!("test_macro_gauge_2", "help");
        assert!(res2.is_ok());

        let res3 = register_gauge!("test_macro_gauge_3", "help", labels!{"a" => "b",});
        assert!(res3.is_ok());
    }

    #[test]
    fn test_macro_histogram_opts() {
        let name = "test_histogram_opts";
        let help = "test opts help";

        let opts = histogram_opts!(name, help);
        assert_eq!(opts.common_opts.name, name);
        assert_eq!(opts.common_opts.help, help);

        let opts = histogram_opts!(name, help, labels!{"test" => "hello", "foo" => "bar",});
        assert_eq!(opts.common_opts.const_labels.len(), 2);
        assert!(opts.common_opts.const_labels.get("foo").is_some());
        assert_eq!(opts.common_opts.const_labels.get("foo").unwrap(), "bar");

        let opts = histogram_opts!(name, help, labels!{"test" => "hello", "foo" => "bar",});
        assert_eq!(opts.common_opts.const_labels.len(), 2);
        assert!(opts.common_opts.const_labels.get("test").is_some());
        assert_eq!(opts.common_opts.const_labels.get("test").unwrap(), "hello");

        let opts = histogram_opts!(name, help, []);
        assert_eq!(opts.buckets.len(), 0);

        let opts = histogram_opts!(name, help, [Vec::from(&[1.0, 2.0] as &[f64])]);
        assert_eq!(opts.buckets.len(), 2);

        let opts = histogram_opts!(name,
                                   help,
                                   labels!{"a" => "c",},
                                   [Vec::from(&[1.0, 2.0] as &[f64]), Vec::from(&[3.0] as &[f64])]);
        assert_eq!(opts.buckets.len(), 3);

        let opts = histogram_opts!(name,
                                   help,
                                   labels!{"a" => "c",},
                                   [linear_buckets(1.0, 0.5, 4).unwrap(),
                                    exponential_buckets(4.0, 1.1, 4).unwrap()]);
        assert_eq!(opts.buckets.len(), 8);
    }

    #[test]
    fn test_macro_histogram() {
        let opts = histogram_opts!("test_macro_histogram",
                                   "help",
                                   labels!{"test" => "hello", "foo" => "bar",});

        let res1 = register_histogram!(opts);
        assert!(res1.is_ok());

        let res2 = register_histogram!("test_macro_histogram_2", "help");
        assert!(res2.is_ok());

        let res3 = register_histogram!("test_macro_histogram_3", "help", labels!{"a" => "b",});
        assert!(res3.is_ok());

        let res4 = register_histogram!("test_macro_histogram_4",
                                       "help",
                                       labels!{"a" => "b",},
                                       [Vec::from(&[1.0, 2.0] as &[f64])]);
        assert!(res4.is_ok());
    }
}
