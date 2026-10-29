//! How invoice numbers are made. A pattern only formats: like the one for dates in a layout (`YYYY`,
//! `MM`, `DD`, `[text]`), plus `N` for the counter. `NNN` fills it with zeros to three digits, `N`
//! does not fill. The fiscal year has tokens of its own (`GGGG` the year it starts in, `gggg` the
//! year it ends in, `GG`/`gg` with two digits, `Q` its quarter), computed from the issuer's `Calendar`.
//!
//! When the counter starts again with 1 is a setting of its own, the `Reset`: never, by fiscal year,
//! by quarter or by month. The counters run in periods (`Calendar::period`) of the day a document
//! is issued on, and a document keeps its counter, so changing the pattern never confuses them.
//! A reset only works when the pattern tells the periods apart, `reset_problem` checks that.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, PoisonError};

use anyhow::{bail, Result};
use en16931::Date;
use serde::{Deserialize, Serialize};

use crate::render::format::date_pattern;

/// The numbers `RE-2026-001`, `RE-2026-002`, …
pub const DEFAULT_PATTERN: &str = "[RE-]YYYY-NNN";

/// When the counter starts again with 1
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reset {
    Never,
    /// With every business year, the calendar year without a fiscal year
    #[default]
    FiscalYear,
    /// With every quarter of the business year
    Quarter,
    /// With every calendar month
    Month,
}

impl Reset {
    /// What the pattern has to tell apart, or numbers repeat
    fn needs(self) -> &'static str {
        match self {
            Self::Never => "",
            Self::FiscalYear => "Mit Neubeginn je Geschäftsjahr muss das Muster das Geschäftsjahr enthalten (z. B. GGGG oder, wenn es dem Kalenderjahr entspricht, YYYY), sonst wiederholen sich Nummern.",
            Self::Quarter => "Mit Neubeginn je Quartal muss das Muster das Quartal samt Jahr enthalten (z. B. YYYY-[Q]Q), sonst wiederholen sich Nummern.",
            Self::Month => "Mit Neubeginn je Monat muss das Muster den Monat samt Jahr enthalten (z. B. YYYYMM), sonst wiederholen sich Nummern.",
        }
    }
}

/// Quarters of a short first business year: before its start the calendar quarters count, after
/// it the fiscal ones, and both write the same year and quarter.
const SHORT_YEAR_QUARTERS: &str = "Vor dem Beginn des ersten Geschäftsjahres zählen die Kalenderquartale, danach die Quartale des Geschäftsjahres: Jahr und Quartal wiederholen sich dabei. Mit dem Monat im Muster (z. B. YYYYMM-NN) oder Neubeginn je Monat bleiben die Nummern eindeutig.";

/// The business year of an issuer: the calendar year, or one that starts on a day of the year.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Calendar {
    /// The start of the first business year: its day and month repeat every year, before it the
    /// calendar year applies (a short first year after a switch). `None` is the calendar year.
    pub fiscal_year_start: Option<Date>,
}

/// Where a day lies in the business year
struct Fiscal {
    /// The calendar year it starts in
    from: i32,
    /// The calendar year it ends in
    to: i32,
    quarter: u8,
    /// The calendar year applies: no fiscal year, or the day lies before the first start
    calendar: bool,
}

impl Calendar {
    fn fiscal(self, date: Date) -> Fiscal {
        let calendar = Fiscal {
            from: date.year(),
            to: date.year(),
            quarter: (date.month() - 1) / 3 + 1,
            calendar: true,
        };
        let Some(start) = self.fiscal_year_start.filter(|start| date >= *start) else {
            return calendar;
        };
        let anchor = (start.month(), start.day());
        let from = if (date.month(), date.day()) >= anchor {
            date.year()
        } else {
            date.year() - 1
        };
        // whole months since the year began
        let months = (date.year() - from) * 12 + i32::from(date.month())
            - i32::from(start.month())
            - i32::from(date.day() < start.day());
        Fiscal {
            from,
            to: if anchor == (1, 1) { from } else { from + 1 },
            quarter: u8::try_from(months / 3 + 1).unwrap_or(4).min(4),
            calendar: false,
        }
    }
}

/// A stretch of days one counter runs through, by a `Reset`. A floor (`Store::set_counter`)
/// belongs to one period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Period(String);

impl Period {
    /// What identifies the period in the store, it names the reset as well
    pub fn key(&self) -> &str {
        &self.0
    }
}

