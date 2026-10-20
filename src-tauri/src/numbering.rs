//! How invoice numbers are made: a pattern like the one for dates in a layout (`YYYY`, `MM`, `DD`,
//! `[text]`), plus `N` for the counter. `NNN` fills it with zeros to three digits, `N` does not fill.
//! The counter starts again whenever what the pattern writes before or after it changes, `[RE-]YYYY-NNN`
//! counts per year, `[RE-]YYYYMM-NNN` per month, `[RE-]NNNN` never.

use anyhow::{bail, Result};
use en16931::Date;

use crate::render::format::date_pattern;

/// The numbers `RE-2026-001`, `RE-2026-002`, …
pub const DEFAULT_PATTERN: &str = "[RE-]YYYY-NNN";

pub struct Pattern {
    /// What stands before and after the counter, still with the date tokens
    before: String,
    after: String,
    /// The digits the counter is filled up to
    width: usize,
}

/// The positions of the `N`s that are not in `[brackets]`.
fn counters(pattern: &str) -> Vec<usize> {
    let mut found = Vec::new();
    let mut quoted = false;
    for (at, c) in pattern.char_indices() {
        match c {
            '[' if !quoted && pattern[at..].contains(']') => quoted = true,
            ']' if quoted => quoted = false,
            'N' if !quoted => found.push(at),
            _ => {}
        }
    }
    found
}

impl Pattern {
    pub fn parse(pattern: &str) -> Result<Self> {
        let found = counters(pattern);
        let (Some(&first), Some(&last)) = (found.first(), found.last()) else {
            bail!("Das Muster braucht einen Zähler: N, NNN für drei Stellen mit führenden Nullen.");
        };
        // one run of Ns, not a counter here and another there
        if last - first + 1 != found.len() {
            bail!(
                "Das Muster darf nur einen Zähler haben, Text mit N gehört in [eckige Klammern]."
            );
        }
        Ok(Self {
            before: pattern[..first].to_owned(),
            after: pattern[last + 1..].to_owned(),
            width: found.len(),
        })
    }

    fn render(&self, date: Date, counter: u32) -> String {
        format!(
            "{}{counter:0width$}{}",
            date_pattern(date, &self.before),
            date_pattern(date, &self.after),
            width = self.width
        )
    }

    /// The number after the highest of `existing` that the pattern could have made on this day.
    pub fn next<'a>(&self, date: Date, existing: impl IntoIterator<Item = &'a str>) -> String {
        let before = date_pattern(date, &self.before);
        let after = date_pattern(date, &self.after);
        let highest = existing
            .into_iter()
            .filter_map(|n| {
                n.strip_prefix(before.as_str())?
                    .strip_suffix(after.as_str())
            })
            .filter(|digits| !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
            .filter_map(|digits| digits.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        self.render(date, highest + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i32, m: u8, d: u8) -> Date {
        Date::new(y, m, d).unwrap()
    }

    fn next(pattern: &str, date: Date, existing: &[&str]) -> String {
        Pattern::parse(pattern)
            .unwrap()
            .next(date, existing.iter().copied())
    }

    #[test]
    fn the_default_makes_the_numbers_it_always_made() {
        let pattern = Pattern::parse(DEFAULT_PATTERN).unwrap();

        assert_eq!(pattern.next(day(2026, 3, 7), []), "RE-2026-001");
        assert_eq!(
            pattern.next(
                day(2026, 3, 7),
                ["RE-2026-001", "RE-2026-007", "RE-2025-099"]
            ),
            "RE-2026-008"
        );
        assert_eq!(
            pattern.next(day(2027, 1, 1), ["RE-2026-007"]),
            "RE-2027-001"
        );
    }

    #[test]
    fn the_counter_starts_again_when_the_date_part_changes() {
        let existing = ["2026-03-004", "2026-04-009"];

        assert_eq!(
            next("YYYY-MM-NNN", day(2026, 3, 7), &existing),
            "2026-03-005"
        );
        assert_eq!(
            next("YYYY-MM-NNN", day(2026, 5, 1), &existing),
            "2026-05-001"
        );
        // no date in it: one counter for ever
        assert_eq!(next("[R]NNNN", day(2026, 5, 1), &["R0009", "R12"]), "R0013");
    }

    #[test]
    fn the_fill_is_the_number_of_ns_and_a_big_counter_just_grows() {
        assert_eq!(next("N", day(2026, 1, 1), &["9"]), "10");
        assert_eq!(next("NNNNN", day(2026, 1, 1), &[]), "00001");
        assert_eq!(next("NNN", day(2026, 1, 1), &["999"]), "1000");
        assert_eq!(next("YY/DD/NN", day(2026, 3, 7), &["26/07/12"]), "26/07/13");
    }

    #[test]
    fn text_after_the_counter_and_in_brackets_stays() {
        assert_eq!(
            next(
                "[Nr. ]NNN[/]YY",
                day(2026, 3, 7),
                &["Nr. 004/26", "Nr. 009/25"]
            ),
            "Nr. 005/26"
        );
        // an N in brackets is no counter
        assert_eq!(next("[NEU-]NNN", day(2026, 1, 1), &[]), "NEU-001");
    }

    #[test]
    fn a_pattern_needs_exactly_one_counter() {
        assert!(Pattern::parse("YYYY").is_err());
        assert!(Pattern::parse("[N]YYYY").is_err());
        assert!(Pattern::parse("NN-YYYY-NN").is_err());
        assert!(Pattern::parse("N").is_ok());
    }

    #[test]
    fn something_that_is_no_number_is_not_counted() {
        assert_eq!(
            next(
                "[RE-]NNN",
                day(2026, 1, 1),
                &["RE-", "RE-x1", "RE-+5", "XX-004"]
            ),
            "RE-001"
        );
    }
}
