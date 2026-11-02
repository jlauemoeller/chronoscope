//! Snapshots of the details overlay (SPEC §5.3): one section of each kind,
//! fully populated from plain data (the harness is not involved), at 80x24
//! and 120x40 in the `default` and `terminal` themes, and content taller than
//! the screen scrolled to the top and to the end.

use ratatui::layout::Rect;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

use super::*;
use crate::app::layout::help_overlay_rect;
use crate::test_support::{buffer_to_styled, render};
use crate::theme::builtin;

fn group(title: &str, rows: &[(&str, &str)]) -> DetailsGroup {
    DetailsGroup::new(
        title,
        rows.iter().map(|(l, v)| DetailsRow::new(*l, *v)).collect(),
    )
}

const CWD: &str = "/home/user/projects/brio/crates/camera-profiles/src/descriptors";

fn claude_turn() -> DetailsGroup {
    group(
        "Turn",
        &[
            ("model", "claude-opus-5-5"),
            ("effort", "high"),
            ("start", "2026-10-21 08:35:18Z"),
            ("duration", "4m 3s"),
            ("messages", "12"),
            ("input tokens", "2.8k"),
            ("output tokens", "42.4k"),
            ("thinking tokens", "9.0k"),
            ("cache read tokens", "359.4M"),
            ("cache write tokens", "1.2k"),
        ],
    )
}

fn claude_message() -> DetailsGroup {
    group(
        "Message",
        &[
            ("message.id", "msg_01Q7ZkR2m4Wc9xT8vHn3LbYe"),
            ("requestId", "req_011CUKq8xN2dPz5aVf7GhJrT"),
            ("timestamp", "2026-10-21 08:35:21Z"),
            ("requestedModel", "claude-opus-5-5"),
            ("stop_reason", "end_turn"),
            ("service_tier", "standard"),
            ("speed", "standard"),
            ("inference_geo", "us"),
            ("5m cache writes", "800"),
            ("1h cache writes", "400"),
            ("web searches", "2"),
            ("web fetches", "1"),
            ("diagnostics.cache_miss_reason", "prefix_changed"),
        ],
    )
}

fn claude_environment() -> DetailsGroup {
    group(
        "Environment",
        &[
            ("gitBranch", "feature/camera-profiles"),
            ("cwd", CWD),
            ("version", "2.1.0"),
            ("entrypoint", "cli"),
        ],
    )
}

fn open_code_turn() -> DetailsGroup {
    group(
        "Turn",
        &[
            ("model", "claude-sonnet-5-5"),
            ("effort", "medium"),
            ("start", "2026-10-21 08:35:18Z"),
            ("duration", "45s"),
            ("messages", "3"),
            ("input tokens", "950"),
            ("output tokens", "4.1k"),
            ("thinking tokens", "1.3k"),
            ("cache read tokens", "120.5k"),
            ("cache write tokens", "2.0k"),
            ("cost", "$0.42"),
            ("provider", "anthropic"),
            ("agent", "build"),
        ],
    )
}

fn open_code_message() -> DetailsGroup {
    group(
        "Message",
        &[
            ("row id", "msg_c0a1b2d3e4f5"),
            ("time.created", "2026-10-21 08:35:18Z"),
            ("time.streamed", "2026-10-21 08:35:20Z"),
            ("time.completed", "2026-10-21 08:35:41Z"),
            ("finish", "error"),
            ("rawFinish", "stop_sequence"),
            (
                "error",
                "APIError: 529 overloaded_error: Overloaded. The model is receiving more requests than it can serve right now, retry later.",
            ),
        ],
    )
}

fn open_code_environment() -> DetailsGroup {
    group(
        "Environment",
        &[
            ("snapshot.start", "6f1e2d3c4b5a69788796a5b4c3d2e1f001122334"),
            ("snapshot.end", "0a1b2c3d4e5f60718293a4b5c6d7e8f901234567"),
        ],
    )
}