/// A number that was given: the counter in it and the day it was issued on
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Given {
    pub number: String,
    pub counter: u32,
    pub date: Date,
}

impl Calendar {
    /// The period a day is in. A business year is named by what the text of `GGGG` says, so a short
    /// first year counts with the one that follows: both write the same year, one counter keeps
    /// their numbers apart. A quarter is told apart by its kind as well: the calendar quarters
    /// before the first start are other stretches of time than the fiscal ones that write the same
    /// `GGGG` and `Q` later, so `reset_problem` asks the pattern to tell them apart.
    pub fn period(self, reset: Reset, date: Date) -> Period {
        let fiscal = self.fiscal(date);
        Period(match reset {
            Reset::Never => "never".to_owned(),
            Reset::FiscalYear => format!("year:{}", fiscal.from),
            Reset::Quarter if fiscal.calendar => {
                format!("quarter:{}-{}", fiscal.from, fiscal.quarter)
            }
            Reset::Quarter => format!("quarter:fiscal-{}-{}", fiscal.from, fiscal.quarter),
            Reset::Month => format!("month:{}-{:02}", date.year(), date.month()),
        })
    }

    /// The highest counter given in a period
    pub fn highest<'a>(
        self,
        reset: Reset,
        period: &Period,
        given: &'a [Given],
    ) -> Option<&'a Given> {
        given
            .iter()
            .filter(|g| self.period(reset, g.date) == *period)
            .max_by_key(|g| g.counter)
    }
}

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

    /// The number a day gets with this counter
    pub fn number(&self, date: Date, calendar: Calendar, counter: u32) -> String {
        format!(
            "{}{counter:0width$}{}",
            written(date, calendar, &self.before),
            written(date, calendar, &self.after),
            width = self.width
        )
    }

    /// What a number of this day looks like before it is given: the counter as `#`, `RE-2026-###`.
    pub fn masked(&self, date: Date, calendar: Calendar) -> String {
        format!(
            "{}{}{}",
            written(date, calendar, &self.before),
            "#".repeat(self.width),
            written(date, calendar, &self.after)
        )
    }

    /// The counter of a number this pattern wrote on that day, for numbers that kept none. None
    /// for any other text.
    pub fn counter_of(&self, number: &str, date: Date, calendar: Calendar) -> Option<u32> {
        let digits = number
            .strip_prefix(written(date, calendar, &self.before).as_str())?
            .strip_suffix(written(date, calendar, &self.after).as_str())?;
        if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        digits.parse().ok()
    }
}

/// Days a check of a reset looks at: every period repeats within three years
const CHECKED_DAYS: i32 = 3 * 366;

/// What a check depends on: the pattern (before the counter, after it, its width), the reset, the
/// start of the business year and the year the check begins in
type CheckKey = ((String, String, usize), Reset, Option<(i32, u8, u8)>, i32);

/// Results are kept: the check formats three years of days, and the settings are checked on every
/// keystroke and every read of a context
static CHECKED: OnceLock<Mutex<HashMap<CheckKey, Option<&'static str>>>> = OnceLock::new();

/// Patterns typed one letter at a time are many, the cache starts over before it grows without end
const CHECKED_KEPT: usize = 512;

/// What keeps a reset from working with a pattern, none when it does: two days of different
/// periods that make the same number would repeat it. Looks at three years from the first of
/// January of `year` (or of the year the business year starts, when that is later).
pub fn reset_problem(
    pattern: &Pattern,
    reset: Reset,
    calendar: Calendar,
    year: i32,
) -> Option<&'static str> {
    if reset == Reset::Never {
        return None;
    }
    let year = calendar
        .fiscal_year_start
        .map_or(year, |start| year.max(start.year()));
    let key: CheckKey = (
        (pattern.before.clone(), pattern.after.clone(), pattern.width),
        reset,
        calendar
            .fiscal_year_start
            .map(|start| (start.year(), start.month(), start.day())),
        year,
    );
    let cache = CHECKED.get_or_init(Mutex::default);
    if let Some(found) = cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
    {
        return *found;
    }
    let found = find_problem(pattern, reset, calendar, year);
    let mut kept = cache.lock().unwrap_or_else(PoisonError::into_inner);
    if kept.len() >= CHECKED_KEPT {
        kept.clear();
    }
    kept.insert(key, found);
    found
}

