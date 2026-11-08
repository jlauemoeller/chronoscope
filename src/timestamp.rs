//! Timestamp display (product §7.6).

use std::fmt::Write;

use chrono::format::{Item, StrftimeItems};
use chrono::{DateTime, FixedOffset, Local, TimeZone, Utc};

/// The default `timestamp_format`.
pub const DEFAULT_TIMESTAMP_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

/// Returns whether `format` is a strftime string chrono accepts: it must parse
/// and also render (some specifiers, such as `%#z`, parse but fail to render).
pub fn is_valid_format(format: &str) -> bool {
    if StrftimeItems::new(format).any(|item| matches!(item, Item::Error)) {
        return false;
    }
    let Some(offset) = FixedOffset::east_opt(0) else {
        return false;
    };
    let Some(sample) = offset.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).single() else {
        return false;
    };
    let mut out = String::new();
    write!(
        out,
        "{}",
        sample.format_with_items(StrftimeItems::new(format))
    )
    .is_ok()
}

/// The timezone used when `utc` is off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayTz {
    /// The system's local timezone.
    Local,
    /// A fixed offset (used by tests for determinism).
    Fixed(FixedOffset),
}

/// Formats timestamps with the user's `timestamp_format`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimestampFormatter {
    format: String,
    short_format: String,
    utc: bool,
    tz: DisplayTz,
}

impl TimestampFormatter {
    /// Builds a formatter. An invalid `format` falls back to
    /// [`DEFAULT_TIMESTAMP_FORMAT`] (config loading already warns about it).
    ///
    /// With `utc` set, a `Z` is always appended, so a format that itself
    /// contains a zone specifier (`%z`, `%Z`) shows the zone twice.
    pub fn new(format: &str, utc: bool, tz: DisplayTz) -> Self {
        let format = if is_valid_format(format) {
            format
        } else {
            DEFAULT_TIMESTAMP_FORMAT
        };
        let short_format = short_format(format);
        Self {
            format: format.to_owned(),
            short_format,
            utc,
            tz,
        }
    }

    /// Renders `time` with the full format. With `utc` set, the time is shown
    /// in UTC with a `Z` suffix; otherwise in the display timezone.
    pub fn format(&self, time: DateTime<Utc>) -> String {
        self.render(time, &self.format)
    }

    /// Like [`format`](Self::format) but without seconds (for the selector's
    /// session rows).
    pub fn format_short(&self, time: DateTime<Utc>) -> String {
        self.render(time, &self.short_format)
    }

    /// Renders `time` with `format` (a strftime string; not validated, so
    /// pass a fixed one) in the display timezone: UTC when `utc` is set,
    /// else the display timezone. Unlike [`format`](Self::format) it adds no
    /// `Z`, so it is fit for file names (`%Y%m%d-%H%M`).
    pub fn format_with(&self, time: DateTime<Utc>, format: &str) -> String {
        self.render_as(time, format, false)
    }

    fn render(&self, time: DateTime<Utc>, format: &str) -> String {
        self.render_as(time, format, true)
    }

    fn render_as(&self, time: DateTime<Utc>, format: &str, z: bool) -> String {
        let items = StrftimeItems::new(format);
        let mut out = String::new();
        // The format was validated by `is_valid_format`, so writing cannot
        // fail; if it somehow does, whatever was rendered so far is shown.
        let _ = if self.utc {
            let suffix = if z { "Z" } else { "" };
            write!(out, "{}{suffix}", time.format_with_items(items))
        } else {
            match self.tz {
                DisplayTz::Local => write!(
                    out,
                    "{}",
                    time.with_timezone(&Local).format_with_items(items)
                ),
                DisplayTz::Fixed(offset) => write!(
                    out,
                    "{}",
                    time.with_timezone(&offset).format_with_items(items)
                ),
            }
        };
        out
    }
}

/// One piece of a strftime string.
enum Token {
    Literal(char),
    /// A conversion specifier with its conversion character, e.g. `%.3f`.
    Spec(String, char),
}

fn tokenize(format: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = format.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            tokens.push(Token::Literal(c));
            continue;
        }
        let mut spec = String::from('%');
        let mut conv = None;
        while let Some(&next) = chars.peek() {
            chars.next();
            spec.push(next);
            if next.is_ascii_alphabetic() || next == '%' || next == '+' {
                conv = Some(next);
                break;
            }
        }
        match conv {
            Some('%') if spec == "%%" => tokens.push(Token::Literal('%')),
            Some(conv) => tokens.push(Token::Spec(spec, conv)),
            // A dangling `%`: keep it as is (chrono rejects it anyway).
            None => spec.chars().for_each(|c| tokens.push(Token::Literal(c))),
        }
    }
    tokens
}