/// Every kind of section with the groups that apply to it.
fn kinds() -> Vec<(&'static str, DetailsContent)> {
    let content = |groups: Vec<DetailsGroup>| DetailsContent::new(groups);
    vec![
        (
            "user_prompt",
            content(vec![
                group(
                    "Prompt",
                    &[
                        ("timestamp", "2026-10-21 08:35:15Z"),
                        ("permissionMode", "default"),
                        ("promptSource", "user"),
                    ],
                ),
                claude_environment(),
            ]),
        ),
        (
            "assistant_text",
            content(vec![claude_turn(), claude_message(), claude_environment()]),
        ),
        (
            "thinking",
            content(vec![claude_turn(), claude_message(), claude_environment()]),
        ),
        (
            "tool_call",
            content(vec![
                claude_turn(),
                group(
                    "Tool call",
                    &[
                        ("start", "2026-10-21 08:35:24Z"),
                        ("duration", "1m 2s"),
                        ("status", "ok"),
                        ("permissionDecision", "allow"),
                        ("sandboxed", "true"),
                        ("resolvedModel", "claude-opus-5-5"),
                        ("isAsync", "false"),
                    ],
                ),
                claude_environment(),
            ]),
        ),
        (
            "model_note",
            content(vec![claude_turn(), claude_environment()]),
        ),
        (
            "stats_footer",
            content(vec![open_code_turn(), open_code_environment()]),
        ),
        (
            "compaction",
            content(vec![group(
                "Environment",
                &[
                    ("timestamp", "2026-10-21 09:00:00Z"),
                    ("gitBranch", "feature/camera-profiles"),
                    ("cwd", CWD),
                    ("version", "2.1.0"),
                    ("entrypoint", "cli"),
                ],
            )]),
        ),
        (
            "system",
            content(vec![
                claude_turn(),
                group(
                    "Environment",
                    &[
                        ("timestamp", "2026-10-21 08:35:10Z"),
                        ("gitBranch", "feature/camera-profiles"),
                        ("cwd", CWD),
                        ("version", "2.1.0"),
                        ("entrypoint", "cli"),
                    ],
                ),
            ]),
        ),
        (
            "notice",
            content(vec![
                open_code_turn(),
                open_code_message(),
                open_code_environment(),
            ]),
        ),
    ]
}

fn overlay(
    content: DetailsContent,
    theme: Theme,
    width: u16,
    height: u16,
) -> (DetailsOverlay, Rect) {
    let mut overlay = DetailsOverlay::new(content, theme, Keymap::default());
    let screen = Rect::new(0, 0, width, height);
    let wanted = overlay.desired_size(width.saturating_sub(4));
    let area = help_overlay_rect(screen, wanted);
    overlay.attr(
        INTERIOR_ATTR,
        interior_value(crate::components::interior_of(area)),
    );
    (overlay, area)
}

fn draw(overlay: &mut DetailsOverlay, area: Rect, width: u16, height: u16) -> String {
    let buffer = render(width, height, |frame| overlay.view(frame, area));
    buffer_to_styled(&buffer)
}

fn press(overlay: &mut DetailsOverlay, code: Key) {
    overlay.on(&Event::Keyboard(KeyEvent::new(code, KeyModifiers::NONE)));
}

fn themes() -> [(&'static str, Theme); 2] {
    [
        ("default", builtin::default_theme()),
        ("terminal", builtin::terminal_theme()),
    ]
}

#[test]
fn every_kind_of_section_at_both_sizes_in_both_themes() {
    for (kind, content) in kinds() {
        for (width, height) in [(80, 24), (120, 40)] {
            for (name, theme) in themes() {
                let (mut overlay, area) = overlay(content.clone(), theme, width, height);
                insta::assert_snapshot!(
                    format!("{kind}_{width}x{height}_{name}"),
                    draw(&mut overlay, area, width, height)
                );
            }
        }
    }
}

#[test]
fn content_taller_than_the_screen_at_80x24_scrolled_to_the_top_and_to_the_end() {
    let (_, content) = kinds().remove(1);
    for (name, theme) in themes() {
        let (mut overlay, area) = overlay(content.clone(), theme, 80, 24);
        assert!(
            usize::from(area.height) - 2 < content.row_count(area.width - 2),
            "the content must not fit"
        );
        insta::assert_snapshot!(
            format!("tall_80x24_top_{name}"),
            draw(&mut overlay, area, 80, 24)
        );
        for _ in 0..5 {
            press(&mut overlay, Key::PageDown);
        }
        insta::assert_snapshot!(
            format!("tall_80x24_end_{name}"),
            draw(&mut overlay, area, 80, 24)
        );
    }
}

#[test]
fn a_long_value_wraps_to_the_value_column_at_80x24() {
    let (_, content) = kinds().remove(6);
    let (mut overlay, area) = overlay(content, builtin::default_theme(), 80, 24);
    let drawn = draw(&mut overlay, area, 80, 24);
    // The path is wider than the room left of the border.
    assert!(!drawn.contains(CWD), "{drawn}");
    insta::assert_snapshot!("wrapped_value_80x24_default", drawn);
}

#[test]
fn an_unavailable_group_is_one_muted_row() {
    let content = DetailsContent::new(vec![
        group("Turn", &[("model", "claude-opus-5-5")]),
        DetailsGroup::new("Message", vec![DetailsRow::unavailable()]),
    ]);
    let (mut overlay, area) = overlay(content, builtin::default_theme(), 80, 24);
    insta::assert_snapshot!(
        "unavailable_80x24_default",
        draw(&mut overlay, area, 80, 24)
    );
}
