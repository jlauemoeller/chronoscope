//! Classification of Claude entries as injected content or noise (SPEC §5.6)
//! and extraction of their human-readable text.

use serde_json::Value;

/// The newest Claude Code `version` the adapter has been checked against
/// (the newest found in the fixtures). Entries with a newer version log one
/// warning per session and are still parsed best-effort.
pub const NEWEST_KNOWN_VERSION: &str = "2.1.314";

/// Attachment kinds that count as injected content.
const INJECTED_ATTACHMENTS: &[&str] = &[
    "hook_system_message",
    "file",
    "directory",
    "opened_file_in_ide",
    "selected_lines_in_ide",
    "queued_command",
    "instructions",
    "plan_mode",
    "plan_mode_exit",
    "compact_file_reference",
];

/// Attachment kinds known to be noise. Any other kind outside the allowlist
/// is still noise, but is counted and logged so the allowlist can be
/// extended.
const NOISE_ATTACHMENTS: &[&str] = &[
    "total_tokens_reminder",
    "skill_listing",
    "credential_org",
    "batching_reminder_sent",
    "environment",
    "date",
    "model",
    "auto_mode",
    "silent_turn_reminder",
    "task_reminder",
    "agent_listing_delta",
    "mcp_instructions_delta",
    "session_context",
    "remote_session_change",
    "advisor_tool",
    "bash_output_audience_note",
    "command_permissions",
    "prompt_snapshot",
    "thinking_drop",
    "session_cron_carry",
    "ultra_effort_enter",
];

/// Entry types that carry no conversation content and are never shown.
const NOISE_TYPES: &[&str] = &[
    "mode",
    "permission-mode",
    "atis-latch",
    "ai-title",
    "last-prompt",
    "cost-state",
];

/// Known session metadata entry types: skipped, logged, not counted.
const METADATA_TYPES: &[&str] = &[
    "agent-name",
    "queue-operation",
    "continued-in",
    "bridge-session",
    "tools-used",
];

/// `system` subtypes that count as injected content.
const INJECTED_SYSTEM: &[&str] = &["local_command", "informational", "away_summary"];

pub(super) fn is_injected_attachment(kind: &str) -> bool {
    INJECTED_ATTACHMENTS.contains(&kind)
}

pub(super) fn is_known_noise_attachment(kind: &str) -> bool {
    NOISE_ATTACHMENTS.contains(&kind) || kind.starts_with("deferred_tools_")
}

pub(super) fn is_noise_type(kind: &str) -> bool {
    NOISE_TYPES.contains(&kind)
        || kind.starts_with("api-request")
        || kind.starts_with("file-history-")
}

pub(super) fn is_metadata_type(kind: &str) -> bool {
    METADATA_TYPES.contains(&kind)
}

pub(super) fn is_injected_system(subtype: &str) -> bool {
    INJECTED_SYSTEM.contains(&subtype)
}

/// Whether `version` is numerically newer than `known`, comparing the
/// dot-separated parts (a part without leading digits counts as 0, missing
/// trailing parts as 0).
pub(super) fn version_newer(version: &str, known: &str) -> bool {
    fn parts(v: &str) -> Vec<u64> {
        v.split('.')
            .map(|p| {
                let digits: String = p.chars().take_while(char::is_ascii_digit).collect();
                digits.parse().unwrap_or(0)
            })
            .collect()
    }
    let (mut a, mut b) = (parts(version), parts(known));
    let len = a.len().max(b.len());
    a.resize(len, 0);
    b.resize(len, 0);
    a > b
}