fn find_problem(
    pattern: &Pattern,
    reset: Reset,
    calendar: Calendar,
    year: i32,
) -> Option<&'static str> {
    let first = Date::new(year, 1, 1).ok()?;
    // the period a text was first seen in, and whether that day still counted calendar quarters
    let mut seen: HashMap<String, (Period, bool)> = HashMap::new();
    (0..CHECKED_DAYS)
        .filter_map(|days| first.checked_add_days(days))
        .find_map(|day| {
            let period = calendar.period(reset, day);
            let before_start = calendar.fiscal(day).calendar;
            let (first_period, first_before_start) = seen
                .entry(pattern.masked(day, calendar))
                .or_insert_with(|| (period.clone(), before_start))
                .clone();
            (first_period != period).then(|| {
                if reset == Reset::Quarter && first_before_start != before_start {
                    SHORT_YEAR_QUARTERS
                } else {
                    reset.needs()
                }
            })
        })
}

/// A pattern with the fiscal tokens, then the date tokens, written out for a day.
fn written(date: Date, calendar: Calendar, pattern: &str) -> String {
    let fiscal = calendar.fiscal(date);
    let tokens = [
        ("GGGG", fiscal.from.to_string()),
        ("GG", format!("{:02}", fiscal.from.rem_euclid(100))),
        ("gggg", fiscal.to.to_string()),
        ("gg", format!("{:02}", fiscal.to.rem_euclid(100))),
        ("Q", fiscal.quarter.to_string()),
    ];
    let mut out = String::new();
    let mut rest = pattern;
    while let Some(first) = rest.chars().next() {
        if first == '[' {
            if let Some(close) = rest.find(']') {
                out.push_str(&rest[..=close]);
                rest = &rest[close + 1..];
                continue;
            }
        }
        if let Some((token, value)) = tokens.iter().find(|(token, _)| rest.starts_with(token)) {
            // digits, which no date token reads
            out.push_str(value);
            rest = &rest[token.len()..];
        } else {
            out.push(first);
            rest = &rest[first.len_utf8()..];
        }
    }
    date_pattern(date, &out)
}

