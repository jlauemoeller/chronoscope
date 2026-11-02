//! The rows of the details overlay that the model already knows (SPEC
//! §5.3): the Turn group, the duration and status of a tool call, and the
//! timestamp of a prompt, a compaction or an injected entry.
//!
//! Everything else the overlay can show lives in the harness's own source
//! and is read when the overlay opens; it is not part of the model.

use super::content::{DetailsContent, DetailsGroup, DetailsRow};
use crate::components::turn_block::renderers::{format_cost, format_tokens};
use crate::components::turn_block::{SectionContent, SectionId, derive_sections};
use crate::duration::format_duration;
use crate::model::{AssistantTurn, ToolCall, ToolStatus, Turn};
use crate::timestamp::TimestampFormatter;

/// The title of the group about the turn.
pub const TURN: &str = "Turn";
/// The title of the group about a tool call.
pub const TOOL_CALL: &str = "Tool call";
/// The title of the group about a user prompt.
pub const PROMPT: &str = "Prompt";
/// The title of the group about where the section was recorded.
pub const ENVIRONMENT: &str = "Environment";

/// The details of the section `id` of `turn`, from the model alone.
///
/// A `… N more lines` row is its tool call. Returns `None` when `turn` has
/// no such section. Groups come in the order of SPEC §5.3: Turn, Message,
/// Tool call, Prompt, Environment. A group with no rows is left out.
pub fn section_details(
    turn: &Turn,
    id: &SectionId,
    formatter: &TimestampFormatter,
) -> Option<DetailsContent> {
    let id = id.more_parent().unwrap_or_else(|| id.clone());
    // System sections are found whether or not they are shown.
    let sections = derive_sections(turn, true);
    let section = sections.iter().find(|s| s.id == id)?;
    let mut groups = Vec::new();
    if let Turn::Assistant(assistant) = turn {
        groups.push(turn_group(assistant, formatter));
    }
    match section.content {
        SectionContent::Tool(call) => groups.push(tool_group(call)),
        SectionContent::Prompt(_) => groups.push(DetailsGroup::new(
            PROMPT,
            vec![DetailsRow::new(
                "timestamp",
                formatter.format(turn.timestamp()),
            )],
        )),
        SectionContent::Compaction(_) => groups.push(environment(formatter, turn.timestamp())),
        SectionContent::System(entries) => {
            let first = entries
                .first()
                .map_or_else(|| turn.timestamp(), |e| e.timestamp);
            groups.push(environment(formatter, first));
        }
        SectionContent::Text(_)
        | SectionContent::Thinking(_)
        | SectionContent::ModelChange(_)
        | SectionContent::Notice(..)
        | SectionContent::Stats(_) => {}
    }
    Some(DetailsContent::new(groups))
}

/// The Environment group as far as the model goes: the timestamp.
fn environment(
    formatter: &TimestampFormatter,
    time: chrono::DateTime<chrono::Utc>,
) -> DetailsGroup {
    DetailsGroup::new(
        ENVIRONMENT,
        vec![DetailsRow::new("timestamp", formatter.format(time))],
    )
}

/// The Turn group of an Assistant turn: model, effort, start, then what the
/// stats say. Fields that are not known are left out.
fn turn_group(turn: &AssistantTurn, formatter: &TimestampFormatter) -> DetailsGroup {
    let mut rows = Vec::new();
    if !turn.model.is_empty() {
        rows.push(DetailsRow::new("model", turn.model.clone()));
    }
    if let Some(effort) = turn.effort.as_deref().filter(|e| !e.is_empty()) {
        rows.push(DetailsRow::new("effort", effort));
    }
    rows.push(DetailsRow::new("start", formatter.format(turn.timestamp)));
    if let Some(stats) = &turn.stats {
        rows.push(DetailsRow::new("duration", format_duration(stats.duration)));
        if stats.message_count > 0 {
            rows.push(DetailsRow::new("messages", stats.message_count.to_string()));
        }
        if let Some(usage) = &stats.tokens {
            // The model cannot tell zero from not recorded: no zero rows.
            for (label, tokens) in [
                ("input tokens", usage.input),
                ("output tokens", usage.output),
                ("thinking tokens", usage.reasoning),
                ("cache read tokens", usage.cache_read),
                ("cache write tokens", usage.cache_write),
            ] {
                if tokens > 0 {
                    rows.push(DetailsRow::new(label, format_tokens(tokens)));
                }
            }
        }
        if let Some(cost) = stats.cost {
            rows.push(DetailsRow::new("cost", format_cost(cost)));
        }
    }
    DetailsGroup::new(TURN, rows)
}