/// Derives the "without seconds" variant of a strftime string.
///
/// `%S` is removed together with the separator character before it,
/// fractional-second specifiers (`%f`, `%.f`, `%.3f`, `%3f`, ...) are removed
/// (with a directly preceding `.` or `,`), `%T` and `%X` become `%H:%M`, and
/// `%r` becomes `%I:%M %p`. The composite `%c` and `%+` keep their seconds.
fn short_format(format: &str) -> String {
    let mut out: Vec<Token> = Vec::new();
    for token in tokenize(format) {
        match token {
            Token::Spec(_, 'r') => {
                out.push(Token::Spec("%I".into(), 'I'));
                out.push(Token::Literal(':'));
                out.push(Token::Spec("%M".into(), 'M'));
                out.push(Token::Literal(' '));
                out.push(Token::Spec("%p".into(), 'p'));
            }
            Token::Spec(_, 'T' | 'X') => {
                out.push(Token::Spec("%H".into(), 'H'));
                out.push(Token::Literal(':'));
                out.push(Token::Spec("%M".into(), 'M'));
            }
            Token::Spec(_, 'S') => {
                if matches!(out.last(), Some(Token::Literal(c)) if !c.is_alphanumeric()) {
                    out.pop();
                }
            }
            Token::Spec(_, 'f') => {
                if matches!(out.last(), Some(Token::Literal('.' | ','))) {
                    out.pop();
                }
            }
            other => out.push(other),
        }
    }
    out.iter()
        .map(|token| match token {
            Token::Literal('%') => "%%".to_owned(),
            Token::Literal(c) => c.to_string(),
            Token::Spec(spec, _) => spec.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn instant() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 21, 6, 35, 15).unwrap()
    }

    fn plus_two() -> DisplayTz {
        DisplayTz::Fixed(FixedOffset::east_opt(2 * 3600).unwrap())
    }

    #[test]
    fn format_renders_in_the_display_timezone() {
        let f = TimestampFormatter::new(DEFAULT_TIMESTAMP_FORMAT, false, plus_two());
        assert_eq!(f.format(instant()), "2026-10-21 08:35:15");
    }

    #[test]
    fn format_with_utc_adds_z_suffix() {
        let f = TimestampFormatter::new(DEFAULT_TIMESTAMP_FORMAT, true, plus_two());
        assert_eq!(f.format(instant()), "2026-10-21 06:35:15Z");
    }

    #[test]
    fn format_with_uses_the_display_timezone_and_adds_no_z() {
        let local = TimestampFormatter::new(DEFAULT_TIMESTAMP_FORMAT, false, plus_two());
        assert_eq!(local.format_with(instant(), "%Y%m%d-%H%M"), "20261021-0835");
        let utc = TimestampFormatter::new(DEFAULT_TIMESTAMP_FORMAT, true, plus_two());
        assert_eq!(utc.format_with(instant(), "%Y%m%d-%H%M"), "20261021-0635");
    }

    #[test]
    fn format_short_drops_seconds() {
        let f = TimestampFormatter::new(DEFAULT_TIMESTAMP_FORMAT, false, plus_two());
        assert_eq!(f.format_short(instant()), "2026-10-21 08:35");
        let f = TimestampFormatter::new("%d.%m.%Y %T", false, plus_two());
        assert_eq!(f.format_short(instant()), "21.10.2026 08:35");
        let f = TimestampFormatter::new(DEFAULT_TIMESTAMP_FORMAT, true, plus_two());
        assert_eq!(f.format_short(instant()), "2026-10-21 06:35Z");
    }

    #[test]
    fn short_format_strips_seconds_and_fractions() {
        assert_eq!(short_format("%H:%M:%S"), "%H:%M");
        assert_eq!(short_format("%H:%M:%S%.f"), "%H:%M");
        assert_eq!(short_format("%H:%M:%S%.3f"), "%H:%M");
        assert_eq!(short_format("%H:%M:%S.%3f"), "%H:%M");
        assert_eq!(short_format("%H:%M:%S%6f"), "%H:%M");
        assert_eq!(short_format("%H.%M.%S"), "%H.%M");
        assert_eq!(short_format("%H:%M:%-S"), "%H:%M");
        assert_eq!(short_format("%T"), "%H:%M");
        assert_eq!(short_format("%F %X"), "%F %H:%M");
        assert_eq!(short_format("%r"), "%I:%M %p");
        assert_eq!(short_format("%T%.9f"), "%H:%M");
        assert_eq!(short_format("%Y-%m-%d"), "%Y-%m-%d");
        assert_eq!(short_format("100%% %H:%M:%S"), "100%% %H:%M");
    }

    #[test]
    fn short_format_output_is_always_valid() {
        for f in ["%T", "%H:%M:%S%.3f", "%%S", "%F %T%.f", "%H:%M:%S"] {
            assert!(is_valid_format(&short_format(f)), "{f}");
        }
    }

    #[test]
    fn fractional_seconds_are_rendered_by_format() {
        let f = TimestampFormatter::new("%H:%M:%S%.3f", false, plus_two());
        let t = instant() + chrono::Duration::milliseconds(42);
        assert_eq!(f.format(t), "08:35:15.042");
        assert_eq!(f.format_short(t), "08:35");
    }

    #[test]
    fn format_that_parses_but_cannot_render_is_invalid() {
        assert!(!is_valid_format("%Y %#z"));
        let f = TimestampFormatter::new("%Y %#z", false, plus_two());
        assert_eq!(f.format(instant()), "2026-10-21 08:35:15");
        let f = TimestampFormatter::new("%Y %#z", true, plus_two());
        assert_eq!(f.format(instant()), "2026-10-21 06:35:15Z");
    }

    #[test]
    fn invalid_format_falls_back_to_default() {
        assert!(!is_valid_format("%Q"));
        let f = TimestampFormatter::new("%Q", false, plus_two());
        assert_eq!(f.format(instant()), "2026-10-21 08:35:15");
    }

    #[test]
    fn local_timezone_renders_without_panicking() {
        let f = TimestampFormatter::new(DEFAULT_TIMESTAMP_FORMAT, false, DisplayTz::Local);
        assert_eq!(f.format(instant()).len(), "2026-10-21 08:35:15".len());
    }
}
