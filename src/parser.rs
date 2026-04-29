use std::str::FromStr;

use croner::Cron;
use thiserror::Error;
use unicode_properties::UnicodeEmoji;
use unicode_segmentation::UnicodeSegmentation;

const DEFAULT_ICON: &str = "🔔";

/// A single parsed reminder. The original schedule string is kept alongside
/// the parsed `Cron` so it can be shown verbatim in the main window.
#[derive(Debug)]
pub struct Reminder {
    // `cron` is read by the scheduler (build-order step 5); silence dead-code
    // until then so step 4 can land cleanly under `-D warnings`.
    #[allow(dead_code)]
    pub cron: Cron,
    pub schedule: String,
    pub icon: String,
    pub body: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseLineError {
    #[error("missing '|' separator")]
    MissingSeparator,
    #[error("invalid cron expression: {0}")]
    InvalidCron(String),
    #[error("empty message")]
    EmptyMessage,
}

/// Result of parsing an entire reminders file.
#[derive(Debug, Default)]
pub struct ParseReport {
    pub reminders: Vec<Reminder>,
    /// `(1-based line number, error)` for each line that failed to parse.
    pub errors: Vec<(usize, ParseLineError)>,
}

/// Parse a reminders file. Comments and blank lines are silently skipped;
/// invalid lines collect into [`ParseReport::errors`] without aborting.
pub fn parse(input: &str) -> ParseReport {
    let mut report = ParseReport::default();
    for (i, raw) in input.lines().enumerate() {
        let lineno = i + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        match parse_line(trimmed) {
            Ok(r) => report.reminders.push(r),
            Err(e) => report.errors.push((lineno, e)),
        }
    }
    report
}

/// Parse a single non-empty, non-comment line. Caller is responsible for
/// having already stripped comments and blanks.
pub fn parse_line(line: &str) -> Result<Reminder, ParseLineError> {
    let (schedule_part, message_part) = line
        .split_once('|')
        .ok_or(ParseLineError::MissingSeparator)?;

    let schedule = schedule_part.trim().to_string();
    let message = message_part.trim();
    if message.is_empty() {
        return Err(ParseLineError::EmptyMessage);
    }

    let cron = Cron::from_str(&schedule).map_err(|e| ParseLineError::InvalidCron(e.to_string()))?;

    let (icon, body) = extract_icon(message);
    Ok(Reminder {
        cron,
        schedule,
        icon,
        body,
    })
}

/// Split a non-empty trimmed message into `(icon, body)`. If the first
/// grapheme cluster is an emoji it becomes the icon and the rest of the
/// message (left-trimmed) becomes the body. Otherwise the icon falls back
/// to [`DEFAULT_ICON`] and the body is the message unchanged.
fn extract_icon(message: &str) -> (String, String) {
    let mut graphemes = message.graphemes(true);
    if let Some(first) = graphemes.next() {
        let is_emoji = first
            .chars()
            .next()
            .is_some_and(UnicodeEmoji::is_emoji_char);
        if is_emoji {
            let body = graphemes.as_str().trim_start().to_string();
            return (first.to_string(), body);
        }
    }
    (DEFAULT_ICON.to_string(), message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_line_with_emoji() {
        let r = parse_line("*/30 9-17 * * 1-5 | 💧 Drink water").expect("parses");
        assert_eq!(r.schedule, "*/30 9-17 * * 1-5");
        assert_eq!(r.icon, "💧");
        assert_eq!(r.body, "Drink water");
    }

    #[test]
    fn valid_line_plain_uses_default_icon() {
        let r = parse_line("*/15 * * * * | Plain reminder").expect("parses");
        assert_eq!(r.icon, DEFAULT_ICON);
        assert_eq!(r.body, "Plain reminder");
    }

    #[test]
    fn missing_separator_is_rejected() {
        assert!(matches!(
            parse_line("0 9 * * 1 Weekly review"),
            Err(ParseLineError::MissingSeparator)
        ));
    }

    #[test]
    fn invalid_cron_is_rejected() {
        let err = parse_line("not a cron | hi").expect_err("rejects");
        assert!(matches!(err, ParseLineError::InvalidCron(_)), "got {err:?}");
    }

    #[test]
    fn empty_message_is_rejected() {
        assert!(matches!(
            parse_line("0 9 * * 1 |    "),
            Err(ParseLineError::EmptyMessage)
        ));
    }

    #[test]
    fn multi_codepoint_grapheme_is_preserved() {
        // Family ZWJ sequence: man + ZWJ + woman + ZWJ + girl.
        let r = parse_line("0 0 * * * | 👨\u{200D}👩\u{200D}👧 Family time").expect("parses");
        assert_eq!(r.icon, "👨\u{200D}👩\u{200D}👧");
        assert_eq!(r.body, "Family time");
    }

    #[test]
    fn pipe_in_message_is_preserved() {
        // Split is on the first '|'; later pipes belong to the body.
        let r = parse_line("0 9 * * 1 | review | follow up").expect("parses");
        assert_eq!(r.body, "review | follow up");
    }

    #[test]
    fn parse_skips_comments_and_blanks() {
        let input = "
# this is a comment

0 9 * * 1 | Weekly review
   # indented comment

*/30 * * * * | Hydrate
";
        let report = parse(input);
        assert_eq!(report.reminders.len(), 2);
        assert!(report.errors.is_empty());
        assert_eq!(report.reminders[0].body, "Weekly review");
        assert_eq!(report.reminders[1].body, "Hydrate");
    }

    #[test]
    fn parse_collects_errors_with_line_numbers() {
        let input = "0 9 * * 1 | ok\nbroken\n0 9 * * 1 | also ok\nnot a cron | x";
        let report = parse(input);
        assert_eq!(report.reminders.len(), 2);
        assert_eq!(report.errors.len(), 2);
        assert_eq!(report.errors[0].0, 2);
        assert!(matches!(
            report.errors[0].1,
            ParseLineError::MissingSeparator
        ));
        assert_eq!(report.errors[1].0, 4);
        assert!(matches!(report.errors[1].1, ParseLineError::InvalidCron(_)));
    }
}
