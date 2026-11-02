//! The per-file scan behind session discovery (SPEC §3.1, §9.2).
//!
//! A scan reads one transcript once, front to back, and keeps only what the
//! session list needs. Lines that are not valid entries are skipped, so a
//! half-written last line or a damaged line never fails the scan.

use std::io::{self, BufRead};

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

use super::inject::queued_prompt_text;
use super::wire::read_entry;

/// The first prompt is kept up to this many characters; the list only needs
/// its first line.
const MAX_PROMPT_CHARS: usize = 4096;

/// What a scan keeps of one transcript.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct FileScan {
    /// The `cwd` of the first entry that has one.
    pub cwd: Option<String>,
    /// The timestamp of the first entry that has one.
    pub started_at: Option<DateTime<Utc>>,
    /// The text of the first human prompt (blank prompts do not count).
    pub first_prompt: Option<String>,
    /// The last non-empty `ai-title`.
    pub ai_title: Option<String>,
    /// The timestamp of the last entry that has one.
    pub last_timestamp: Option<DateTime<Utc>>,
}

/// The fields every line is classified by. Apart from `type`, they are read
/// as loose JSON values, so an unexpected type loses that one field, not the
/// line.
#[derive(Deserialize)]
struct ScanHead {
    #[serde(rename = "type")]
    kind: Option<String>,
    timestamp: Option<Value>,
    cwd: Option<Value>,
    #[serde(rename = "aiTitle")]
    ai_title: Option<Value>,
    attachment: Option<Value>,
}

/// Scans a transcript, reading `reader` once to the end.
pub(super) fn scan_reader<R: BufRead>(mut reader: R) -> io::Result<FileScan> {
    let mut scan = FileScan::default();
    let mut line = Vec::new();
    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line)? == 0 {
            return Ok(scan);
        }
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let Ok(head) = serde_json::from_slice::<ScanHead>(&line) else {
            continue;
        };
        let kind = head.kind.as_deref().unwrap_or_default();

        if let Some(ts) = head
            .timestamp
            .as_ref()
            .and_then(Value::as_str)
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|t| t.with_timezone(&Utc))
        {
            scan.started_at.get_or_insert(ts);
            scan.last_timestamp = Some(ts);
        }
        if scan.cwd.is_none()
            && let Some(cwd) = head.cwd.as_ref().and_then(Value::as_str)
            && !cwd.is_empty()
        {
            scan.cwd = Some(cwd.to_owned());
        }
        if kind == "ai-title"
            && let Some(title) = head.ai_title.as_ref().and_then(Value::as_str)
            && !title.trim().is_empty()
        {
            scan.ai_title = Some(title.trim().to_owned());
        }
        if scan.first_prompt.is_none() {
            let text = match kind {
                "user" => prompt_of_user(&line),
                "attachment" if is_queued_command(head.attachment.as_ref()) => {
                    prompt_of_queued(&line)
                }
                _ => None,
            };
            scan.first_prompt = text
                .filter(|t| !t.trim().is_empty())
                .map(|t| t.chars().take(MAX_PROMPT_CHARS).collect());
        }
    }
}

fn is_queued_command(attachment: Option<&Value>) -> bool {
    attachment.and_then(|a| a.get("type")?.as_str()) == Some("queued_command")
}

fn prompt_of_user(line: &[u8]) -> Option<String> {
    read_entry(line).ok()?.human_prompt_text()
}

