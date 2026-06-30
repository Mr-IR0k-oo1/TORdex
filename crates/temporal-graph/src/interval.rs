use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// A time interval with an optional end (None = unbounded / still active).
///
/// Intervals are **immutable** — every operation returns a new `TimeInterval`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeInterval {
    /// Start of the interval (inclusive).
    pub start: OffsetDateTime,
    /// End of the interval (exclusive). `None` means unbounded.
    pub end: Option<OffsetDateTime>,
}

/// A far-future sentinel used internally for unbounded interval comparison.
const FAR_FUTURE: time::Duration = time::Duration::days(365_000);

impl TimeInterval {
    /// Create an interval starting at `start` with no end (still active).
    #[must_use]
    pub fn starting_at(start: OffsetDateTime) -> Self {
        Self { start, end: None }
    }

    /// Create a bounded interval from `start` to `end`.
    #[must_use]
    pub fn bounded(start: OffsetDateTime, end: OffsetDateTime) -> Self {
        Self {
            start,
            end: Some(end),
        }
    }

    /// Check if the interval contains a given timestamp.
    #[must_use]
    pub fn contains(&self, t: OffsetDateTime) -> bool {
        t >= self.start && self.end.map_or(true, |end| t < end)
    }

    fn end_or_far(&self) -> OffsetDateTime {
        self.end.unwrap_or_else(|| OffsetDateTime::now_utc() + FAR_FUTURE)
    }

    /// Check if this interval overlaps with another.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        let self_end = self.end_or_far();
        let other_end = other.end_or_far();
        self.start < other_end && other.start < self_end
    }

    /// Duration of the interval. Returns `None` if unbounded.
    #[must_use]
    pub fn duration(&self) -> Option<time::Duration> {
        self.end.map(|end| end - self.start)
    }

    /// Intersection of two intervals.
    #[must_use]
    pub fn intersection(&self, other: &Self) -> Option<Self> {
        let start = self.start.max(other.start);
        let self_end = self.end_or_far();
        let other_end = other.end_or_far();
        let end = self_end.min(other_end);
        let far = OffsetDateTime::now_utc() + FAR_FUTURE;
        if start < end {
            Some(Self {
                start,
                end: if end >= far { None } else { Some(end) },
            })
        } else {
            None
        }
    }

    /// Check if this interval is before another (this ends before other starts).
    #[must_use]
    pub fn before(&self, other: &Self) -> bool {
        self.end.map_or(false, |end| other.start >= end)
    }

    /// Check if this interval is after another (this starts after other ends).
    #[must_use]
    pub fn after(&self, other: &Self) -> bool {
        other.end.map_or(false, |end| self.start >= end)
    }

    /// Check if this interval fully contains another.
    #[must_use]
    pub fn contains_interval(&self, other: &Self) -> bool {
        self.start <= other.start
            && self.end.map_or(true, |self_end| {
                other.end.map_or(false, |other_end| self_end >= other_end)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn unbounded_contains_all_after_start() {
        let iv = TimeInterval::starting_at(datetime!(2024-01-01 0:00 UTC));
        assert!(iv.contains(datetime!(2024-06-15 0:00 UTC)));
        assert!(!iv.contains(datetime!(2023-12-31 23:59 UTC)));
    }

    #[test]
    fn bounded_interval() {
        let iv = TimeInterval::bounded(
            datetime!(2024-01-01 0:00 UTC),
            datetime!(2024-12-31 23:59 UTC),
        );
        assert!(iv.contains(datetime!(2024-06-15 0:00 UTC)));
        assert!(!iv.contains(datetime!(2025-01-01 0:00 UTC)));
    }

    #[test]
    fn overlap_detection() {
        let a = TimeInterval::bounded(
            datetime!(2024-01-01 0:00 UTC),
            datetime!(2024-06-30 0:00 UTC),
        );
        let b = TimeInterval::bounded(
            datetime!(2024-03-01 0:00 UTC),
            datetime!(2024-09-30 0:00 UTC),
        );
        assert!(a.overlaps(&b));
    }

    #[test]
    fn no_overlap() {
        let a = TimeInterval::bounded(
            datetime!(2024-01-01 0:00 UTC),
            datetime!(2024-03-01 0:00 UTC),
        );
        let b = TimeInterval::bounded(
            datetime!(2024-04-01 0:00 UTC),
            datetime!(2024-06-01 0:00 UTC),
        );
        assert!(!a.overlaps(&b));
        assert!(a.before(&b));
        assert!(b.after(&a));
    }

    #[test]
    fn intersection_returns_common() {
        let a = TimeInterval::bounded(
            datetime!(2024-01-01 0:00 UTC),
            datetime!(2024-06-30 0:00 UTC),
        );
        let b = TimeInterval::bounded(
            datetime!(2024-03-01 0:00 UTC),
            datetime!(2024-09-30 0:00 UTC),
        );
        let inter = a.intersection(&b).unwrap();
        assert_eq!(inter.start, datetime!(2024-03-01 0:00 UTC));
    }

    #[test]
    fn duration_unbounded_returns_none() {
        let iv = TimeInterval::starting_at(datetime!(2024-01-01 0:00 UTC));
        assert!(iv.duration().is_none());
    }

    #[test]
    fn duration_bounded() {
        let iv = TimeInterval::bounded(
            datetime!(2024-01-01 0:00 UTC),
            datetime!(2024-01-02 0:00 UTC),
        );
        assert_eq!(iv.duration(), Some(time::Duration::days(1)));
    }
}