/// The Tool call group: how long the call took, if known, and its status.
fn tool_group(call: &ToolCall) -> DetailsGroup {
    let mut rows = Vec::new();
    if let Some(duration) = call.duration {
        rows.push(DetailsRow::new("duration", format_duration(duration)));
    }
    let status = match call.status {
        ToolStatus::Pending => "pending",
        ToolStatus::Ok => "ok",
        ToolStatus::Error => "error",
    };
    rows.push(DetailsRow::new("status", status));
    DetailsGroup::new(TOOL_CALL, rows)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;

    use super::*;
    use crate::components::turn_block::SectionKey;
    use crate::components::turn_block::fixtures::{
        assistant, compaction, formatter, injected, item_id, key_id, notice, stats, text, thinking,
        tool, user, with_duration,
    };
    use crate::model::{
        AssistantTurn, ItemId, NoticeLevel, Thinking, TokenUsage, ToolStatus, TurnStats,
    };

    fn groups(content: &DetailsContent) -> Vec<&str> {
        content.groups().iter().map(|g| g.title.as_str()).collect()
    }

    fn rows<'a>(content: &'a DetailsContent, group: &str) -> Vec<(&'a str, &'a str)> {
        content
            .groups()
            .iter()
            .find(|g| g.title == group)
            .unwrap_or_else(|| panic!("no {group} group in {content:?}"))
            .rows
            .iter()
            .map(|r| (r.label.as_str(), r.value.as_str()))
            .collect()
    }

    fn details(turn: &Turn, id: &SectionId) -> DetailsContent {
        section_details(turn, id, &formatter()).expect("the section exists")
    }

    fn full_stats() -> TurnStats {
        TurnStats {
            duration: Duration::from_secs(243),
            message_count: 12,
            tokens: Some(TokenUsage {
                input: 2_800,
                output: 42_400,
                cache_read: 359_400_000,
                cache_write: 1_200,
                reasoning: 9_000,
            }),
            cost: Some(0.4),
        }
    }

    fn call_turn(stats: Option<TurnStats>) -> Turn {
        assistant(
            Some("high"),
            vec![
                text("t1", "Hello"),
                with_duration(
                    tool(
                        "c1",
                        "Run ls",
                        json!({"command": "ls"}),
                        Some(("ok", false)),
                        ToolStatus::Ok,
                    ),
                    62_000,
                ),
            ],
            stats,
        )
    }

    #[test]
    fn the_turn_group_lists_every_known_field_in_order() {
        let turn = call_turn(Some(full_stats()));
        let content = details(&turn, &item_id(&turn, "t1"));
        assert_eq!(groups(&content), ["Turn"]);
        assert_eq!(
            rows(&content, "Turn"),
            [
                ("model", "claude-opus-5.5"),
                ("effort", "high"),
                ("start", "2026-10-21 08:35:18Z"),
                ("duration", "4m 3s"),
                ("messages", "12"),
                ("input tokens", "2.8k"),
                ("output tokens", "42.4k"),
                ("thinking tokens", "9.0k"),
                ("cache read tokens", "359.4M"),
                ("cache write tokens", "1.2k"),
                ("cost", "$0.40"),
            ]
        );
    }

    #[test]
    fn a_missing_field_is_left_out() {
        let mut stats = full_stats();
        stats.cost = None;
        stats.tokens = Some(TokenUsage {
            input: 10,
            ..TokenUsage::default()
        });
        let turn = assistant(None, vec![text("t1", "Hello")], Some(stats));
        let content = details(&turn, &item_id(&turn, "t1"));
        assert_eq!(
            rows(&content, "Turn"),
            [
                ("model", "claude-opus-5.5"),
                ("start", "2026-10-21 08:35:18Z"),
                ("duration", "4m 3s"),
                ("messages", "12"),
                ("input tokens", "10"),
            ],
            "no effort, no zero token rows, no cost"
        );
    }

    #[test]
    fn there_is_no_cost_row_without_a_cost() {
        let mut stats = full_stats();
        stats.cost = None;
        let turn = assistant(Some("high"), vec![text("t1", "Hello")], Some(stats));
        let content = details(&turn, &item_id(&turn, "t1"));
        assert!(
            rows(&content, "Turn")
                .iter()
                .all(|(label, _)| *label != "cost"),
            "{content:?}"
        );
    }

    #[test]
    fn a_zero_message_count_and_unknown_tokens_are_left_out() {
        let stats = TurnStats {
            duration: Duration::from_secs(5),
            message_count: 0,
            tokens: None,
            cost: None,
        };
        let turn = assistant(None, vec![text("t1", "Hello")], Some(stats));
        let content = details(&turn, &item_id(&turn, "t1"));
        assert_eq!(
            rows(&content, "Turn"),
            [
                ("model", "claude-opus-5.5"),
                ("start", "2026-10-21 08:35:18Z"),
                ("duration", "5s"),
            ]
        );
    }

    #[test]
    fn a_turn_without_stats_still_has_model_effort_and_start() {
        let turn = assistant(Some("low"), vec![text("t1", "Hello")], None);
        let content = details(&turn, &item_id(&turn, "t1"));
        assert_eq!(
            rows(&content, "Turn"),
            [
                ("model", "claude-opus-5.5"),
                ("effort", "low"),
                ("start", "2026-10-21 08:35:18Z"),
            ]
        );
    }

    #[test]
    fn an_empty_model_is_left_out() {
        let mut turn = AssistantTurn {
            model: String::new(),
            ..match assistant(None, vec![text("t1", "Hello")], None) {
                Turn::Assistant(a) => a,
                _ => unreachable!(),
            }
        };
        turn.effort = Some(String::new());
        let turn = Turn::Assistant(turn);
        let content = details(&turn, &item_id(&turn, "t1"));
        assert_eq!(rows(&content, "Turn"), [("start", "2026-10-21 08:35:18Z")]);
    }

    #[test]
    fn the_timestamps_use_the_configured_format() {
        let turn = assistant(None, vec![text("t1", "Hello")], None);
        let local = crate::timestamp::TimestampFormatter::new(
            "%H:%M",
            true,
            crate::timestamp::DisplayTz::Local,
        );
        let content = section_details(&turn, &item_id(&turn, "t1"), &local).unwrap();
        assert_eq!(rows(&content, "Turn")[1], ("start", "08:35Z"));
    }

    #[test]
    fn the_turn_group_is_on_every_section_of_an_assistant_turn() {
        let turn = assistant(
            Some("high"),
            vec![
                text("t1", "Hello"),
                thinking("th1", Thinking::Text("hm".into())),
                tool(
                    "c1",
                    "Run ls",
                    json!({"command": "ls"}),
                    None,
                    ToolStatus::Pending,
                ),
                crate::model::Item::ModelChange {
                    id: ItemId::from("m1"),
                    model: "other".into(),
                },
                notice("n1", NoticeLevel::Error, "Boom"),
                injected("i1", &[("hook_system_message", "x")]),
            ],
            Some(stats()),
        );
        let mut ids: Vec<SectionId> = ["t1", "th1", "c1", "m1", "n1", "i1"]
            .iter()
            .map(|item| item_id(&turn, item))
            .collect();
        ids.push(key_id(&turn, SectionKey::Footer));
        for id in ids {
            let content = details(&turn, &id);
            assert_eq!(content.groups()[0].title, "Turn", "{id:?}");
            assert_eq!(rows(&content, "Turn")[0].0, "model", "{id:?}");
        }
    }

    #[test]
    fn a_users_sections_have_no_turn_group() {
        let turn = user("Hello", vec![injected("i1", &[("file", "x")])]);
        for id in [key_id(&turn, SectionKey::Prompt), item_id(&turn, "i1")] {
            let content = details(&turn, &id);
            assert!(!groups(&content).contains(&"Turn"), "{id:?}");
        }
    }

    #[test]
    fn a_compaction_has_no_turn_group_but_its_timestamp() {
        let turn = compaction("Summary");
        let content = details(&turn, &key_id(&turn, SectionKey::Summary));
        assert_eq!(groups(&content), ["Environment"]);
        assert_eq!(
            rows(&content, "Environment"),
            [("timestamp", "2026-10-21 09:00:00Z")]
        );
    }

    #[test]
    fn a_prompt_has_its_timestamp() {
        let turn = user("Hello", Vec::new());
        let content = details(&turn, &key_id(&turn, SectionKey::Prompt));
        assert_eq!(groups(&content), ["Prompt"]);
        assert_eq!(
            rows(&content, "Prompt"),
            [("timestamp", "2026-10-21 08:35:15Z")]
        );
    }

    #[test]
    fn a_system_section_has_its_first_entrys_timestamp_under_environment() {
        let turn = user(
            "Hello",
            vec![injected("i1", &[("file", "x"), ("meta", "y")])],
        );
        let content = details(&turn, &item_id(&turn, "i1"));
        assert_eq!(groups(&content), ["Environment"]);
        assert_eq!(
            rows(&content, "Environment"),
            [("timestamp", "2026-10-21 08:35:10Z")]
        );
    }

    #[test]
    fn a_system_section_of_an_assistant_turn_has_the_turn_and_environment_groups() {
        let turn = assistant(None, vec![injected("i1", &[("file", "x")])], None);
        let content = details(&turn, &item_id(&turn, "i1"));
        assert_eq!(groups(&content), ["Turn", "Environment"]);
    }

    #[test]
    fn a_tool_call_has_its_duration_and_status_after_the_turn() {
        let turn = call_turn(Some(full_stats()));
        let content = details(&turn, &item_id(&turn, "c1"));
        assert_eq!(groups(&content), ["Turn", "Tool call"]);
        assert_eq!(
            rows(&content, "Tool call"),
            [("duration", "1m 2s"), ("status", "ok")]
        );
    }

    #[test]
    fn a_pending_call_has_no_duration_and_an_error_call_says_error() {
        let turn = assistant(
            None,
            vec![
                tool("c1", "Run", json!({}), None, ToolStatus::Pending),
                tool("c2", "Run", json!({}), Some(("x", true)), ToolStatus::Error),
            ],
            None,
        );
        let pending = details(&turn, &item_id(&turn, "c1"));
        assert_eq!(rows(&pending, "Tool call"), [("status", "pending")]);
        let failed = details(&turn, &item_id(&turn, "c2"));
        assert_eq!(rows(&failed, "Tool call"), [("status", "error")]);
    }

    #[test]
    fn a_more_row_is_its_tool_call() {
        let turn = call_turn(None);
        let call = item_id(&turn, "c1");
        let more = call.more_row().unwrap();
        assert_eq!(details(&turn, &more), details(&turn, &call));
        assert_eq!(groups(&details(&turn, &more)), ["Turn", "Tool call"]);
    }

    #[test]
    fn the_groups_come_in_the_order_of_the_table() {
        let turn = call_turn(None);
        let content = details(&turn, &item_id(&turn, "c1"));
        let order = ["Turn", "Message", "Tool call", "Prompt", "Environment"];
        let at = |title: &str| order.iter().position(|t| *t == title).unwrap();
        let titles = groups(&content);
        assert!(titles.windows(2).all(|w| at(w[0]) < at(w[1])), "{titles:?}");
    }

    #[test]
    fn an_unknown_section_has_no_details() {
        let turn = call_turn(None);
        let other = SectionId {
            turn: turn.id().clone(),
            key: SectionKey::Item(ItemId::from("nope")),
        };
        assert_eq!(section_details(&turn, &other, &formatter()), None);
        // A prompt key on an assistant turn is no section either.
        let prompt = key_id(&turn, SectionKey::Prompt);
        assert_eq!(section_details(&turn, &prompt, &formatter()), None);
    }
}