fn prompt_of_queued(line: &[u8]) -> Option<String> {
    let entry = read_entry(line).ok()?;
    queued_prompt_text(entry.attachment.as_ref()?, entry.origin.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(text: &str) -> FileScan {
        scan_reader(text.as_bytes()).expect("in-memory reads do not fail")
    }

    fn ts(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn empty_input_scans_to_nothing() {
        assert_eq!(scan(""), FileScan::default());
    }

    #[test]
    fn cwd_and_start_come_from_the_first_entry_that_has_them() {
        let s = scan(concat!(
            "{\"type\":\"mode\",\"mode\":\"normal\"}\n",
            "{\"type\":\"user\",\"cwd\":\"/a\",\"timestamp\":\"2026-01-01T00:00:00Z\",",
            "\"message\":{\"content\":\"hi\"}}\n",
            "{\"type\":\"assistant\",\"cwd\":\"/b\",\"timestamp\":\"2026-01-02T00:00:00Z\"}\n",
        ));
        assert_eq!(s.cwd.as_deref(), Some("/a"));
        assert_eq!(s.started_at, Some(ts("2026-01-01T00:00:00Z")));
        assert_eq!(s.last_timestamp, Some(ts("2026-01-02T00:00:00Z")));
    }

    #[test]
    fn last_non_empty_ai_title_wins() {
        let s = scan(concat!(
            "{\"type\":\"ai-title\",\"aiTitle\":\"one\"}\n",
            "{\"type\":\"ai-title\",\"aiTitle\":\"two\"}\n",
            "{\"type\":\"ai-title\",\"aiTitle\":\"\"}\n",
        ));
        assert_eq!(s.ai_title.as_deref(), Some("two"));
    }

    #[test]
    fn malformed_and_partial_lines_are_skipped() {
        let s = scan(concat!(
            "not json\n",
            "{\"type\":\"ai-title\",\"aiTitle\":\"kept\"}\n",
            "{\"type\":\"user\",\"cwd\":\"/half",
        ));
        assert_eq!(s.ai_title.as_deref(), Some("kept"));
        assert_eq!(s.cwd, None);
    }

    #[test]
    fn a_prompt_is_found_on_a_line_with_invalid_utf8_in_usage() {
        let bytes = b"{\"type\":\"user\",\"origin\":{\"kind\":\"human\"},\"message\":{\"content\":\"real\",\"usage\":{\"service_tier\":\"\xff\"}}}\n";
        let s = scan_reader(&bytes[..]).unwrap();
        assert_eq!(s.first_prompt.as_deref(), Some("real"));
    }

    #[test]
    fn invalid_utf8_does_not_end_the_scan() {
        let mut bytes = b"{\"type\":\"x\",\"cwd\":\"\xff\xfe\"}\n".to_vec();
        bytes.extend_from_slice(b"{\"type\":\"ai-title\",\"aiTitle\":\"after\"}\n");
        let s = scan_reader(&bytes[..]).unwrap();
        assert_eq!(s.ai_title.as_deref(), Some("after"));
    }

    #[test]
    fn first_prompt_skips_tool_results_meta_and_origins_other_than_human() {
        let s = scan(concat!(
            "{\"type\":\"user\",\"message\":{\"content\":[{\"type\":\"tool_result\",",
            "\"tool_use_id\":\"t\",\"content\":\"x\"}]}}\n",
            "{\"type\":\"user\",\"isMeta\":true,\"message\":{\"content\":\"meta\"}}\n",
            "{\"type\":\"user\",\"origin\":{\"kind\":\"task-notification\"},",
            "\"message\":{\"content\":\"note\"}}\n",
            "{\"type\":\"user\",\"message\":{\"content\":\"   \"}}\n",
            "{\"type\":\"user\",\"origin\":{\"kind\":\"human\"},",
            "\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"real\"}]}}\n",
            "{\"type\":\"user\",\"message\":{\"content\":\"later\"}}\n",
        ));
        assert_eq!(s.first_prompt.as_deref(), Some("real"));
    }

    #[test]
    fn a_non_object_attachment_does_not_lose_the_line() {
        let s = scan(
            "{\"type\":\"attachment\",\"cwd\":\"/a\",\"timestamp\":\"2026-01-01T00:00:00Z\",\"attachment\":\"odd\"}\n",
        );
        assert_eq!(s.cwd.as_deref(), Some("/a"));
        assert!(s.started_at.is_some());
    }

    #[test]
    fn human_queued_command_counts_as_a_prompt() {
        let s = scan(concat!(
            "{\"type\":\"attachment\",\"attachment\":{\"type\":\"queued_command\",",
            "\"prompt\":\"robot\",\"origin\":{\"kind\":\"task-notification\"}}}\n",
            "{\"type\":\"attachment\",\"attachment\":{\"type\":\"queued_command\",",
            "\"prompt\":\"typed\",\"origin\":{\"kind\":\"human\"}}}\n",
        ));
        assert_eq!(s.first_prompt.as_deref(), Some("typed"));
    }

    #[test]
    fn long_prompts_are_capped_on_a_char_boundary() {
        let long = "é".repeat(MAX_PROMPT_CHARS + 10);
        let s = scan(&format!(
            "{{\"type\":\"user\",\"message\":{{\"content\":\"{long}\"}}}}\n"
        ));
        assert_eq!(s.first_prompt.unwrap().chars().count(), MAX_PROMPT_CHARS);
    }
}