/// Text of a message-like value: a string as is, an array of strings and
/// `{text}` objects joined by newlines.
pub(super) fn text_of(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| match part {
                Value::String(s) => Some(s.as_str()),
                Value::Object(o) => o.get("text").and_then(Value::as_str),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// `origin.kind` of an attachment (falling back to the entry's own `origin`).
fn is_human_queued(attachment: &Value, entry_origin: Option<&Value>) -> bool {
    attachment
        .get("origin")
        .or(entry_origin)
        .and_then(|o| o.get("kind"))
        .and_then(Value::as_str)
        == Some("human")
}

/// The prompt text of a `queued_command` attachment, when it is a human prompt
/// (typed while the agent was busy); `None` for any other queued command.
pub(super) fn queued_prompt_text(
    attachment: &Value,
    entry_origin: Option<&Value>,
) -> Option<String> {
    is_human_queued(attachment, entry_origin)
        .then(|| attachment.get("prompt").map(text_of).unwrap_or_default())
}

fn str_at<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

/// `head` and `body` on separate lines, skipping an empty one.
fn head_and_body(head: &str, body: &str) -> String {
    match (head.is_empty(), body.is_empty()) {
        (_, true) => head.to_owned(),
        (true, false) => body.to_owned(),
        (false, false) => format!("{head}\n{body}"),
    }
}

/// The readable text of an allowlisted attachment of the given `kind`.
pub(super) fn attachment_text(kind: &str, a: &Value) -> String {
    match kind {
        "file" => {
            let path = str_at(a, "filename")
                .or_else(|| a.pointer("/content/file/filePath").and_then(Value::as_str))
                .unwrap_or_default();
            let body = a
                .pointer("/content/file/content")
                .and_then(Value::as_str)
                .or_else(|| str_at(a, "content"))
                .unwrap_or_default();
            head_and_body(path, body)
        }
        "directory" => head_and_body(
            str_at(a, "path").unwrap_or_default(),
            str_at(a, "content").unwrap_or_default(),
        ),
        "selected_lines_in_ide" => {
            let file = str_at(a, "filename").unwrap_or_default();
            let head = match (
                a.get("lineStart").and_then(Value::as_u64),
                a.get("lineEnd").and_then(Value::as_u64),
            ) {
                (Some(s), Some(e)) if s != e => format!("{file}:{s}-{e}"),
                (Some(s), _) => format!("{file}:{s}"),
                _ => file.to_owned(),
            };
            head_and_body(&head, str_at(a, "content").unwrap_or_default())
        }
        "opened_file_in_ide" | "compact_file_reference" => {
            str_at(a, "filename").unwrap_or_default().to_owned()
        }
        "hook_system_message" => str_at(a, "content")
            .or_else(|| str_at(a, "hookName"))
            .unwrap_or_default()
            .to_owned(),
        "instructions" => match a.get("files").and_then(Value::as_array) {
            Some(files) => files
                .iter()
                .map(|f| {
                    head_and_body(
                        str_at(f, "path").unwrap_or_default(),
                        str_at(f, "content").unwrap_or_default(),
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n"),
            None => a.get("content").map(text_of).unwrap_or_default(),
        },
        "queued_command" => a.get("prompt").map(text_of).unwrap_or_default(),
        "plan_mode" | "plan_mode_exit" => str_at(a, "planFilePath").unwrap_or_default().to_owned(),
        _ => String::new(),
    }
}

/// The command a `system` `local_command` entry ran, as `/command args`, or
/// `None` without a `commandRun` that names one.
pub(super) fn system_command(command_run: Option<&Value>) -> Option<String> {
    let command = command_run
        .and_then(|c| str_at(c, "command"))
        .filter(|c| !c.is_empty())?;
    let args = command_run
        .and_then(|c| str_at(c, "args"))
        .unwrap_or_default();
    Some(if args.is_empty() {
        format!("/{command}")
    } else {
        format!("/{command} {args}")
    })
}

/// The readable text of an injected `system` entry.
///
/// For `local_command` this is the command that was run (`/clear`) followed
/// by its output, when there is any.
pub(super) fn system_text(
    subtype: &str,
    content: Option<&Value>,
    command_run: Option<&Value>,
) -> String {
    let content = content.and_then(Value::as_str).unwrap_or_default();
    if subtype != "local_command" {
        return content.to_owned();
    }
    let Some(command) = system_command(command_run) else {
        return content.to_owned();
    };
    let output = content
        .trim()
        .strip_prefix("<local-command-stdout>")
        .and_then(|s| s.strip_suffix("</local-command-stdout>"))
        .unwrap_or(content)
        .trim();
    head_and_body(&command, output)
}

/// `rest` after the element `<open>...<close>` that it starts with, with the
/// whitespace after it skipped, and the element's trimmed content. `None`
/// when `rest` does not start with `open` or the element is not closed.
fn element<'a>(rest: &'a str, open: &str, close: &str) -> Option<(&'a str, &'a str)> {
    let (content, after) = rest.strip_prefix(open)?.split_once(close)?;
    Some((content.trim(), after.trim_start()))
}

/// The command and arguments of an echo, or `None` when the body is not the
/// echo layout: `<command-name>`, optionally a `<command-message>`, optionally
/// `<command-args>`, with only whitespace between and after them.
fn echo_text(text: &str) -> Option<String> {
    let (name, rest) = element(text, "<command-name>", "</command-name>")?;
    if name.is_empty() {
        return None;
    }
    let rest = element(rest, "<command-message>", "</command-message>").map_or(rest, |(_, r)| r);
    let (args, rest) = if rest.starts_with("<command-args>") {
        element(rest, "<command-args>", "</command-args>")?
    } else {
        ("", rest)
    };
    rest.is_empty().then(|| {
        if args.is_empty() {
            name.to_owned()
        } else {
            format!("{name} {args}")
        }
    })
}

/// The trimmed text inside an output's tags, or `None` when the body is not
/// exactly one such element.
fn output_text(text: &str) -> Option<&str> {
    [
        ("<local-command-stdout>", "</local-command-stdout>"),
        ("<local-command-stderr>", "</local-command-stderr>"),
    ]
    .into_iter()
    .find_map(|(open, close)| element(text, open, close))
    .filter(|(_, after)| after.is_empty())
    .map(|(content, _)| content)
}

/// The readable text of a local command's `user` entry, without its markup
/// (SPEC cycle 3 §2.2).
///
/// An echo (`<command-name>/model</command-name>` and, on the lines after it,
/// `<command-message>model</command-message>` and
/// `<command-args>opus</command-args>`) becomes the command and its
/// arguments, `/model opus`. An output (`<local-command-stdout>` or
/// `<local-command-stderr>`) becomes the trimmed text inside its tags. A body
/// that matches neither form is returned as it is: no closing tag, no name,
/// or any text outside the expected tags (after an output's closing tag, or
/// between or after the echo's tags). The tags are found by their literal
/// names.
pub(super) fn local_command_display_text(raw: &str) -> String {
    let text = raw.trim_start();
    let shown = if text.starts_with("<command-name>") {
        echo_text(text)
    } else {
        output_text(text).map(str::to_owned)
    };
    shown.unwrap_or_else(|| raw.to_owned())
}

/// The command an echo stands for, if `raw` is an echo (its text starts with
/// `<command-name>`): `/model opus`, as [`local_command_display_text`] shows
/// it. An echo with stray text, which is shown as it is, stands for its
/// `<command-name>` alone; with a name that is not closed it is the text after
/// the tag, and with an empty name the body as it is. Only the first line is
/// kept, with tabs as spaces. `None` for an output or anything else.
pub(super) fn echo_command(raw: &str) -> Option<String> {
    let text = raw.trim_start();
    if !text.starts_with("<command-name>") {
        return None;
    }
    let command = echo_text(text)
        .or_else(|| {
            element(text, "<command-name>", "</command-name>")
                .map(|(name, _)| name.to_owned())
                .filter(|name| !name.is_empty())
        })
        .unwrap_or_else(|| unclosed_name(text).unwrap_or(text).to_owned());
    Some(first_line(&command))
}

/// The text after `<command-name>` when the tag is never closed and
/// something follows it.
fn unclosed_name(text: &str) -> Option<&str> {
    let rest = text.strip_prefix("<command-name>")?.trim();
    (!rest.is_empty() && !text.contains("</command-name>")).then_some(rest)
}

/// The first line of `text`, trimmed, with tabs as spaces: a row of the list
/// of local commands, which cannot hold control characters.
pub(super) fn first_line(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or_default()
        .trim()
        .replace('\t', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_numerically_per_part() {
        assert!(version_newer("2.1.315", "2.1.314"));
        assert!(version_newer("2.1.1000", "2.1.314"));
        assert!(version_newer("2.2.0", "2.1.314"));
        assert!(version_newer("3", "2.1.314"));
        assert!(version_newer("2.1.314.1", "2.1.314"));
        assert!(!version_newer("2.1.314", "2.1.314"));
        assert!(!version_newer("2.1.99", "2.1.314"));
        assert!(!version_newer("2.1", "2.1.314"));
        assert!(!version_newer("1.9.9", "2.1.314"));
        assert!(!version_newer("garbage", "2.1.314"));
        assert!(!version_newer("2.1.314-beta", "2.1.314"));
    }

    #[test]
    fn api_request_and_file_history_prefixes_are_noise() {
        for ty in [
            "api-request",
            "api-request-blob",
            "api-request-shape",
            "file-history-snapshot",
            "file-history-delta",
            "mode",
        ] {
            assert!(is_noise_type(ty), "{ty}");
        }
        assert!(!is_noise_type("agent-name"));
        assert!(!is_noise_type("user"));
    }

    const ECHO: &str = "<command-name>/model</command-name>\n            <command-message>model</command-message>\n            <command-args>opus</command-args>";

    #[test]
    fn an_echo_becomes_the_command_and_its_arguments() {
        assert_eq!(local_command_display_text(ECHO), "/model opus");
    }

    #[test]
    fn an_echo_without_arguments_is_the_bare_command() {
        let empty = "<command-name>/effort</command-name>\n            <command-message>effort</command-message>\n            <command-args></command-args>";
        assert_eq!(local_command_display_text(empty), "/effort");
        assert_eq!(
            local_command_display_text("<command-name>/clear</command-name>"),
            "/clear"
        );
        assert_eq!(
            local_command_display_text("  \n<command-name>/clear</command-name>"),
            "/clear"
        );
    }

    #[test]
    fn arguments_are_trimmed() {
        let echo = "<command-name>/model</command-name>\n<command-args>  opus \n</command-args>";
        assert_eq!(local_command_display_text(echo), "/model opus");
    }

    #[test]
    fn an_output_becomes_the_text_inside_its_tags() {
        assert_eq!(
            local_command_display_text("<local-command-stdout>Cancelled</local-command-stdout>"),
            "Cancelled"
        );
        assert_eq!(
            local_command_display_text(
                "<local-command-stderr> Error: nope\n</local-command-stderr>\n"
            ),
            "Error: nope"
        );
        assert_eq!(
            local_command_display_text("<local-command-stdout></local-command-stdout>"),
            ""
        );
    }

    #[test]
    fn a_body_that_matches_neither_form_is_kept_as_it_is() {
        for raw in [
            "<command-name>/model",
            "<command-name></command-name>",
            "<command-name>/model</command-name><command-args>opus",
            "<command-name>/foo</command-name>\nplease also do this",
            "<command-name>/foo</command-name><command-args>a</command-args> dropped text",
            "<command-name>/foo</command-name><command-args>a </command-args> b</command-args>",
            "<command-name>/foo</command-name><command-message>a</command-message> x",
            "<command-name>/foo</command-name><command-message>a",
            "<local-command-stdout>no end",
            "<local-command-stdout>a</local-command-stdout> trailing",
            "<local-command-stdout>a</local-command-stderr>",
            "  <local-command-stdout>a",
            "plain text",
        ] {
            assert_eq!(local_command_display_text(raw), raw, "{raw:?}");
        }
    }

    #[test]
    fn an_echo_stands_for_its_command_and_arguments() {
        let echo = "<command-name>/model</command-name>\n  <command-message>model</command-message>\n  <command-args>opus</command-args>";
        assert_eq!(echo_command(echo).as_deref(), Some("/model opus"));
        assert_eq!(
            echo_command("  <command-name>/clear</command-name>").as_deref(),
            Some("/clear")
        );
    }

    #[test]
    fn an_echo_with_stray_text_stands_for_its_name_alone() {
        let stray = "<command-name>/model</command-name>\n<command-args>opus</command-args>\nstray";
        assert_eq!(echo_command(stray).as_deref(), Some("/model"));
        // A name that is not closed: the text after the tag, first line.
        assert_eq!(
            echo_command("<command-name>/model\nmore").as_deref(),
            Some("/model")
        );
        // An empty name: the body as it is.
        assert_eq!(
            echo_command("<command-name></command-name>\nstray").as_deref(),
            Some("<command-name></command-name>")
        );
        assert_eq!(
            echo_command("<command-name>").as_deref(),
            Some("<command-name>")
        );
        // Tabs are spaces.
        assert_eq!(
            echo_command("<command-name>/tabbed</command-name>\n<command-args>a\tb</command-args>")
                .as_deref(),
            Some("/tabbed a b")
        );
    }

    #[test]
    fn outputs_and_prompts_are_no_echoes() {
        for raw in [
            "<local-command-stdout>Cancelled</local-command-stdout>",
            "<local-command-stderr>x</local-command-stderr>",
            "hello",
            "x <command-name>/a</command-name>",
        ] {
            assert_eq!(echo_command(raw), None, "{raw:?}");
        }
    }

    #[test]
    fn a_system_command_needs_a_named_command_run() {
        let run = |json: &str| serde_json::from_str::<Value>(json).unwrap();
        assert_eq!(
            system_command(Some(&run(r#"{"command":"clear"}"#))).as_deref(),
            Some("/clear")
        );
        assert_eq!(
            system_command(Some(&run(r#"{"command":"model","args":"opus"}"#))).as_deref(),
            Some("/model opus")
        );
        assert_eq!(system_command(Some(&run(r#"{"command":""}"#))), None);
        assert_eq!(system_command(Some(&run(r#"{"args":"x"}"#))), None);
        assert_eq!(system_command(None), None);
    }
}
