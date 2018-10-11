// Copyright 2014 The Prometheus Authors
// Copyright 2018 PingCAP, Inc.
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

use std::cmp::*;
use std::ops::*;

#[cfg(not(feature = "nightly"))]
mod fallback;
#[cfg(not(feature = "nightly"))]
pub use self::fallback::{AtomicF64, AtomicI64, AtomicU64};

#[cfg(feature = "nightly")]
mod nightly;
#[cfg(feature = "nightly")]
pub use self::nightly::{AtomicF64, AtomicI64, AtomicU64};

/// An interface for numbers. Used to generically model float metrics and integer metrics, i.e.
/// [`Counter`](::Counter) and [`IntCounter`](::IntCounter).
pub trait Number:
    Sized + AddAssign + SubAssign + PartialOrd + PartialEq + Copy + Send + Sync
{
    /// `std::convert::From<i64> for f64` is not implemented, so that we need to implement our own.
    fn from_i64(v: i64) -> Self;
    /// Convert to a f64.
    fn into_f64(self) -> f64;
}

impl Number for i64 {
    #[inline]
    fn from_i64(v: i64) -> Self {
        v
    }

    #[inline]
    fn into_f64(self) -> f64 {
        self as f64
    }
}

impl Number for u64 {
    #[inline]
    fn from_i64(v: i64) -> Self {
        v as u64
    }

    #[inline]
    fn into_f64(self) -> f64 {
        self as f64
    }
}

impl Number for f64 {
    #[inline]
    fn from_i64(v: i64) -> Self {
        v as f64
    }

    #[inline]
    fn into_f64(self) -> f64 {
        self
    }
}

/// An interface for atomics. Used to generically model float metrics and integer metrics, i.e.
/// [`Counter`](::Counter) and [`IntCounter`](::IntCounter).
pub trait Atomic: Send + Sync {
    /// The numeric type associated with this atomic.
    type T: Number;
    /// Create a new atomic value.
    fn new(val: Self::T) -> Self;
    /// Set the value to the provided value.
    fn set(&self, val: Self::T);
    /// Get the value.
    fn get(&self) -> Self::T;
    /// Increment the value by a given amount.
    fn inc_by(&self, delta: Self::T);
    /// Decrement the value by a given amount.
    fn dec_by(&self, delta: Self::T);
}

#[cfg(test)]
mod test {
    use super::*;
    use std::f64::consts::PI;
    use std::f64::{self, EPSILON};

    #[test]
    fn test_atomic_f64() {
        let table: Vec<f64> = vec![0.0, 1.0, PI, f64::MIN, f64::MAX];

        for f in table {
            assert!((f - AtomicF64::new(f).get()).abs() < EPSILON);
        }
    }

    #[test]
    fn test_atomic_i64() {
        let ai64 = AtomicI64::new(0);
        assert_eq!(ai64.get(), 0);

        ai64.inc_by(1);
        assert_eq!(ai64.get(), 1);

        ai64.inc_by(-5);
        assert_eq!(ai64.get(), -4);
    }

    #[test]
    fn test_atomic_u64() {
        let au64 = AtomicU64::new(0);
        assert_eq!(au64.get(), 0);

        au64.inc_by(123);
        assert_eq!(au64.get(), 123);
    }
}
