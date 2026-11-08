//! Snapshots of notices: both levels, short and wrapping, selected, unselected
//! with `dim_unselected` on and off, at block widths 80 and 120.

use super::super::fixtures::*;
use crate::model::{NoticeLevel, Turn};
use crate::test_support::buffer_to_styled;

const SHORT_ERROR: &str = "provider.transport: connection reset";
const LONG_ERROR: &str = "provider.invalid-request: messages: text content blocks must be non-empty, and the request was rejected before any tokens were generated, so nothing was billed for it";
const SHORT_WARNING: &str = "aborted";
const LONG_WARNING: &str = "aborted: Tool execution interrupted while the agent was waiting for the subagent to finish its work, and the step was abandoned";

/// An Assistant turn with some text, then the notice (`n1`), then more text,
/// so that the notice is in the turn's order and has neighbours.
fn turn(level: NoticeLevel, text: &str) -> Turn {
    assistant(
        Some("high"),
        vec![
            self::text("t1", "Running the build now."),
            notice("n1", level, text),
            self::text("t2", "Anything after the notice."),
        ],
        None,
    )
}

fn draw_notice(level: NoticeLevel, text: &str, selected: bool, dim: bool, width: u16) -> String {
    let turn = turn(level, text);
    let mut opts = Opts::default().dim(dim);
    if selected {
        opts = opts.select(item_id(&turn, "n1"));
    }
    buffer_to_styled(&draw(&turn, &opts, width))
}

macro_rules! notice_snapshots {
    ($($name:ident: $level:expr, $text:expr, $selected:expr, $dim:expr, $width:expr;)*) => {$(
        #[test]
        fn $name() {
            insta::assert_snapshot!(draw_notice($level, $text, $selected, $dim, $width));
        }
    )*};
}

notice_snapshots! {
    error_short_selected_at_80: NoticeLevel::Error, SHORT_ERROR, true, true, 80;
    error_short_selected_at_120: NoticeLevel::Error, SHORT_ERROR, true, true, 120;
    error_short_dimmed_at_80: NoticeLevel::Error, SHORT_ERROR, false, true, 80;
    error_short_dimmed_at_120: NoticeLevel::Error, SHORT_ERROR, false, true, 120;
    error_short_undimmed_at_80: NoticeLevel::Error, SHORT_ERROR, false, false, 80;
    error_short_undimmed_at_120: NoticeLevel::Error, SHORT_ERROR, false, false, 120;
    error_long_selected_at_80: NoticeLevel::Error, LONG_ERROR, true, true, 80;
    error_long_selected_at_120: NoticeLevel::Error, LONG_ERROR, true, true, 120;
    error_long_dimmed_at_80: NoticeLevel::Error, LONG_ERROR, false, true, 80;
    error_long_dimmed_at_120: NoticeLevel::Error, LONG_ERROR, false, true, 120;
    error_long_undimmed_at_80: NoticeLevel::Error, LONG_ERROR, false, false, 80;
    error_long_undimmed_at_120: NoticeLevel::Error, LONG_ERROR, false, false, 120;
    warning_short_selected_at_80: NoticeLevel::Warning, SHORT_WARNING, true, true, 80;
    warning_short_selected_at_120: NoticeLevel::Warning, SHORT_WARNING, true, true, 120;
    warning_short_dimmed_at_80: NoticeLevel::Warning, SHORT_WARNING, false, true, 80;
    warning_short_dimmed_at_120: NoticeLevel::Warning, SHORT_WARNING, false, true, 120;
    warning_short_undimmed_at_80: NoticeLevel::Warning, SHORT_WARNING, false, false, 80;
    warning_short_undimmed_at_120: NoticeLevel::Warning, SHORT_WARNING, false, false, 120;
    warning_long_selected_at_80: NoticeLevel::Warning, LONG_WARNING, true, true, 80;
    warning_long_selected_at_120: NoticeLevel::Warning, LONG_WARNING, true, true, 120;
    warning_long_dimmed_at_80: NoticeLevel::Warning, LONG_WARNING, false, true, 80;
    warning_long_dimmed_at_120: NoticeLevel::Warning, LONG_WARNING, false, true, 120;
    warning_long_undimmed_at_80: NoticeLevel::Warning, LONG_WARNING, false, false, 80;
    warning_long_undimmed_at_120: NoticeLevel::Warning, LONG_WARNING, false, false, 120;
}
