//! Fixture turns and drawing helpers shared by the tests.

use std::time::Duration;

use chrono::{DateTime, Utc};
use ratatui::buffer::Buffer;
use ratatui::widgets::Widget;
use serde_json::json;

use super::*;
use crate::model::{
    AssistantTurn, CompactionTurn, InjectedEntry, Item, ItemId, Thinking, ToolCall, ToolKind,
    ToolOutput, ToolResult, ToolStatus, Turn, TurnId, TurnStats, UserTurn,
};
use crate::test_support::buffer_to_text;
use crate::theme::{Role, Theme, Token, TokenOverride, builtin};
use crate::timestamp::{DEFAULT_TIMESTAMP_FORMAT, DisplayTz, TimestampFormatter};

pub const LONG_PROMPT: &str = "I have a Logitech BRIO web camera attached to this machine via USB. It works fine - both camera and microphone - but I don't have a way to access settings that would have been available on say a mac or windows, since there are no proprietary drivers: we just get the vanilla USB profiles. Can you help me investigate what additional profiles the camera has and how to access them? The ideal solution would be to be able to write an app that allowed me to change the settings, if possible.";

pub fn ts(time: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(time)
        .unwrap()
        .with_timezone(&Utc)
}

pub fn formatter() -> TimestampFormatter {
    TimestampFormatter::new(DEFAULT_TIMESTAMP_FORMAT, true, DisplayTz::Local)
}

pub fn theme() -> Theme {
    builtin::default_theme()
}

/// A theme whose tokens all differ from `muted_foreground` and from each
/// other, so that a colour identifies the token it came from.
pub fn distinct_theme() -> Theme {
    let base = builtin::default_theme();
    let fg = |role| TokenOverride::fg(base.role(role));
    base.clone()
        .with_token(Token::UserTitle, fg(Role::Primary))
        .with_token(Token::AssistantTitle, fg(Role::Accent))
        .with_token(Token::SystemTitle, fg(Role::Warning))
        .with_token(Token::Timestamp, fg(Role::Destructive))
        .with_token(Token::ToolDescription, fg(Role::Primary))
        .with_token(Token::Label, fg(Role::Warning))
        .with_token(Token::Command, fg(Role::Accent))
        .with_token(Token::Output, fg(Role::Secondary))
        .with_token(Token::ErrorOutput, fg(Role::Destructive))
        .with_token(Token::Thinking, fg(Role::PrimaryForeground))
        .with_token(Token::SelectionMarker, fg(Role::Ring))
}

pub fn tool(
    id: &str,
    description: &str,
    input: serde_json::Value,
    result: Option<(&str, bool)>,
    status: ToolStatus,
) -> Item {
    Item::ToolCall(ToolCall {
        id: ItemId::from(id),
        tool: ToolKind::Shell,
        description: description.to_owned(),
        input,
        result: result.map(|(text, is_error)| ToolResult {
            output: ToolOutput::inline(text.to_owned()),
            is_error,
        }),
        status,
        duration: None,
    })
}

pub fn text(id: &str, text: &str) -> Item {
    Item::Text {
        id: ItemId::from(id),
        text: text.to_owned(),
    }
}

pub fn thinking(id: &str, thinking: Thinking) -> Item {
    Item::Thinking {
        id: ItemId::from(id),
        thinking,
    }
}

pub fn injected(id: &str, entries: &[(&str, &str)]) -> Item {
    Item::Injected {
        id: ItemId::from(id),
        entries: entries
            .iter()
            .map(|(kind, text)| InjectedEntry {
                kind: (*kind).to_owned(),
                timestamp: ts("2026-10-21T08:35:10Z"),
                text: (*text).to_owned(),
            })
            .collect(),
    }
}

pub fn user(text: &str, items: Vec<Item>) -> Turn {
    Turn::User(UserTurn {
        id: TurnId::from("u1"),
        timestamp: ts("2026-10-21T08:35:15.800Z"),
        text: text.to_owned(),
        items,
    })
}

pub fn assistant(effort: Option<&str>, items: Vec<Item>, stats: Option<TurnStats>) -> Turn {
    Turn::Assistant(AssistantTurn {
        id: TurnId::from("a1"),
        timestamp: ts("2026-10-21T08:35:18.271Z"),
        model: "claude-opus-5.5".to_owned(),
        effort: effort.map(str::to_owned),
        items,
        stats,
    })
}

pub fn compaction(summary: &str) -> Turn {
    Turn::Compaction(CompactionTurn {
        id: TurnId::from("c1"),
        timestamp: ts("2026-10-21T09:00:00Z"),
        summary: summary.to_owned(),
    })
}

pub fn stats() -> TurnStats {
    TurnStats {
        duration: Duration::from_millis(302_646),
        message_count: 112,
        tokens: Some(45_213),
        cost: None,
    }
}

pub const SCRATCH: &str =
    "/tmp/claude-1000/-home-jacob-Development-projects-brio/scratchpad/lsusb.txt";

