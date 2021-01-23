//! Routines for working with probability.

use ordered_float::OrderedFloat;
use std::{fmt, ops::Mul, slice};

/// A probability, represented as negative log probability. This makes it
/// trivial to describe highly improbable events without underflowing a `f64`.
///
/// This also means that we can't represent a probability of 0, but that's OK,
/// because in a naive Bayesian world, a probability of 0 means "This is
/// absolutely impossible and no amount of evidence can convince me otherwise."
#[derive(Clone, Copy, PartialEq)]
pub struct Prob(f64);

impl Prob {
    /// The probably of an event which always happens.
    pub fn always() -> Self {
        Prob(0.0)
    }

    /// Construct a probability from `num / denom`.
    pub fn from_fraction(num: u64, denom: u64) -> Self {
        Self(-f64::ln(num as f64 / denom as f64))
    }

    /// Convert from a 64-bit number, typically coming from an `fst::Map`.
    pub fn from_bits(bits: u64) -> Self {
        Self(f64::from_bits(bits))
    }

    // Convert to a 64-bit number for storage in an `fst::Map`.
    pub fn to_bits(self) -> u64 {
        self.0.to_bits()
    }
}

impl fmt::Debug for Prob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for Prob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Mul for Prob {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl PartialOrd for Prob {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // Flip order of comparison because we use negative log probability.
        other.0.partial_cmp(&self.0)
    }
}

/// A probability distribution. The empty probability distribution represents an
/// impossible world, and it should trigger backtracking. The probabilities may
/// sum to a number between 0.0 and 1.0, exclusive. In this case, they should be
/// normalized.
#[derive(Debug)]
pub struct Dist<T>(Vec<(Prob, T)>);

impl<T> Dist<T> {
    /// Construct a distribution from a vector of events.
    pub fn from_vec(v: Vec<(Prob, T)>) -> Self {
        Dist(v)
    }

    /// Sort a probability distribution in order of descending probability.
    pub fn sort_by_probability(&mut self) {
        self.0.sort_by_key(|(p, _)| OrderedFloat(p.0));
    }
}

impl<T: fmt::Display> fmt::Display for Dist<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (p, v) in &self.0 {
            writeln!(f, "{:6.2} {}", p, v)?;
        }
        Ok(())
    }
}

impl<'a, T> IntoIterator for &'a Dist<T> {
    type Item = (Prob, &'a T);

    type IntoIter = DistIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        DistIter((&self.0).into_iter())
    }
}

pub struct DistIter<'a, T>(slice::Iter<'a, (Prob, T)>);

impl<'a, T> Iterator for DistIter<'a, T> {
    type Item = (Prob, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|&(prob, ref val)| (prob, val))
    }
}
