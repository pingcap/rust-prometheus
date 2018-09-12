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

use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::From;
use std::sync::Arc;
use std::time::{Duration, Instant as StdInstant};

use atomic64::{Atomic, AtomicF64, AtomicU64};
use desc::{Desc, Describer};
use errors::{Error, Result};
use metrics::{Collector, Metric, Opts};
use proto;
use protobuf::RepeatedField;
use value::make_label_pairs;
use vec::{MetricVec, MetricVecBuilder};

/// The default [`Histogram`](::Histogram) buckets. The default buckets are
/// tailored to broadly measure the response time (in seconds) of a
/// network service. Most likely, however, you will be required to define
/// buckets customized to your use case.
pub const DEFAULT_BUCKETS: &[f64; 11] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Used for the label that defines the upper bound of a
/// bucket of a histogram ("le" -> "less or equal").
pub const BUCKET_LABEL: &str = "le";

#[inline]
fn check_bucket_lable(label: &str) -> Result<()> {
    if label == BUCKET_LABEL {
        return Err(Error::Msg(
            "`le` is not allowed as label name in histograms".to_owned(),
        ));
    }

    Ok(())
}

fn check_and_adjust_buckets(mut buckets: Vec<f64>) -> Result<Vec<f64>> {
    if buckets.is_empty() {
        buckets = Vec::from(DEFAULT_BUCKETS as &'static [f64]);
    }

    for (i, upper_bound) in buckets.iter().enumerate() {
        if i < (buckets.len() - 1) && *upper_bound >= buckets[i + 1] {
            return Err(Error::Msg(format!(
                "histogram buckets must be in increasing \
                 order: {} >= {}",
                upper_bound,
                buckets[i + 1]
            )));
        }
    }

    let tail = *buckets.last().unwrap();
    if tail.is_sign_positive() && tail.is_infinite() {
        // The +Inf bucket is implicit. Remove it here.
        buckets.pop();
    }

    Ok(buckets)
}

/// A struct that bundles the options for creating a [`Histogram`](::Histogram) metric. It is
/// mandatory to set Name and Help to a non-empty string. All other fields are
/// optional and can safely be left at their zero value.
#[derive(Clone)]
pub struct HistogramOpts {
    pub common_opts: Opts,

    // Defines the buckets into which observations are counted. Each
    // element in the slice is the upper inclusive bound of a bucket. The
    // values must be sorted in strictly increasing order. There is no need
    // to add a highest bucket with +Inf bound, it will be added
    // implicitly. The default value is DefBuckets.
    pub buckets: Vec<f64>,
}

impl HistogramOpts {
    /// Create a [`HistogramOpts`](::HistogramOpts) with the `name` and `help` arguments.
    pub fn new<S: Into<String>>(name: S, help: S) -> HistogramOpts {
        HistogramOpts {
            common_opts: Opts::new(name, help),
            buckets: Vec::from(DEFAULT_BUCKETS as &'static [f64]),
        }
    }

    /// `namespace` sets the namespace.
    pub fn namespace<S: Into<String>>(mut self, namesapce: S) -> Self {
        self.common_opts.namespace = namesapce.into();
        self
    }

    /// `subsystem` sets the sub system.
    pub fn subsystem<S: Into<String>>(mut self, subsystem: S) -> Self {
        self.common_opts.subsystem = subsystem.into();
        self
    }

    /// `const_labels` sets the const labels.
    pub fn const_labels(mut self, const_labels: HashMap<String, String>) -> Self {
        self.common_opts = self.common_opts.const_labels(const_labels);
        self
    }

    /// `const_label` adds a const label.
    pub fn const_label<S: Into<String>>(mut self, name: S, value: S) -> Self {
        self.common_opts = self.common_opts.const_label(name, value);
        self
    }

    /// `variable_labels` sets the variable labels.
    pub fn variable_labels(mut self, variable_labels: Vec<String>) -> Self {
        self.common_opts = self.common_opts.variable_labels(variable_labels);
        self
    }

    /// `variable_label` adds a variable label.
    pub fn variable_label<S: Into<String>>(mut self, name: S) -> Self {
        self.common_opts = self.common_opts.variable_label(name);
        self
    }

    /// `fq_name` returns the fq_name.
    pub fn fq_name(&self) -> String {
        self.common_opts.fq_name()
    }

    /// `buckets` set the buckets.
    pub fn buckets(mut self, buckets: Vec<f64>) -> Self {
        self.buckets = buckets;
        self
    }
}

impl Describer for HistogramOpts {
    fn describe(&self) -> Result<Desc> {
        self.common_opts.describe()
    }
}

impl From<Opts> for HistogramOpts {
    fn from(opts: Opts) -> HistogramOpts {
        HistogramOpts {
            common_opts: opts,
            buckets: Vec::from(DEFAULT_BUCKETS as &'static [f64]),
        }
    }
}

pub struct HistogramCore {
    desc: Desc,
    label_pairs: Vec<proto::LabelPair>,

    sum: AtomicF64,
    count: AtomicU64,

    upper_bounds: Vec<f64>,
    counts: Vec<AtomicU64>,
}

impl HistogramCore {
    pub fn new(opts: &HistogramOpts, label_values: &[&str]) -> Result<HistogramCore> {
        let desc = opts.describe()?;

        for name in &desc.variable_labels {
            check_bucket_lable(name)?;
        }
        for pair in &desc.const_label_pairs {
            check_bucket_lable(pair.get_name())?;
        }
        let pairs = make_label_pairs(&desc, label_values);

        let buckets = check_and_adjust_buckets(opts.buckets.clone())?;

        let mut counts = Vec::new();
        for _ in 0..buckets.len() {
            counts.push(AtomicU64::new(0));
        }

        Ok(HistogramCore {
            desc,
            label_pairs: pairs,
            sum: AtomicF64::new(0.0),
            count: AtomicU64::new(0),
            upper_bounds: buckets,
            counts,
        })
    }

    pub fn observe(&self, v: f64) {
        // Try find the bucket.
        let mut iter = self
            .upper_bounds
            .iter()
            .enumerate()
            .filter(|&(_, f)| v <= *f);
        if let Some((i, _)) = iter.next() {
            self.counts[i].inc_by(1);
        }

        self.count.inc_by(1);
        self.sum.inc_by(v);
    }

    pub fn proto(&self) -> proto::Histogram {
        let mut h = proto::Histogram::new();
        h.set_sample_sum(self.sum.get());
        h.set_sample_count(self.count.get() as u64);

        let mut count = 0;
        let mut buckets = Vec::with_capacity(self.upper_bounds.len());
        for (i, upper_bound) in self.upper_bounds.iter().enumerate() {
            count += self.counts[i].get();
            let mut b = proto::Bucket::new();
            b.set_cumulative_count(count as u64);
            b.set_upper_bound(*upper_bound);
            buckets.push(b);
        }
        h.set_bucket(RepeatedField::from_vec(buckets));

        h
    }
}

enum Instant {
    Monotonic(StdInstant),
    #[cfg(all(feature = "nightly", target_os = "linux"))]
    MonotonicCoarse(timespec),
}

impl Instant {
    fn now() -> Instant {
        Instant::Monotonic(StdInstant::now())
    }

    #[cfg(all(feature = "nightly", target_os = "linux"))]
    fn now_coarse() -> Instant {
        Instant::MonotonicCoarse(get_time_coarse())
    }

    #[cfg(all(feature = "nightly", not(target_os = "linux")))]
    fn now_coarse() -> Instant {
        Instant::Monotonic(StdInstant::now())
    }

    fn elapsed(&self) -> Duration {
        match *self {
            Instant::Monotonic(i) => i.elapsed(),

            // It is different from `Instant::Monotonic`, the resolution here is millisecond.
            // The processors in an SMP system do not start all at exactly the same time
            // and therefore the timer registers are typically running at an offset.
            // Use millisecond resolution for ignoring the error.
            // See more: https://linux.die.net/man/2/clock_gettime
            #[cfg(all(feature = "nightly", target_os = "linux"))]
            Instant::MonotonicCoarse(t) => {
                let now = get_time_coarse();
                let now_ms = now.tv_sec * MILLIS_PER_SEC + now.tv_nsec / NANOS_PER_MILLI;
                let t_ms = t.tv_sec * MILLIS_PER_SEC + t.tv_nsec / NANOS_PER_MILLI;
                let dur = now_ms - t_ms;
                if dur >= 0 {
                    Duration::from_millis(dur as u64)
                } else {
                    Duration::from_millis(0)
                }
            }
        }
    }
}

#[cfg(all(feature = "nightly", target_os = "linux"))]
use self::coarse::*;

#[cfg(all(feature = "nightly", target_os = "linux"))]
mod coarse {
    pub use libc::timespec;
    use libc::{clock_gettime, CLOCK_MONOTONIC_COARSE};

    pub const NANOS_PER_MILLI: i64 = 1_000_000;
    pub const MILLIS_PER_SEC: i64 = 1_000;

    pub fn get_time_coarse() -> timespec {
        let mut t = timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        assert_eq!(unsafe { clock_gettime(CLOCK_MONOTONIC_COARSE, &mut t) }, 0);
        t
    }
}

/// A struct represents an event being timed. When the timer goes out of
/// scope, the duration will be observed, or call `observe_duration` to manually
/// observe.
///
/// NOTICE: A timer can be observed only once (automatically or manually).
#[must_use = "Timer should be kept in a variable otherwise it cannot observe duration"]
pub struct HistogramTimer {
    histogram: Histogram,
    start: Instant,
}

impl HistogramTimer {
    fn new(histogram: Histogram) -> HistogramTimer {
        HistogramTimer {
            histogram,
            start: Instant::now(),
        }
    }

    #[cfg(feature = "nightly")]
    fn new_coarse(histogram: Histogram) -> HistogramTimer {
        HistogramTimer {
            histogram,
            start: Instant::now_coarse(),
        }
    }

    /// `observe_duration` observes the amount of time in seconds since
    /// `Histogram.start_timer` was called.
    pub fn observe_duration(self) {
        drop(self);
    }

    fn observe(&mut self) {
        let v = duration_to_seconds(self.start.elapsed());
        self.histogram.observe(v)
    }
}

impl Drop for HistogramTimer {
    fn drop(&mut self) {
        self.observe();
    }
}

/// A [`Metric`](::core::Metric) counts individual observations from an event or sample stream in
/// configurable buckets. Similar to a summary, it also provides a sum of
/// observations and an observation count.
///
/// On the Prometheus server, quantiles can be calculated from a [`Histogram`](::Histogram) using
/// the `histogram_quantile` function in the query language.
///
/// Note that Histograms, in contrast to Summaries, can be aggregated with the
/// Prometheus query language (see the documentation for detailed
/// procedures). However, Histograms require the user to pre-define suitable
/// buckets, and they are in general less accurate. The Observe method of a
/// [`Histogram`](::Histogram) has a very low performance overhead in comparison with the Observe
/// method of a Summary.
#[derive(Clone)]
pub struct Histogram {
    core: Arc<HistogramCore>,
}

impl Histogram {
    /// `with_opts` creates a [`Histogram`](::Histogram) with the `opts` options.
    pub fn with_opts(opts: HistogramOpts) -> Result<Histogram> {
        Histogram::with_opts_and_label_values(&opts, &[])
    }

    fn with_opts_and_label_values(
        opts: &HistogramOpts,
        label_values: &[&str],
    ) -> Result<Histogram> {
        let core = HistogramCore::new(opts, label_values)?;

        Ok(Histogram {
            core: Arc::new(core),
        })
    }
}

impl Histogram {
    /// Add a single observation to the [`Histogram`](::Histogram).
    pub fn observe(&self, v: f64) {
        self.core.observe(v)
    }

    /// Return a [`HistogramTimer`](::HistogramTimer) to track a duration.
    pub fn start_timer(&self) -> HistogramTimer {
        HistogramTimer::new(self.clone())
    }

    /// Return a [`HistogramTimer`](::HistogramTimer) to track a duration.
    /// It is faster but less precise.
    #[cfg(feature = "nightly")]
    pub fn start_coarse_timer(&self) -> HistogramTimer {
        HistogramTimer::new_coarse(self.clone())
    }

    /// Return a [`LocalHistogram`](::local::LocalHistogram) for single thread usage.
    pub fn local(&self) -> LocalHistogram {
        LocalHistogram::new(self.clone())
    }
}

impl Metric for Histogram {
    fn metric(&self) -> proto::Metric {
        let mut m = proto::Metric::new();
        m.set_label(RepeatedField::from_vec(self.core.label_pairs.clone()));

        let h = self.core.proto();
        m.set_histogram(h);

        m
    }
}

impl Collector for Histogram {
    fn desc(&self) -> Vec<&Desc> {
        vec![&self.core.desc]
    }

    fn collect(&self) -> Vec<proto::MetricFamily> {
        let mut m = proto::MetricFamily::new();
        m.set_name(self.core.desc.fq_name.clone());
        m.set_help(self.core.desc.help.clone());
        m.set_field_type(proto::MetricType::HISTOGRAM);
        m.set_metric(RepeatedField::from_vec(vec![self.metric()]));

        vec![m]
    }
}

#[derive(Clone)]
pub struct HistogramVecBuilder {}

impl MetricVecBuilder for HistogramVecBuilder {
    type M = Histogram;
    type P = HistogramOpts;

    fn build(&self, opts: &HistogramOpts, vals: &[&str]) -> Result<Histogram> {
        Histogram::with_opts_and_label_values(opts, vals)
    }
}

/// A [`Collector`](::core::Collector) that bundles a set of Histograms that all share the
/// same [`Desc`](::core::Desc), but have different values for their variable labels. This is used
/// if you want to count the same thing partitioned by various dimensions
/// (e.g. HTTP request latencies, partitioned by status code and method).
pub type HistogramVec = MetricVec<HistogramVecBuilder>;

impl HistogramVec {
    /// Create a new [`HistogramVec`](::HistogramVec) based on the provided
    /// [`HistogramOpts`](::HistogramOpts) and partitioned by the given label names. At least
    /// one label name must be provided.
    pub fn new(opts: HistogramOpts, label_names: &[&str]) -> Result<HistogramVec> {
        let variable_names = label_names.iter().map(|s| (*s).to_owned()).collect();
        let opts = opts.variable_labels(variable_names);
        let metric_vec =
            MetricVec::create(proto::MetricType::HISTOGRAM, HistogramVecBuilder {}, opts)?;

        Ok(metric_vec as HistogramVec)
    }

    /// Return a `LocalHistogramVec` for single thread usage.
    pub fn local(&self) -> LocalHistogramVec {
        let vec = self.clone();
        LocalHistogramVec::new(vec)
    }
}

/// Create `count` buckets, each `width` wide, where the lowest
/// bucket has an upper bound of `start`. The final +Inf bucket is not counted
/// and not included in the returned slice. The returned slice is meant to be
/// used for the Buckets field of [`HistogramOpts`](::HistogramOpts).
///
/// The function returns an error if `count` is zero or `width` is zero or
/// negative.
pub fn linear_buckets(start: f64, width: f64, count: usize) -> Result<Vec<f64>> {
    if count < 1 {
        return Err(Error::Msg(format!(
            "LinearBuckets needs a positive count, count: {}",
            count
        )));
    }
    if width <= 0.0 {
        return Err(Error::Msg(format!(
            "LinearBuckets needs a width greater then 0, width: {}",
            width
        )));
    }

    let mut next = start;
    let mut buckets = Vec::with_capacity(count);
    for _ in 0..count {
        buckets.push(next);
        next += width;
    }

    Ok(buckets)
}

/// Create `count` buckets, where the lowest bucket has an
/// upper bound of `start` and each following bucket's upper bound is `factor`
/// times the previous bucket's upper bound. The final +Inf bucket is not counted
/// and not included in the returned slice. The returned slice is meant to be
/// used for the Buckets field of [`HistogramOpts`](::HistogramOpts).
///
/// The function returns an error if `count` is zero, if `start` is zero or
/// negative, or if `factor` is less than or equal 1.
pub fn exponential_buckets(start: f64, factor: f64, count: usize) -> Result<Vec<f64>> {
    if count < 1 {
        return Err(Error::Msg(format!(
            "exponential_buckets needs a positive count, count: {}",
            count
        )));
    }
    if start <= 0.0 {
        return Err(Error::Msg(format!(
            "exponential_buckets needs a positive start value, \
             start: {}",
            start
        )));
    }
    if factor <= 1.0 {
        return Err(Error::Msg(format!(
            "exponential_buckets needs a factor greater than 1, \
             factor: {}",
            factor
        )));
    }

    let mut next = start;
    let mut buckets = Vec::with_capacity(count);
    for _ in 0..count {
        buckets.push(next);
        next *= factor;
    }

    Ok(buckets)
}

/// `duration_to_seconds` converts Duration to seconds.
#[inline]
fn duration_to_seconds(d: Duration) -> f64 {
    let nanos = f64::from(d.subsec_nanos()) / 1e9;
    d.as_secs() as f64 + nanos
}

#[derive(Clone)]
pub struct LocalHistogramCore {
    histogram: Histogram,
    counts: Vec<u64>,
    count: u64,
    sum: f64,
}

/// An unsync [`Histogram`](::Histogram).
pub struct LocalHistogram {
    core: RefCell<LocalHistogramCore>,
}

impl Clone for LocalHistogram {
    fn clone(&self) -> LocalHistogram {
        let core = self.core.clone();
        let lh = LocalHistogram { core };
        lh.clear();
        lh
    }
}

/// An unsync [`HistogramTimer`](::HistogramTimer).
#[must_use = "Timer should be kept in a variable otherwise it cannot observe duration"]
pub struct LocalHistogramTimer {
    local: LocalHistogram,
    start: Instant,
}

impl LocalHistogramTimer {
    /// `observe_duration` observes the amount of time in seconds since
    /// `LocalHistogram.start_timer` was called.
    pub fn observe_duration(self) {
        drop(self);
    }

    fn observe(&mut self) {
        let v = duration_to_seconds(self.start.elapsed());
        self.local.observe(v)
    }
}

impl Drop for LocalHistogramTimer {
    fn drop(&mut self) {
        self.observe()
    }
}

impl LocalHistogramCore {
    fn new(histogram: Histogram) -> LocalHistogramCore {
        let counts = vec![0; histogram.core.counts.len()];

        LocalHistogramCore {
            histogram,
            counts,
            count: 0,
            sum: 0.0,
        }
    }

    pub fn observe(&mut self, v: f64) {
        // Try find the bucket.
        let mut iter = self
            .histogram
            .core
            .upper_bounds
            .iter()
            .enumerate()
            .filter(|&(_, f)| v <= *f);
        if let Some((i, _)) = iter.next() {
            self.counts[i] += 1;
        }

        self.count += 1;
        self.sum += v;
    }

    pub fn clear(&mut self) {
        for v in &mut self.counts {
            *v = 0
        }

        self.count = 0;
        self.sum = 0.0;
    }

    pub fn flush(&mut self) {
        // No cached metric, return.
        if self.count == 0 {
            return;
        }

        {
            let h = &self.histogram;

            for (i, v) in self.counts.iter().enumerate() {
                if *v > 0 {
                    h.core.counts[i].inc_by(*v);
                }
            }

            h.core.count.inc_by(self.count);
            h.core.sum.inc_by(self.sum);
        }

        self.clear()
    }
}

impl LocalHistogram {
    fn new(histogram: Histogram) -> LocalHistogram {
        let core = LocalHistogramCore::new(histogram);
        LocalHistogram {
            core: RefCell::new(core),
        }
    }

    /// Add a single observation to the [`Histogram`](::Histogram).
    pub fn observe(&self, v: f64) {
        self.core.borrow_mut().observe(v);
    }

    /// Return a `LocalHistogramTimer` to track a duration.
    pub fn start_timer(&self) -> LocalHistogramTimer {
        LocalHistogramTimer {
            local: self.clone(),
            start: Instant::now(),
        }
    }

    /// Return a `LocalHistogramTimer` to track a duration.
    /// It is faster but less precise.
    #[cfg(feature = "nightly")]
    pub fn start_coarse_timer(&self) -> LocalHistogramTimer {
        LocalHistogramTimer {
            local: self.clone(),
            start: Instant::now_coarse(),
        }
    }

    /// Clear the local metric.
    pub fn clear(&self) {
        self.core.borrow_mut().clear();
    }

    /// Flush the local metrics to the [`Histogram`](::Histogram) metric.
    pub fn flush(&self) {
        self.core.borrow_mut().flush();
    }
}

impl Drop for LocalHistogram {
    fn drop(&mut self) {
        self.flush()
    }
}

/// An unsync [`HistogramVec`](::HistogramVec).
pub struct LocalHistogramVec {
    vec: HistogramVec,
    local: HashMap<u64, LocalHistogram>,
}

impl LocalHistogramVec {
    fn new(vec: HistogramVec) -> LocalHistogramVec {
        let local = HashMap::with_capacity(vec.v.children.read().len());
        LocalHistogramVec { vec, local }
    }

    /// Get a [`LocalHistogram`](::local::LocalHistogram) by label values.
    /// See more [MetricVec::with_label_values](::core::MetricVec::with_label_values).
    pub fn with_label_values<'a>(&'a mut self, vals: &[&str]) -> &'a LocalHistogram {
        let hash = self.vec.v.hash_label_values(vals).unwrap();
        let vec = &self.vec;
        self.local
            .entry(hash)
            .or_insert_with(|| vec.with_label_values(vals).local())
    }

    /// Remove a [`LocalHistogram`](::local::LocalHistogram) by label values.
    /// See more [MetricVec::remove_label_values](::core::MetricVec::remove_label_values).
    pub fn remove_label_values(&mut self, vals: &[&str]) -> Result<()> {
        let hash = self.vec.v.hash_label_values(vals)?;
        self.local.remove(&hash);
        self.vec.v.delete_label_values(vals)
    }

    /// Flush the local metrics to the [`HistogramVec`](::HistogramVec) metric.
    pub fn flush(&mut self) {
        for h in self.local.values() {
            h.flush();
        }
    }
}

impl Clone for LocalHistogramVec {
    fn clone(&self) -> LocalHistogramVec {
        LocalHistogramVec::new(self.vec.clone())
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use metrics::Collector;
    use metrics::Metric;
    use std::f64::{EPSILON, INFINITY};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_histogram() {
        let opts = HistogramOpts::new("test1", "test help")
            .const_label("a", "1")
            .const_label("b", "2");
        let histogram = Histogram::with_opts(opts).unwrap();
        histogram.observe(1.0);

        let timer = histogram.start_timer();
        thread::sleep(Duration::from_millis(100));
        timer.observe_duration();

        let timer = histogram.start_timer();
        let handler = thread::spawn(move || {
            let _timer = timer;
            thread::sleep(Duration::from_millis(400));
        });
        assert!(handler.join().is_ok());

        let mut mfs = histogram.collect();
        assert_eq!(mfs.len(), 1);

        let mf = mfs.pop().unwrap();
        let m = mf.get_metric().get(0).unwrap();
        assert_eq!(m.get_label().len(), 2);
        let proto_histogram = m.get_histogram();
        assert_eq!(proto_histogram.get_sample_count(), 3);
        assert!(proto_histogram.get_sample_sum() >= 1.5);
        assert_eq!(proto_histogram.get_bucket().len(), DEFAULT_BUCKETS.len());

        let buckets = vec![1.0, 2.0, 3.0];
        let opts = HistogramOpts::new("test2", "test help").buckets(buckets.clone());
        let histogram = Histogram::with_opts(opts).unwrap();
        let mut mfs = histogram.collect();
        assert_eq!(mfs.len(), 1);

        let mf = mfs.pop().unwrap();
        let m = mf.get_metric().get(0).unwrap();
        assert_eq!(m.get_label().len(), 0);
        let proto_histogram = m.get_histogram();
        assert_eq!(proto_histogram.get_sample_count(), 0);
        assert!((proto_histogram.get_sample_sum() - 0.0) < EPSILON);
        assert_eq!(proto_histogram.get_bucket().len(), buckets.len())
    }

    #[test]
    #[cfg(feature = "nightly")]
    fn test_histogram_coarse_timer() {
        let opts = HistogramOpts::new("test1", "test help");
        let histogram = Histogram::with_opts(opts).unwrap();

        let timer = histogram.start_coarse_timer();
        thread::sleep(Duration::from_millis(100));
        timer.observe_duration();

        let timer = histogram.start_coarse_timer();
        let handler = thread::spawn(move || {
            let _timer = timer;
            thread::sleep(Duration::from_millis(400));
        });
        assert!(handler.join().is_ok());

        let mut mfs = histogram.collect();
        assert_eq!(mfs.len(), 1);

        let mf = mfs.pop().unwrap();
        let m = mf.get_metric().get(0).unwrap();
        let proto_histogram = m.get_histogram();
        assert_eq!(proto_histogram.get_sample_count(), 2);
        assert!((proto_histogram.get_sample_sum() - 0.0) > EPSILON);
    }

    #[test]
    #[cfg(feature = "nightly")]
    fn test_instant_on_smp() {
        let zero = Duration::from_millis(0);
        for i in 0..100_000 {
            let now = Instant::now();
            let now_coarse = Instant::now_coarse();
            if i % 100 == 0 {
                thread::yield_now();
            }
            assert!(now.elapsed() >= zero);
            assert!(now_coarse.elapsed() >= zero);
        }
    }

    #[test]
    fn test_buckets_invalidation() {
        let table = vec![
            (vec![], true, DEFAULT_BUCKETS.len()),
            (vec![-2.0, -1.0, -0.5, 0.0, 0.5, 1.0, 2.0], true, 7),
            (vec![-2.0, -1.0, -0.5, 10.0, 0.5, 1.0, 2.0], false, 7),
            (vec![-2.0, -1.0, -0.5, 0.0, 0.5, 1.0, INFINITY], true, 6),
        ];

        for (buckets, is_ok, length) in table {
            let got = check_and_adjust_buckets(buckets);
            assert_eq!(got.is_ok(), is_ok);
            if is_ok {
                assert_eq!(got.unwrap().len(), length);
            }
        }
    }

    #[test]
    fn test_buckets_functions() {
        let linear_table = vec![
            (
                -15.0,
                5.0,
                6,
                true,
                vec![-15.0, -10.0, -5.0, 0.0, 5.0, 10.0],
            ),
            (-15.0, 0.0, 6, false, vec![]),
            (-15.0, 5.0, 0, false, vec![]),
        ];

        for (param1, param2, param3, is_ok, vec) in linear_table {
            let got = linear_buckets(param1, param2, param3);
            assert_eq!(got.is_ok(), is_ok);
            if got.is_ok() {
                assert_eq!(got.unwrap(), vec);
            }
        }

        let exponential_table = vec![
            (100.0, 1.2, 3, true, vec![100.0, 120.0, 144.0]),
            (100.0, 0.5, 3, false, vec![]),
            (100.0, 1.2, 0, false, vec![]),
        ];

        for (param1, param2, param3, is_ok, vec) in exponential_table {
            let got = exponential_buckets(param1, param2, param3);
            assert_eq!(got.is_ok(), is_ok);
            if got.is_ok() {
                assert_eq!(got.unwrap(), vec);
            }
        }
    }

    #[test]
    fn test_duration_to_seconds() {
        let tbls = vec![(1000, 1.0), (1100, 1.1), (100_111, 100.111)];
        for (millis, seconds) in tbls {
            let d = Duration::from_millis(millis);
            let v = duration_to_seconds(d);
            assert!((v - seconds).abs() < EPSILON);
        }
    }

    #[test]
    fn test_histogram_vec_with_label_values() {
        let vec = HistogramVec::new(
            HistogramOpts::new("test_histogram_vec", "test histogram vec help"),
            &["l1", "l2"],
        ).unwrap();

        assert!(vec.remove_label_values(&["v1", "v2"]).is_err());
        vec.with_label_values(&["v1", "v2"]).observe(1.0);
        assert!(vec.remove_label_values(&["v1", "v2"]).is_ok());

        assert!(vec.remove_label_values(&["v1"]).is_err());
        assert!(vec.remove_label_values(&["v1", "v3"]).is_err());
    }

    #[test]
    fn test_histogram_vec_with_opts_buckets() {
        let labels = ["l1", "l2"];
        let buckets = vec![1.0, 2.0, 3.0];
        let vec = HistogramVec::new(
            HistogramOpts::new("test_histogram_vec", "test histogram vec help")
                .buckets(buckets.clone()),
            &labels,
        ).unwrap();

        let histogram = vec.with_label_values(&["v1", "v2"]);
        histogram.observe(1.0);

        let m = histogram.metric();
        assert_eq!(m.get_label().len(), labels.len());

        let proto_histogram = m.get_histogram();
        assert_eq!(proto_histogram.get_sample_count(), 1);
        assert!((proto_histogram.get_sample_sum() - 1.0) < EPSILON);
        assert_eq!(proto_histogram.get_bucket().len(), buckets.len())
    }

    #[test]
    fn test_histogram_local() {
        let buckets = vec![1.0, 2.0, 3.0];
        let opts = HistogramOpts::new("test_histogram_local", "test histogram local help")
            .buckets(buckets.clone());
        let histogram = Histogram::with_opts(opts).unwrap();
        let local = histogram.local();

        let check = |count, sum| {
            let m = histogram.metric();
            let proto_histogram = m.get_histogram();
            assert_eq!(proto_histogram.get_sample_count(), count);
            assert!((proto_histogram.get_sample_sum() - sum) < EPSILON);
        };

        local.observe(1.0);
        local.observe(4.0);
        check(0, 0.0);

        local.flush();
        check(2, 5.0);

        local.observe(2.0);
        local.clear();
        check(2, 5.0);

        local.observe(2.0);
        drop(local);
        check(3, 7.0);
    }

    #[test]
    fn test_histogram_vec_local() {
        let vec = HistogramVec::new(
            HistogramOpts::new("test_histogram_vec_local", "test histogram vec help"),
            &["l1", "l2"],
        ).unwrap();
        let mut local_vec = vec.local();

        vec.remove_label_values(&["v1", "v2"]).unwrap_err();
        local_vec.remove_label_values(&["v1", "v2"]).unwrap_err();

        let check = |count, sum| {
            let ms = vec.collect()[0].take_metric();
            let proto_histogram = ms[0].get_histogram();
            assert_eq!(proto_histogram.get_sample_count(), count);
            assert!((proto_histogram.get_sample_sum() - sum) < EPSILON);
        };

        {
            // Flush LocalHistogram
            let h = local_vec.with_label_values(&["v1", "v2"]);
            h.observe(1.0);
            h.flush();
            check(1, 1.0);
        }

        {
            // Flush LocalHistogramVec
            local_vec.with_label_values(&["v1", "v2"]).observe(4.0);
            local_vec.flush();
            check(2, 5.0);
        }
        {
            // Reset ["v1", "v2"]
            local_vec.remove_label_values(&["v1", "v2"]).unwrap();

            // Flush on drop
            local_vec.with_label_values(&["v1", "v2"]).observe(2.0);
            drop(local_vec);
            check(1, 2.0);
        }
    }
}
