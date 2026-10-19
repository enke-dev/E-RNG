//! How numbers, money and dates are written in the document, by what the layout says (`Format`).
//! The defaults are the German way: `1.234,56 €`, `19 %`, `18.10.2026`.

use en16931::Date;

use crate::layout::Format;

const NBSP: char = '\u{a0}';

const MONTHS: [&str; 12] = [
    "Januar",
    "Februar",
    "März",
    "April",
    "Mai",
    "Juni",
    "Juli",
    "August",
    "September",
    "Oktober",
    "November",
    "Dezember",
];

const MONTHS_SHORT: [&str; 12] = [
    "Jan.", "Feb.", "März", "Apr.", "Mai", "Juni", "Juli", "Aug.", "Sept.", "Okt.", "Nov.", "Dez.",
];

pub struct Formatter(Format);

impl Formatter {
    pub fn new(format: &Format) -> Self {
        Self(format.clone())
    }

    /// `-` for a deduction, the typographic minus if the layout wants it.
    pub fn minus(&self) -> &'static str {
        if self.0.true_minus {
            "\u{2212}"
        } else {
            "-"
        }
    }

    /// `1234.5` → `1.234,5` (the separators are the layout's), the sign stays in front.
    pub fn number(&self, plain: &str) -> String {
        let (sign, digits) = plain
            .strip_prefix('-')
            .map_or(("", plain), |rest| (self.minus(), rest));
        let (int, frac) = digits.split_once('.').unwrap_or((digits, ""));
        let mut grouped = String::new();
        for (i, c) in int.chars().enumerate() {
            if i > 0 && (int.len() - i) % 3 == 0 {
                grouped.push_str(&self.0.thousands);
            }
            grouped.push(c);
        }
        if frac.is_empty() {
            format!("{sign}{grouped}")
        } else {
            format!("{sign}{grouped}{}{frac}", self.0.decimal)
        }
    }

    /// A quantity or rate without trailing zeros: `10.000` → `10`, `2.50` → `2,5`.
    pub fn trimmed(&self, plain: &str) -> String {
        let trimmed = if plain.contains('.') {
            plain.trim_end_matches('0').trim_end_matches('.')
        } else {
            plain
        };
        self.number(trimmed)
    }

    /// A rate: `19 %`, or `19%`.
    pub fn percent(&self, plain: &str) -> String {
        let space = if self.0.percent_space { "\u{a0}" } else { "" };
        format!("{}{space}%", self.trimmed(plain))
    }

    /// The currency as a symbol where there is a well known one, else its code.
    fn currency(&self, code: &str) -> String {
        if !self.0.currency_symbol {
            return code.to_owned();
        }
        match code {
            "EUR" => "€",
            "USD" => "$",
            "GBP" => "£",
            "JPY" => "¥",
            other => other,
        }
        .to_owned()
    }

    /// `5` → `5,00 €`: prices show cents even where the model drops trailing zeros.
    pub fn money(&self, amount: impl std::fmt::Display, currency: &str) -> String {
        let plain = amount.to_string();
        let (int, frac) = plain.split_once('.').unwrap_or((&plain, ""));
        let amount = self.number(&format!("{int}.{frac:0<2}"));
        let space = if self.0.currency_space {
            NBSP.to_string()
        } else {
            String::new()
        };
        let currency = self.currency(currency);
        if self.0.currency_after {
            format!("{amount}{space}{currency}")
        } else {
            format!("{currency}{space}{amount}")
        }
    }

    /// An IBAN in groups of four, or as it is.
    pub fn iban(&self, iban: &str) -> String {
        if !self.0.iban_groups {
            return iban.to_owned();
        }
        let compact: Vec<char> = iban.chars().filter(|c| !c.is_whitespace()).collect();
        compact
            .chunks(4)
            .map(|chunk| chunk.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn date(&self, date: Date) -> String {
        date_pattern(date, &self.0.date)
    }

    pub fn date_long(&self, date: Date) -> String {
        date_pattern(date, &self.0.date_long)
    }
}

/// A date by a pattern of the kind dayjs uses: `YYYY` `YY` the year, `MMMM` `MMM` the name of the
/// month, `MM` `M` its number, `DD` `D` the day. What is in `[brackets]` and any other character
/// stay as they are.
pub fn date_pattern(date: Date, pattern: &str) -> String {
    let month = usize::from(date.month()).saturating_sub(1);
    let tokens: [(&str, String); 8] = [
        ("YYYY", date.year().to_string()),
        ("YY", format!("{:02}", date.year().rem_euclid(100))),
        (
            "MMMM",
            MONTHS.get(month).copied().unwrap_or_default().to_owned(),
        ),
        (
            "MMM",
            MONTHS_SHORT
                .get(month)
                .copied()
                .unwrap_or_default()
                .to_owned(),
        ),
        ("MM", format!("{:02}", date.month())),
        ("M", date.month().to_string()),
        ("DD", format!("{:02}", date.day())),
        ("D", date.day().to_string()),
    ];
    let mut out = String::new();
    let mut rest = pattern;
    while let Some(first) = rest.chars().next() {
        if first == '[' {
            if let Some(close) = rest.find(']') {
                out.push_str(&rest[1..close]);
                rest = &rest[close + 1..];
                continue;
            }
        }
        if let Some((token, value)) = tokens.iter().find(|(token, _)| rest.starts_with(token)) {
            out.push_str(value);
            rest = &rest[token.len()..];
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

    fn german() -> Formatter {
        Formatter::new(&Format::default())
    }

    #[test]
    fn german_numbers_group_thousands_and_use_a_comma() {
        let fmt = german();
        assert_eq!(fmt.number("1234567.89"), "1.234.567,89");
        assert_eq!(fmt.number("999.5"), "999,5");
        assert_eq!(fmt.number("-1000.00"), "-1.000,00");
        assert_eq!(fmt.number("12"), "12");
    }

    #[test]
    fn the_separators_are_the_layouts() {
        let english = Formatter::new(&Format {
            decimal: ".".into(),
            thousands: ",".into(),
            ..Format::default()
        });
        assert_eq!(english.number("1234567.89"), "1,234,567.89");
        let swiss = Formatter::new(&Format {
            decimal: ".".into(),
            thousands: "’".into(),
            true_minus: true,
            ..Format::default()
        });
        assert_eq!(swiss.number("-1234.5"), "\u{2212}1’234.5");
        let none = Formatter::new(&Format {
            thousands: String::new(),
            ..Format::default()
        });
        assert_eq!(none.number("1234.5"), "1234,5");
    }

    #[test]
    fn quantities_lose_trailing_zeros() {
        let fmt = german();
        assert_eq!(fmt.trimmed("10.000"), "10");
        assert_eq!(fmt.trimmed("2.50"), "2,5");
        assert_eq!(fmt.trimmed("19"), "19");
    }

    #[test]
    fn money_is_written_the_way_the_layout_says() {
        let fmt = german();
        assert_eq!(fmt.money("1000", "EUR"), "1.000,00\u{a0}€");
        assert_eq!(fmt.money("0.5", "EUR"), "0,50\u{a0}€");
        assert_eq!(fmt.money("19.999", "EUR"), "19,999\u{a0}€");
        assert_eq!(fmt.money("5.00", "CHF"), "5,00\u{a0}CHF");

        let code_first = Formatter::new(&Format {
            currency_symbol: false,
            currency_after: false,
            currency_space: false,
            ..Format::default()
        });
        assert_eq!(code_first.money("1234.5", "EUR"), "EUR1.234,50");
        let symbol_first = Formatter::new(&Format {
            currency_after: false,
            ..Format::default()
        });
        assert_eq!(symbol_first.money("12", "USD"), "$\u{a0}12,00");
    }

    #[test]
    fn a_percent_sign_has_a_space_or_not() {
        assert_eq!(german().percent("19.00"), "19\u{a0}%");
        let tight = Formatter::new(&Format {
            percent_space: false,
            ..Format::default()
        });
        assert_eq!(tight.percent("7"), "7%");
    }

    #[test]
    fn an_iban_comes_in_groups_of_four_if_asked() {
        assert_eq!(
            german().iban("DE02120300000000202051"),
            "DE02120300000000202051"
        );
        let grouped = Formatter::new(&Format {
            iban_groups: true,
            ..Format::default()
        });
        assert_eq!(
            grouped.iban("DE02 1203 0000 0000 2020 51"),
            "DE02 1203 0000 0000 2020 51"
        );
        assert_eq!(
            grouped.iban("DE02120300000000202051"),
            "DE02 1203 0000 0000 2020 51"
        );
    }

    #[test]
    fn dates_follow_a_pattern() {
        let day = Date::new(2026, 3, 7).unwrap();
        assert_eq!(date_pattern(day, "DD.MM.YYYY"), "07.03.2026");
        assert_eq!(date_pattern(day, "D.M.YY"), "7.3.26");
        assert_eq!(date_pattern(day, "YYYY-MM-DD"), "2026-03-07");
        assert_eq!(date_pattern(day, "D. MMMM YYYY"), "7. März 2026");
        assert_eq!(date_pattern(day, "D MMM YYYY"), "7 März 2026");
        // text in brackets is not a pattern: the D of "Datum" stays
        assert_eq!(date_pattern(day, "[Datum:] DD/MM"), "Datum: 07/03");
        assert_eq!(german().date(day), "07.03.2026");
        assert_eq!(german().date_long(day), "7. März 2026");
    }
}