/// Puts the letters that became tokens (`G`, `g`, `Q`) into brackets where they are not yet, so a
/// pattern from before keeps writing what it wrote: `RG-YYYY-NNN` becomes `R[G]-YYYY-NNN`.
pub fn protect_new_tokens(pattern: &str) -> String {
    let mut out = String::new();
    let mut rest = pattern;
    while let Some(first) = rest.chars().next() {
        if first == '[' {
            if let Some(close) = rest.find(']') {
                out.push_str(&rest[..=close]);
                rest = &rest[close + 1..];
                continue;
            }
        }
        if matches!(first, 'G' | 'g' | 'Q') {
            let run = rest
                .find(|c| !matches!(c, 'G' | 'g' | 'Q'))
                .unwrap_or(rest.len());
            out.push('[');
            out.push_str(&rest[..run]);
            out.push(']');
            rest = &rest[run..];
        } else {
            out.push(first);
            rest = &rest[first.len_utf8()..];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i32, m: u8, d: u8) -> Date {
        Date::new(y, m, d).unwrap()
    }

    fn number(pattern: &str, date: Date, counter: u32) -> String {
        Pattern::parse(pattern)
            .unwrap()
            .number(date, Calendar::default(), counter)
    }

    /// Business years that start on 1 April of 2026
    fn april() -> Calendar {
        Calendar {
            fiscal_year_start: Some(day(2026, 4, 1)),
        }
    }

    fn written_in(calendar: Calendar, pattern: &str, date: Date) -> String {
        Pattern::parse(&format!("{pattern}-N"))
            .unwrap()
            .masked(date, calendar)
            .trim_end_matches("-#")
            .to_owned()
    }

    fn key(calendar: Calendar, reset: Reset, date: Date) -> String {
        calendar.period(reset, date).key().to_owned()
    }

    /// The next counter as the store works it out: the highest of the period plus one
    fn next(calendar: Calendar, reset: Reset, date: Date, issued: &[(u32, Date)]) -> u32 {
        let given: Vec<Given> = issued
            .iter()
            .map(|&(counter, date)| Given {
                number: counter.to_string(),
                counter,
                date,
            })
            .collect();
        calendar
            .highest(reset, &calendar.period(reset, date), &given)
            .map_or(1, |g| g.counter + 1)
    }

    #[test]
    fn the_default_makes_the_numbers_it_always_made() {
        assert_eq!(number(DEFAULT_PATTERN, day(2026, 3, 7), 1), "RE-2026-001");
        assert_eq!(number(DEFAULT_PATTERN, day(2027, 1, 1), 8), "RE-2027-008");
        assert_eq!(Reset::default(), Reset::FiscalYear);
    }

    #[test]
    fn the_fill_is_the_number_of_ns_and_a_big_counter_just_grows() {
        let date = day(2026, 1, 1);
        assert_eq!(number("N", date, 10), "10");
        assert_eq!(number("NNNNN", date, 1), "00001");
        assert_eq!(number("NNN", date, 1000), "1000");
        assert_eq!(number("YY/DD/NN", day(2026, 3, 7), 13), "26/07/13");
    }

    #[test]
    fn text_after_the_counter_and_in_brackets_stays() {
        assert_eq!(number("[Nr. ]NNN[/]YY", day(2026, 3, 7), 5), "Nr. 005/26");
        // an N in brackets is no counter
        assert_eq!(number("[NEU-]NNN", day(2026, 1, 1), 1), "NEU-001");
    }

    #[test]
    fn a_masked_number_shows_the_pattern_with_the_counter_left_open() {
        let masked = |pattern: &str| {
            Pattern::parse(pattern)
                .unwrap()
                .masked(day(2026, 3, 7), Calendar::default())
        };

        assert_eq!(masked(DEFAULT_PATTERN), "RE-2026-###");
        assert_eq!(masked("[Nr. ]N[/]YY"), "Nr. #/26");
        // the fiscal year is in it as well
        assert_eq!(
            Pattern::parse("[RE-]GGGG/gg-NNN")
                .unwrap()
                .masked(day(2026, 3, 7), april()),
            "RE-2026/26-###"
        );
    }

    #[test]
    fn a_pattern_needs_exactly_one_counter() {
        assert!(Pattern::parse("YYYY").is_err());
        assert!(Pattern::parse("[N]YYYY").is_err());
        assert!(Pattern::parse("NN-YYYY-NN").is_err());
        assert!(Pattern::parse("N").is_ok());
    }

    #[test]
    fn the_counter_of_a_number_is_read_back_by_the_pattern_of_its_day() {
        let pattern = Pattern::parse("[RE-]NNN").unwrap();
        let counter = |number| pattern.counter_of(number, day(2026, 1, 1), Calendar::default());

        assert_eq!(counter("RE-007"), Some(7));
        // something that is no number of the pattern has no counter
        for text in ["RE-", "RE-x1", "RE-+5", "XX-004"] {
            assert_eq!(counter(text), None, "{text}");
        }
        let dated = Pattern::parse("[RE-]YYYY-NNN").unwrap();
        assert_eq!(
            dated.counter_of("RE-2026-004", day(2026, 5, 1), Calendar::default()),
            Some(4)
        );
        assert_eq!(
            dated.counter_of("RE-2026-004", day(2027, 5, 1), Calendar::default()),
            None
        );
    }

    #[test]
    fn the_fiscal_year_is_named_by_the_year_it_starts_and_the_year_it_ends_in() {
        let all = |date| written_in(april(), "GGGG|GG|gggg|gg", date);

        assert_eq!(all(day(2026, 4, 1)), "2026|26|2027|27");
        assert_eq!(all(day(2027, 3, 31)), "2026|26|2027|27");
        assert_eq!(all(day(2027, 4, 1)), "2027|27|2028|28");
        assert_eq!(written_in(april(), "GGGG/gg", day(2026, 10, 29)), "2026/27");
        // a fiscal year that starts on 1 January is the calendar year
        let january = Calendar {
            fiscal_year_start: Some(day(2026, 1, 1)),
        };
        assert_eq!(
            written_in(january, "GGGG|gggg", day(2027, 6, 1)),
            "2027|2027"
        );
    }

    #[test]
    fn before_the_first_start_the_calendar_year_applies() {
        let all = |date| written_in(april(), "GGGG|gggg|Q", date);

        // a short first year: January to March 2026
        assert_eq!(all(day(2026, 1, 15)), "2026|2026|1");
        assert_eq!(all(day(2026, 3, 31)), "2026|2026|1");
        assert_eq!(all(day(2025, 12, 31)), "2025|2025|4");
        // from the start on the year repeats
        assert_eq!(all(day(2026, 4, 1)), "2026|2027|1");
    }

    #[test]
    fn a_year_that_starts_in_the_middle_of_a_month_changes_on_that_day() {
        let calendar = Calendar {
            fiscal_year_start: Some(day(2026, 7, 15)),
        };
        let all = |date| written_in(calendar, "GGGG|Q", date);

        assert_eq!(all(day(2027, 7, 14)), "2026|4");
        assert_eq!(all(day(2027, 7, 15)), "2027|1");
        assert_eq!(all(day(2026, 10, 14)), "2026|1");
        assert_eq!(all(day(2026, 10, 15)), "2026|2");
    }

    #[test]
    fn the_quarter_is_the_one_of_the_fiscal_year() {
        let quarter = |calendar, date| written_in(calendar, "Q", date);

        assert_eq!(quarter(april(), day(2026, 4, 1)), "1");
        assert_eq!(quarter(april(), day(2026, 7, 1)), "2");
        assert_eq!(quarter(april(), day(2026, 12, 31)), "3");
        assert_eq!(quarter(april(), day(2027, 3, 31)), "4");
        // the calendar quarter without a fiscal year
        let calendar = Calendar::default();
        assert_eq!(quarter(calendar, day(2026, 1, 1)), "1");
        assert_eq!(quarter(calendar, day(2026, 3, 31)), "1");
        assert_eq!(quarter(calendar, day(2026, 4, 1)), "2");
        assert_eq!(quarter(calendar, day(2026, 12, 31)), "4");
        // and with a fiscal year that starts in January
        let january = Calendar {
            fiscal_year_start: Some(day(2025, 1, 1)),
        };
        assert_eq!(quarter(january, day(2026, 8, 1)), "3");
    }

    #[test]
    fn the_period_of_a_year_is_the_calendar_year_or_the_fiscal_year() {
        let calendar = Calendar::default();
        let year = |date| key(calendar, Reset::FiscalYear, date);

        assert_eq!(year(day(2026, 1, 1)), year(day(2026, 12, 31)));
        assert_ne!(year(day(2026, 12, 31)), year(day(2027, 1, 1)));

        let fiscal = |date| key(april(), Reset::FiscalYear, date);
        assert_eq!(fiscal(day(2026, 4, 1)), fiscal(day(2027, 3, 31)));
        assert_ne!(fiscal(day(2027, 3, 31)), fiscal(day(2027, 4, 1)));
        // the new year does not start a period
        assert_eq!(fiscal(day(2026, 12, 31)), fiscal(day(2027, 1, 1)));
    }

    #[test]
    fn the_period_of_a_quarter_follows_the_fiscal_year_over_new_year() {
        let quarter = |date| key(april(), Reset::Quarter, date);

        // October to December, then January to March are two quarters of one fiscal year
        assert_eq!(quarter(day(2026, 10, 1)), quarter(day(2026, 12, 31)));
        assert_ne!(quarter(day(2026, 12, 31)), quarter(day(2027, 1, 1)));
        assert_eq!(quarter(day(2027, 1, 1)), quarter(day(2027, 3, 31)));
        assert_ne!(quarter(day(2027, 3, 31)), quarter(day(2027, 4, 1)));
        // the same quarter of another fiscal year is another period
        assert_ne!(quarter(day(2026, 5, 1)), quarter(day(2027, 5, 1)));
        // the calendar quarter without a fiscal year
        let calendar = Calendar::default();
        let plain = |date| key(calendar, Reset::Quarter, date);
        assert_eq!(plain(day(2026, 1, 1)), plain(day(2026, 3, 31)));
        assert_ne!(plain(day(2026, 3, 31)), plain(day(2026, 4, 1)));
        assert_ne!(plain(day(2026, 2, 1)), plain(day(2027, 2, 1)));
    }

    /// Business years that start on 15 July of 2026
    fn midsummer() -> Calendar {
        Calendar {
            fiscal_year_start: Some(day(2026, 7, 15)),
        }
    }

    #[test]
    fn the_quarters_of_a_short_first_year_are_not_those_of_the_fiscal_years() {
        let quarter = |date| key(midsummer(), Reset::Quarter, date);

        // 1 to 14 July 2026 is the third calendar quarter, 15 January to 14 April 2027 the third
        // fiscal one: they write the same year and quarter and are not the same stretch of time
        assert_ne!(quarter(day(2026, 7, 10)), quarter(day(2027, 2, 1)));
        // nor are January to March 2026 and the first fiscal quarter, 15 July to 14 October
        assert_ne!(quarter(day(2026, 2, 1)), quarter(day(2026, 8, 1)));
        // a quarter is still whole: calendar ones within a calendar year, fiscal ones over new year
        assert_eq!(quarter(day(2026, 4, 1)), quarter(day(2026, 6, 30)));
        assert_eq!(quarter(day(2026, 12, 1)), quarter(day(2027, 1, 14)));
        assert_ne!(quarter(day(2027, 1, 14)), quarter(day(2027, 1, 15)));
        // every day of a stretch has its key, no two stretches share one
        let keys = (0..700)
            .filter_map(|days| day(2026, 1, 1).checked_add_days(days))
            .map(quarter)
            .fold(Vec::<String>::new(), |mut seen, key| {
                if seen.last() != Some(&key) {
                    seen.push(key);
                }
                seen
            });
        let distinct: std::collections::HashSet<_> = keys.iter().collect();
        assert_eq!(keys.len(), distinct.len(), "{keys:?}");
    }

    #[test]
    fn the_short_first_year_shares_its_year_with_the_fiscal_year_that_follows() {
        let year = |date| key(midsummer(), Reset::FiscalYear, date);

        // both write the year 2026 (`GGGG`), one counter keeps their numbers apart
        assert_eq!(year(day(2026, 3, 1)), year(day(2026, 8, 1)));
        assert_eq!(year(day(2026, 3, 1)), year(day(2027, 7, 14)));
        assert_ne!(year(day(2025, 12, 31)), year(day(2026, 1, 1)));
        assert_ne!(year(day(2027, 7, 14)), year(day(2027, 7, 15)));
        // and no pattern of `GGGG` repeats a number over it
        let pattern = Pattern::parse("[RE-]GGGG-NNN").unwrap();
        assert_eq!(
            reset_problem(&pattern, Reset::FiscalYear, midsummer(), 2026),
            None
        );
    }

    #[test]
    fn a_reset_by_quarter_asks_the_pattern_to_tell_a_short_first_year_apart() {
        let problem = |pattern: &str, year| {
            reset_problem(
                &Pattern::parse(pattern).unwrap(),
                Reset::Quarter,
                midsummer(),
                year,
            )
        };

        // 2026 holds the short year, whose quarters write what the fiscal ones write
        assert!(problem("[RE-]GGGG-[Q]Q-NN", 2026).is_some());
        assert!(problem("[RE-]YYYY-[Q]Q-NN", 2026).is_some());
        // later years are only fiscal ones, whose quarters the pattern tells apart
        assert_eq!(problem("[RE-]GGGG-[Q]Q-NN", 2027), None);
        assert_eq!(problem("[RE-]GGGG-[Q]Q-NN", 2028), None);
        assert!(problem("[RE-]YYYY-[Q]Q-NN", 2027).is_some());
    }

    #[test]
    fn the_period_of_a_month_is_the_calendar_month_of_a_year() {
        let month = |date| key(april(), Reset::Month, date);

        assert_eq!(month(day(2026, 3, 1)), month(day(2026, 3, 31)));
        assert_ne!(month(day(2026, 3, 31)), month(day(2026, 4, 1)));
        assert_ne!(month(day(2026, 3, 7)), month(day(2027, 3, 7)));
    }

    #[test]
    fn no_reset_is_one_period_and_the_periods_of_the_resets_never_equal_each_other() {
        let date = day(2026, 3, 7);
        let keys = [
            Reset::Never,
            Reset::FiscalYear,
            Reset::Quarter,
            Reset::Month,
        ]
        .map(|reset| key(Calendar::default(), reset, date));

        assert_eq!(
            key(Calendar::default(), Reset::Never, day(1999, 1, 1)),
            keys[0]
        );
        assert_eq!(
            keys.iter().collect::<std::collections::HashSet<_>>().len(),
            4
        );
    }

    #[test]
    fn the_counter_goes_on_in_a_period_whatever_the_pattern_wrote() {
        let calendar = Calendar::default();
        let issued = [
            (4, day(2026, 3, 1)),
            (9, day(2026, 4, 9)),
            (12, day(2025, 12, 31)),
        ];

        // by year: the highest of 2026 counts, whatever month it was in
        assert_eq!(
            next(calendar, Reset::FiscalYear, day(2026, 5, 1), &issued),
            10
        );
        assert_eq!(
            next(calendar, Reset::FiscalYear, day(2027, 1, 1), &issued),
            1
        );
        // by month: only the month counts
        assert_eq!(next(calendar, Reset::Month, day(2026, 3, 20), &issued), 5);
        assert_eq!(next(calendar, Reset::Month, day(2026, 5, 1), &issued), 1);
        // by quarter
        assert_eq!(next(calendar, Reset::Quarter, day(2026, 6, 1), &issued), 10);
        assert_eq!(next(calendar, Reset::Quarter, day(2026, 7, 1), &issued), 1);
        // never: the highest of all
        assert_eq!(next(calendar, Reset::Never, day(2030, 1, 1), &issued), 13);
        // the numbers of a fiscal year go on over new year
        assert_eq!(
            next(
                april(),
                Reset::FiscalYear,
                day(2027, 2, 1),
                &[(5, day(2026, 12, 1))]
            ),
            6
        );
    }

    #[test]
    fn the_highest_number_of_a_period_comes_with_its_number() {
        let calendar = Calendar::default();
        let given = [
            ("RE-2026-003", 3, day(2026, 2, 1)),
            ("RE-2026-087", 87, day(2026, 3, 1)),
            ("RE-2025-120", 120, day(2025, 3, 1)),
        ]
        .map(|(number, counter, date)| Given {
            number: number.into(),
            counter,
            date,
        });
        let period = |date| calendar.period(Reset::FiscalYear, date);

        assert_eq!(
            calendar
                .highest(Reset::FiscalYear, &period(day(2026, 6, 1)), &given)
                .map(|g| g.number.as_str()),
            Some("RE-2026-087")
        );
        assert_eq!(
            calendar.highest(Reset::FiscalYear, &period(day(2027, 6, 1)), &given),
            None
        );
    }

    #[test]
    fn a_reset_needs_the_period_in_the_number() {
        let refused = |pattern: &str, reset, calendar| {
            reset_problem(&Pattern::parse(pattern).unwrap(), reset, calendar, 2026)
        };
        let calendar = Calendar::default();

        // never accepts every pattern
        for pattern in ["NNN", "[R]N", "MM-NNN", "YYYY-NNN"] {
            assert_eq!(refused(pattern, Reset::Never, calendar), None, "{pattern}");
            assert_eq!(refused(pattern, Reset::Never, april()), None, "{pattern}");
        }
        // by year
        assert_eq!(refused("[RE-]YYYY-NNN", Reset::FiscalYear, calendar), None);
        assert_eq!(refused("N/YY", Reset::FiscalYear, calendar), None);
        assert!(refused("NNN", Reset::FiscalYear, calendar)
            .unwrap()
            .starts_with(
                "Mit Neubeginn je Geschäftsjahr muss das Muster das Geschäftsjahr enthalten"
            ));
        assert!(refused("YYYY-MM-NNN", Reset::FiscalYear, calendar).is_none());
        // a fiscal year from April: the calendar year says nothing about it
        assert!(refused("[RE-]YYYY-NNN", Reset::FiscalYear, april()).is_some());
        assert_eq!(refused("[RE-]GGGG-NNN", Reset::FiscalYear, april()), None);
        assert_eq!(refused("GGGG/gg-NNN", Reset::FiscalYear, april()), None);
        // a business year that is the calendar year is written by either
        let january = Calendar {
            fiscal_year_start: Some(day(2025, 1, 1)),
        };
        assert_eq!(refused("[RE-]GGGG-NNN", Reset::FiscalYear, january), None);
        assert_eq!(refused("[RE-]YYYY-NNN", Reset::FiscalYear, january), None);
        // by quarter and month the year belongs in as well
        assert_eq!(refused("YYYY-[Q]Q-NN", Reset::Quarter, calendar), None);
        // a short first year has calendar quarters of its own, which write the same year and
        // quarter as the fiscal ones (January to March 2026 and April to June 2026 are both
        // 2026-Q1): the year the check starts in the year of the start refuses that
        assert_eq!(
            refused("YYYY-[Q]Q-NN", Reset::Quarter, april()),
            Some(SHORT_YEAR_QUARTERS)
        );
        let from_2027 = |pattern: &str, calendar| {
            reset_problem(
                &Pattern::parse(pattern).unwrap(),
                Reset::Quarter,
                calendar,
                2027,
            )
        };
        assert_eq!(from_2027("YYYY-[Q]Q-NN", april()), None);
        assert_eq!(from_2027("GGGG-[Q]Q-NN", april()), None);
        assert_eq!(
            refused("[Q]Q-NN", Reset::Quarter, calendar),
            Some(Reset::Quarter.needs())
        );
        assert!(refused("YYYY-NNN", Reset::Quarter, calendar).is_some());
        assert_eq!(refused("YYYYMM-NNN", Reset::Month, calendar), None);
        assert_eq!(refused("YYYYMM-NNN", Reset::Month, april()), None);
        assert!(refused("MM-NNN", Reset::Month, calendar).is_some());
        assert!(refused("YYYY-NNN", Reset::Month, calendar).is_some());
    }

    #[test]
    fn a_kept_answer_is_the_one_the_check_gives() {
        let cases = [
            ("NNN", Reset::FiscalYear, Calendar::default()),
            ("[RE-]YYYY-NNN", Reset::FiscalYear, Calendar::default()),
            ("[RE-]YYYY-NNN", Reset::FiscalYear, april()),
            ("[RE-]GGGG-NNN", Reset::FiscalYear, april()),
            ("YYYY-[Q]Q-NN", Reset::Quarter, april()),
            ("YYYYMM-NNN", Reset::Month, midsummer()),
        ];
        for (text, reset, calendar) in cases {
            let pattern = Pattern::parse(text).unwrap();
            let year = calendar.fiscal_year_start.map_or(2026, Date::year);

            // twice: the second answer comes from the cache
            for _ in 0..2 {
                assert_eq!(
                    reset_problem(&pattern, reset, calendar, 2026),
                    find_problem(&pattern, reset, calendar, year.max(2026)),
                    "{text}"
                );
            }
        }
    }

    #[test]
    fn the_problem_is_told_in_german_per_reset() {
        let pattern = Pattern::parse("NNN").unwrap();
        let problem = |reset| reset_problem(&pattern, reset, Calendar::default(), 2026).unwrap();

        assert_eq!(
            problem(Reset::FiscalYear),
            "Mit Neubeginn je Geschäftsjahr muss das Muster das Geschäftsjahr enthalten (z. B. GGGG oder, wenn es dem Kalenderjahr entspricht, YYYY), sonst wiederholen sich Nummern."
        );
        assert!(problem(Reset::Quarter).contains("je Quartal"));
        assert!(problem(Reset::Month).contains("je Monat"));
    }

    #[test]
    fn a_fiscal_year_in_the_future_is_checked_when_it_is_there() {
        // the calendar year applies before the first start: the check looks at the years from it on
        let later = Calendar {
            fiscal_year_start: Some(day(2028, 4, 1)),
        };
        let pattern = Pattern::parse("[RE-]YYYY-NNN").unwrap();

        assert!(reset_problem(&pattern, Reset::FiscalYear, later, 2026).is_some());
    }

    #[test]
    fn protected_patterns_write_what_they_wrote_before() {
        let date = day(2026, 5, 7);
        let cases = [
            ("RG-YYYY-NNN", "RG-2026-007"),
            ("Q-NNN", "Q-007"),
            ("GG-MM-NNN", "GG-05-007"),
            ("gQ/YY/NNN", "gQ/26/007"),
            ("[RE-]YYYY-NNN", "RE-2026-007"),
        ];
        for (old, written) in cases {
            let pattern = Pattern::parse(&protect_new_tokens(old)).unwrap();

            // with a fiscal year the letters would be tokens, brackets keep them text
            assert_eq!(pattern.number(date, april(), 7), written, "{old}");
        }
    }

    #[test]
    fn letters_that_became_tokens_are_put_in_brackets_once() {
        assert_eq!(protect_new_tokens("RG-YYYY-NNN"), "R[G]-YYYY-NNN");
        assert_eq!(protect_new_tokens("Q-NNN"), "[Q]-NNN");
        assert_eq!(protect_new_tokens("GGgQ"), "[GGgQ]");
        assert_eq!(protect_new_tokens("[RE-]YYYY-NNN"), "[RE-]YYYY-NNN");
        // what is in brackets is left alone, and so is a second run
        assert_eq!(protect_new_tokens("[GG]Q"), "[GG][Q]");
        assert_eq!(
            protect_new_tokens(&protect_new_tokens("RG-QYYYY-NNN")),
            "R[G]-[Q]YYYY-NNN"
        );
    }
}
