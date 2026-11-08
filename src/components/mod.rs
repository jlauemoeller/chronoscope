//! The UI components (architecture.md#components). Each is its own type and module, can be
//! built from plain data and the theme, and rendered into a `TestBackend`
//! frame without an `Application`.

mod background_fill;
mod browser_header;
mod details_overlay;
mod empty_state;
mod global_events;
mod harness_tabs;
mod help_line;
mod help_overlay;
mod list_pane;
mod overlay_frame;
mod panel_row;
mod project_list;
mod prompt;
mod selector_frame;
mod session_list;
mod session_time;
mod session_tokens;
mod theme_selector;
mod toast;
pub mod transcript;
pub mod turn_block;
mod turn_sidebar;
mod working_dir;

use tuirealm::props::{AttrValue, Attribute, PropPayload};

use crate::theme::Theme;

pub use background_fill::BackgroundFill;
pub use browser_header::{
    BREADCRUMB_ATTR, BrowserHeader, HEADER_HEIGHT, LIVE_ATTR, PARSE_WARNINGS_ATTR, SESSION_ID_ATTR,
    parse_warning_note,
};
#[cfg(test)]
pub(crate) use details_overlay::section_details;
pub use details_overlay::{
    DESIRED_SIZE_ATTR, DetailsContent, DetailsGroup, DetailsOverlay, DetailsRequest, DetailsRow,
    MAX_WIDTH_ATTR, compose, max_width_value, section_source,
};
pub use empty_state::{
    EMPTY_SESSION, EmptyState, LOCAL_COMMANDS_ONLY, NO_MATCHING_SESSIONS, NO_SESSIONS, TOO_SMALL,
    empty_session_message,
};
pub use global_events::{GlobalEvents, quit_key_events};
pub use harness_tabs::{HarnessTabs, TABS_ATTR, Tab, Tabs};
pub use help_line::{HINTS_ATTR, HelpLine, Hint, STATUS_ATTR, hints_value, status_value};
pub use help_overlay::{
    HELP_CONTEXT_ATTR, HelpContent, HelpContext, HelpGroup, HelpOverlay, HelpRow,
    MIN_WIDTH as HELP_MIN_WIDTH, SHIFT_NOTE, context_from_value, context_value,
};
pub use list_pane::{
    ListCursor, SELECTED_ATTR, SESSION_OPEN_ATTR, VISIBLE_ROWS_ATTR, selected_from_value,
    selected_value, visible_rows,
};
pub use overlay_frame::{INTERIOR_ATTR, interior_of, interior_value};
pub use project_list::{PROJECT_ROWS_ATTR, ProjectList, ProjectRow, ProjectRows};
pub use prompt::{Prompt, PromptMode, tail_fitting};
pub use selector_frame::SelectorFrame;
pub use session_list::{
    ACTIVE_ATTR, ActiveSessions, SEARCH_NAV_ATTR, SESSION_FILTERED_ATTR, SESSION_ROWS_ATTR,
    SESSION_STATS_ATTR, SessionList, SessionRow, SessionRows, SessionStatsUpdate, format_stats,
};
pub use session_time::{SESSION_TIME_ATTR, SessionTime, SessionTimes};
pub use session_tokens::{SESSION_TOKENS_ATTR, SessionTokens};
pub use theme_selector::{ThemeItem, ThemeRows, ThemeSelector, theme_items};
pub use toast::{MAX_TOAST_WIDTH, TOAST_CHROME, TOAST_DURATION, Toast, truncate_with_ellipsis};
pub use transcript::{
    DIM_UNSELECTED_ATTR, FOLLOWING_ATTR, GO_TO_TURN_ATTR, MODEL_UPDATE_ATTR, NEW_SECTIONS_ATTR,
    OPEN_SUBAGENT_KEY_ATTR, PRESENTATION_ATTR, Presentation, SEARCH_POSITION_ATTR,
    SEARCH_QUERY_ATTR, SEARCH_REQUEST_ATTR, SEARCH_RESULTS_ATTR, SELECTED_SECTION_ATTR,
    SHOW_SYSTEM_ATTR, SearchStart, TIMESTAMP_FORMATTER_ATTR, TRUNCATE_OUTPUT_LINES_ATTR,
    TURN_COUNT_ATTR, Transcript, VIEWPORT_ATTR, model_update_value, search_results_value,
    search_start_value, viewport_value,
};
pub use turn_block::{
    BlockLayout, FoldState, Section, SectionContent, SectionId, SectionKey, SectionKind, TitleBar,
    TurnBlock, derive_sections, emphasize,
};
pub use turn_sidebar::{
    COVER_ATTR, DEFAULT_KEY_LABEL, Entry, EntryKind, KEY_LABEL_ATTR, Row, SELECTED_TURN_ATTR,
    TURN_ROWS_ATTR, TurnRows, TurnSidebar, entry_for, sidebar_list_rows,
};
pub use working_dir::{CWD_ATTR, WorkingDir};

/// The attribute that carries the theme to a component.
pub const THEME_ATTR: Attribute = Attribute::Custom("theme");

/// The attribute value that sets a component's theme.
pub fn theme_value(theme: &Theme) -> AttrValue {
    AttrValue::Payload(PropPayload::Any(Box::new(theme.clone())))
}

/// The theme in an attribute value made by [`theme_value`].
pub fn theme_from_value(value: &AttrValue) -> Option<Theme> {
    match value {
        AttrValue::Payload(PropPayload::Any(any)) => any.as_any().downcast_ref::<Theme>().cloned(),
        _ => None,
    }
}

/// An attribute value carrying any plain-data `payload` (rows, tabs, ...).
pub fn payload_value<T>(payload: T) -> AttrValue
where
    T: tuirealm::props::PropBound,
{
    AttrValue::Payload(PropPayload::Any(Box::new(payload)))
}

/// The payload of type `T` in an attribute value made by [`payload_value`].
pub fn payload_from_value<T: Clone + 'static>(value: &AttrValue) -> Option<T> {
    match value {
        AttrValue::Payload(PropPayload::Any(any)) => any.as_any().downcast_ref::<T>().cloned(),
        _ => None,
    }
}