/// The Assistant turn of the full snapshot: text with a list and inline
/// code, folded thinking, a folded and an unfolded tool call, a model change
/// and the stats footer.
pub fn full_assistant() -> Turn {
    assistant(
        Some("high"),
        vec![
            text(
                "t1",
                "I'll start by looking at what the camera exposes over USB:\n\n- Check `lsusb -v` for the UVC descriptors\n- Look at `v4l2-ctl --list-ctrls`\n\nThen I'll compare the results.",
            ),
            thinking(
                "th1",
                Thinking::Text("The user wants the UVC extension units.\nStart with lsusb.".into()),
            ),
            tool(
                "c1",
                "Find the BRIO on USB and check available tools",
                json!({"command": "lsusb | grep -i brio"}),
                Some((
                    "Bus 001 Device 004: ID 046d:085e Logitech, Inc. BRIO",
                    false,
                )),
                ToolStatus::Ok,
            ),
            tool(
                "c2",
                "Dump full USB descriptors for the BRIO",
                json!({"command": format!("sudo -n true 2>&1 && echo SUDO_OK; lsusb -v -d 046d:085e > {SCRATCH} 2>&1; wc -l {SCRATCH}")}),
                Some((
                    &format!("sudo: a password is required\n1799\n{SCRATCH}"),
                    false,
                )),
                ToolStatus::Ok,
            ),
            Item::ModelChange {
                id: ItemId::from("m1"),
                model: "claude-sonnet-5.5".to_owned(),
            },
        ],
        Some(stats()),
    )
}

pub fn item_id(turn: &Turn, item: &str) -> SectionId {
    SectionId {
        turn: turn.id().clone(),
        key: SectionKey::Item(ItemId::from(item)),
    }
}

pub fn key_id(turn: &Turn, key: SectionKey) -> SectionId {
    SectionId {
        turn: turn.id().clone(),
        key,
    }
}

/// How a test draws a block.
#[derive(Clone)]
pub struct Opts {
    pub folds: FoldState,
    pub selected: Option<SectionId>,
    pub show_system: bool,
    pub dim: bool,
    pub theme: Theme,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            folds: FoldState::new(),
            selected: None,
            show_system: true,
            dim: true,
            theme: theme(),
        }
    }
}

impl Opts {
    pub fn unfold(mut self, id: SectionId) -> Self {
        self.folds.set_folded(id, false);
        self
    }

    pub fn select(mut self, id: SectionId) -> Self {
        self.selected = Some(id);
        self
    }

    pub fn dim(mut self, dim: bool) -> Self {
        self.dim = dim;
        self
    }

    pub fn themed(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

/// Runs `f` with the block for `turn` built from `opts`.
pub fn with_block<R>(turn: &Turn, opts: &Opts, f: impl FnOnce(TurnBlock<'_>) -> R) -> R {
    let formatter = formatter();
    let block = TurnBlock::new(turn, &opts.theme, &formatter)
        .folds(&opts.folds)
        .selected(opts.selected.as_ref())
        .show_system(opts.show_system)
        .dim_unselected(opts.dim);
    f(block)
}

pub fn height(turn: &Turn, opts: &Opts, width: u16) -> usize {
    with_block(turn, opts, |block| block.height(width))
}

/// Draws the block into a buffer of exactly its height and checks that the
/// height is the number of rows drawn: the last row is the bottom border,
/// no earlier row is, and a taller buffer gets nothing below the block.
pub fn draw(turn: &Turn, opts: &Opts, width: u16) -> Buffer {
    let h = height(turn, opts, width);
    let rows = u16::try_from(h).unwrap();
    let exact = render_into(turn, opts, width, rows, 0);
    let text = buffer_to_text(&exact);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), h);
    assert!(lines[h - 1].starts_with('╰') && lines[h - 1].ends_with('╯'));
    assert!(lines[..h - 1].iter().all(|l| !l.starts_with('╰')));
    assert!(lines[0].starts_with('╭') && lines[0].ends_with('╮'));
    let taller = buffer_to_text(&render_into(turn, opts, width, rows + 2, 0));
    let taller: Vec<&str> = taller.lines().collect();
    assert_eq!(taller[..h], lines[..]);
    assert!(taller[h..].iter().all(|l| l.trim().is_empty()));
    exact
}

/// Draws the block with `skip` rows skipped into a `height`-row buffer.
pub fn render_into(turn: &Turn, opts: &Opts, width: u16, height: u16, skip: usize) -> Buffer {
    with_block(turn, opts, |block| {
        crate::test_support::render(width, height, |frame| {
            block
                .skip_rows(skip)
                .render(frame.area(), frame.buffer_mut());
        })
    })
}

pub fn rows(buf: &Buffer) -> Vec<String> {
    buffer_to_text(buf).lines().map(str::to_owned).collect()
}

/// The text of columns `from..to` of a row.
pub fn cols(row: &str, from: usize, to: usize) -> String {
    row.chars().skip(from).take(to - from).collect()
}
