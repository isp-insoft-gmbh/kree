//! Facade over the cron backend.
//!
//! kree needs exactly two things from a cron library: parse a standard
//! 5-field expression, and find the next occurrence strictly after a given
//! instant. Both live behind [`Schedule`], so `croner` is named in this file
//! and nowhere else.
//!
//! The golden tests at the bottom pin the *semantics* we depend on rather
//! than the API we happen to call. That is the point of the facade: a
//! backend swap, or a major version bump of the current backend, is only
//! verifiable if something asserts that `*/30 9-17 * * 1-5` still means what
//! we think it means.

use std::str::FromStr;

use chrono::{DateTime, Local};
use croner::Cron;
use thiserror::Error;

/// A parsed cron schedule.
#[derive(Debug, Clone)]
pub struct Schedule {
    inner: Cron,
}

/// An expression that could not be parsed into a [`Schedule`].
///
/// The backend's message is carried as a string so its error type does not
/// leak into the rest of the app.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("{0}")]
pub struct CronError(String);

impl Schedule {
    /// Parse a standard 5-field cron expression.
    ///
    /// Six- and seven-field forms (leading seconds, trailing year) are also
    /// accepted, because the backend accepts them and `docs/specs/spec.md`
    /// § 4 shows one. See `six_field_expression_is_seconds_first`.
    pub fn parse(expr: &str) -> Result<Self, CronError> {
        Cron::from_str(expr)
            .map(|inner| Self { inner })
            .map_err(|e| CronError(e.to_string()))
    }

    /// The first occurrence *strictly after* `after`.
    ///
    /// Returns `None` when the schedule has no upcoming occurrence — an
    /// expression like `0 9 30 2 *` parses cleanly but names a date that
    /// never happens. Callers must handle that; it is a user typo, not a
    /// bug, and must never panic.
    pub fn next_after(&self, after: DateTime<Local>) -> Option<DateTime<Local>> {
        self.inner.find_next_occurrence(&after, false).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    /// Build a local timestamp for tests.
    ///
    /// Every case below is deliberately placed away from a DST transition so
    /// the expectations hold in any timezone: both the input and the expected
    /// output are constructed as local wall-clock time, so only a fold or gap
    /// between them could shift the result.
    fn local(y: i32, m: u32, d: u32, h: u32, mi: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(y, m, d, h, mi, 0)
            .single()
            .expect("test timestamps are unambiguous local times")
    }

    fn next(expr: &str, from: DateTime<Local>) -> DateTime<Local> {
        Schedule::parse(expr)
            .unwrap_or_else(|e| panic!("{expr} should parse: {e}"))
            .next_after(from)
            .unwrap_or_else(|| panic!("{expr} should have an occurrence after {from}"))
    }

    /// Golden table: the cron semantics kree relies on.
    ///
    /// These assertions are backend-agnostic on purpose. If a croner upgrade
    /// or a swap to another library changes any of these, this test fails —
    /// which is the signal a `cargo build` alone cannot give us.
    #[test]
    fn golden_next_occurrences() {
        // (expression, from, expected next)
        let cases = [
            // Step within an hour range, on a weekday.
            (
                "*/30 9-17 * * 1-5",
                local(2026, 1, 5, 9, 0),
                local(2026, 1, 5, 9, 30),
            ),
            // Friday evening rolls over the weekend to Monday morning.
            (
                "*/30 9-17 * * 1-5",
                local(2026, 1, 2, 17, 30),
                local(2026, 1, 5, 9, 0),
            ),
            // Weekday-only daily, skipping Saturday and Sunday.
            (
                "0 17 * * 1-5",
                local(2026, 1, 2, 17, 0),
                local(2026, 1, 5, 17, 0),
            ),
            // Weekly on Monday.
            (
                "0 9 * * 1",
                local(2026, 1, 5, 9, 0),
                local(2026, 1, 12, 9, 0),
            ),
            // Step from a non-aligned minute snaps to the next multiple.
            (
                "*/15 * * * *",
                local(2026, 1, 5, 10, 7),
                local(2026, 1, 5, 10, 15),
            ),
            (
                "*/20 * * * *",
                local(2026, 1, 5, 10, 5),
                local(2026, 1, 5, 10, 20),
            ),
            // Stepped hour range: 9, 13, 17.
            (
                "0 9-17/4 * * *",
                local(2026, 1, 5, 10, 0),
                local(2026, 1, 5, 13, 0),
            ),
            // Yearly, crossing the year boundary.
            (
                "0 0 1 1 *",
                local(2026, 6, 15, 12, 0),
                local(2027, 1, 1, 0, 0),
            ),
            // Leap day: 2026 and 2027 have no Feb 29, so it lands in 2028.
            (
                "0 9 29 2 *",
                local(2026, 1, 1, 0, 0),
                local(2028, 2, 29, 9, 0),
            ),
        ];

        for (expr, from, expected) in cases {
            assert_eq!(next(expr, from), expected, "schedule {expr} from {from}");
        }
    }

    /// The lookup is exclusive of `after`.
    ///
    /// `Schedule::next_after` hardcodes the backend's "inclusive" flag to
    /// false. The scheduler depends on it: it recomputes from `Local::now()`
    /// after every fire (`docs/specs/spec.md` § 5.1), so an inclusive lookup
    /// would return the occurrence that just fired and spin.
    #[test]
    fn next_after_is_exclusive_of_the_reference_instant() {
        let at_nine = local(2026, 1, 5, 9, 0);
        assert_eq!(next("0 9 * * *", at_nine), local(2026, 1, 6, 9, 0));
    }

    /// Both `0` and `7` mean Sunday, as does the `SUN` alias. Libraries
    /// disagree about this often enough to be worth pinning.
    #[test]
    fn sunday_is_zero_seven_and_sun() {
        let from = local(2026, 1, 5, 0, 0);
        let sunday = local(2026, 1, 11, 9, 0);
        assert_eq!(next("0 9 * * 0", from), sunday);
        assert_eq!(next("0 9 * * 7", from), sunday);
        assert_eq!(next("0 9 * * SUN", from), sunday);
    }

    /// A six-field expression is read seconds-first, not year-last.
    ///
    /// `docs/specs/spec.md` § 4 lists `0 0 * 9-17 * *` as an example. It is
    /// six fields, so it parses as sec=0 min=0 hour=* dom=9-17 month=* dow=*
    /// — every minute on the 9th through 17th of the month, which is almost
    /// certainly not what that example intends. Pinned here as the current
    /// behavior; see docs/specs/open-items.md.
    #[test]
    fn six_field_expression_is_seconds_first() {
        assert_eq!(
            next("0 0 * 9-17 * *", local(2026, 1, 5, 10, 0)),
            local(2026, 1, 9, 0, 0)
        );
    }

    /// A schedule naming a date that never occurs parses, then yields
    /// nothing. It must return `None`, not panic and not error out loudly —
    /// February 30th is a user typo.
    #[test]
    fn impossible_date_has_no_next_occurrence() {
        let schedule = Schedule::parse("0 9 30 2 *").expect("parses; the date is just impossible");
        assert_eq!(schedule.next_after(local(2026, 1, 1, 0, 0)), None);
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(Schedule::parse("not a cron").is_err());
        assert!(Schedule::parse("").is_err());
        assert!(Schedule::parse("* * * *").is_err());
    }

    #[test]
    fn error_carries_the_backend_message() {
        let err = Schedule::parse("* * * *").expect_err("too few fields");
        assert!(!err.to_string().is_empty());
    }
}
