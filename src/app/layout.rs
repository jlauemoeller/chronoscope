//! The screen layout (SPEC §4, §1.4): a pure function from the screen state
//! and size to the area of every visible component.
//!
//! `App::view` draws from it, and the mouse router hit-tests against it, so
//! both always agree on where a component is.

use ratatui::layout::{Rect, Size};

use crate::components::{HEADER_HEIGHT, MAX_TOAST_WIDTH, TOAST_CHROME};

use super::Id;

/// The help line starts this many columns inside the selector overlay.
pub const HELP_LINE_INSET: u16 = 5;
/// The help line is this many rows above the overlay's bottom border
/// (help line, blank row, border).
pub const HELP_LINE_ROWS_FROM_BOTTOM: u16 = 3;
/// The harness tabs are on this row of the overlay (row 3 holds the underline).
pub const TABS_ROW: u16 = 2;
/// The harness tabs start in this column of the overlay.
pub const TABS_COLUMN: u16 = 5;
/// The panes and the Working Directory box start this many columns inside the
/// overlay, and end this many before its right edge.
pub const CONTENT_INSET: u16 = 4;
/// The panes start on this row of the overlay.
pub const PANES_ROW: u16 = 5;
/// The overlay's rows that are not pane rows: the panes start on row 5 and
/// below them come the Working Directory box (5 rows), a blank row, the help
/// line, a blank row and the border.
pub const PANES_HEIGHT_CHROME: u16 = 14;
/// The Working Directory box is this many rows high.
pub const WORKING_DIR_HEIGHT: u16 = 5;
/// The Projects pane is at least this wide.
pub const MIN_PROJECTS_WIDTH: u16 = 20;
/// The Projects pane is at most this wide.
pub const MAX_PROJECTS_WIDTH: u16 = 36;
/// The columns between the Projects and the Sessions pane.
pub const PANE_GAP: u16 = 1;
/// The toast's right edge is this many columns from the screen's right edge.
pub const TOAST_RIGHT_MARGIN: u16 = 1;
/// The toast's bottom border is on this many rows above the screen's last row.
pub const TOAST_BOTTOM_MARGIN: u16 = 1;
/// The toast box is this many rows high.
pub const TOAST_HEIGHT: u16 = 3;

/// The terminal is too small below this many columns.
pub const MIN_WIDTH: u16 = 60;
/// The terminal is too small below this many rows.
pub const MIN_HEIGHT: u16 = 20;
/// The selector overlay is at most this wide.
pub const MAX_OVERLAY_WIDTH: u16 = 160;

/// The browser's help line is this many rows high: a blank separator row and
/// the row with the text.
pub const BROWSER_HELP_HEIGHT: u16 = 2;

/// Which screen is shown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Screen {
    /// The session selector (the startup screen).
    #[default]
    Selector,
    /// The session browser.
    Browser,
    /// The session selector as an overlay over the session browser.
    BrowserSelector,
}

impl Screen {
    /// Whether the session browser is on screen (under the selector, if that
    /// is open).
    pub fn has_browser(self) -> bool {
        matches!(self, Self::Browser | Self::BrowserSelector)
    }

    /// Whether the session selector is on screen.
    pub fn has_selector(self) -> bool {
        matches!(self, Self::Selector | Self::BrowserSelector)
    }
}

/// What the layout depends on besides the size.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LayoutInput {
    /// The screen being shown.
    pub screen: Screen,
    /// The display width of the toast text, if a toast is shown.
    pub toast_text_width: Option<usize>,
    /// Whether a prompt is active: it takes the help line's place.
    pub prompt: bool,
    /// Whether the open session has no turns: the browser shows
    /// [`Id::EmptySession`] in place of the transcript.
    pub session_empty: bool,
    /// The size the help overlay wants, borders included, while it is open.
    pub help: Option<Size>,
    /// The size the details overlay wants, borders included, while it is
    /// open.
    pub details: Option<Size>,
    /// The size the theme selector wants, borders included, while it is
    /// open.
    pub theme_selector: Option<Size>,
}

/// The area of each visible component, in drawing order (overlays last).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScreenLayout {
    areas: Vec<(Id, Rect)>,
}

impl ScreenLayout {
    /// The visible components with their areas, in drawing order.
    pub fn areas(&self) -> &[(Id, Rect)] {
        &self.areas
    }

    /// The area of `id`, or `None` if it is not visible.
    pub fn area(&self, id: Id) -> Option<Rect> {
        self.areas.iter().find(|(i, _)| *i == id).map(|(_, r)| *r)
    }
}

/// Whether a screen of this size is too small to show the UI.
pub fn too_small(area: Rect) -> bool {
    area.width < MIN_WIDTH || area.height < MIN_HEIGHT
}

/// Computes the layout for a screen of `area`.
///
/// When the screen is too small (under 60 columns or 20 rows) only
/// [`Id::EmptyState`] is visible, filling the screen. Otherwise the selector
/// overlay is [`Id::SelectorFrame`]: as wide as the screen up to 160 columns
/// and centred, with one empty row above and below. Inside it come the tabs,
/// the two panes, the Working Directory box (see [`selector_areas`]) and
/// [`Id::HelpLine`]: one row, two rows above the bottom border, from column 5
/// to width - 6. An active prompt ([`LayoutInput::prompt`]) is [`Id::Prompt`]
/// in the same place instead of the help line; over the browser alone it
/// takes the browser's help line.
///
/// The help overlay and the details overlay, when `input` has one, come after
/// the screen (see [`help_overlay_rect`]). The toast, when `input` has one, comes last (on
/// top) in the bottom-right corner of the screen, except on the too-small
/// screen.
pub fn layout(input: &LayoutInput, area: Rect) -> ScreenLayout {
    if too_small(area) {
        return ScreenLayout {
            areas: vec![(Id::EmptyState, area)],
        };
    }
    let mut areas = Vec::new();
    if input.screen.has_browser() {
        let browser = browser_areas(area);
        let body = if input.session_empty {
            Id::EmptySession
        } else {
            Id::Transcript
        };
        // A prompt takes the browser's help line only when no selector is
        // over it: then it takes the selector's.
        let help = if input.prompt && !input.screen.has_selector() {
            Id::Prompt
        } else {
            Id::BrowserHelpLine
        };
        areas.extend([
            (Id::BrowserHeader, browser.header),
            (body, browser.transcript),
            (help, browser.help_line),
        ]);
    }
    if input.screen.has_selector() {
        let overlay = selector_overlay(area);
        let selector = selector_areas(overlay);
        let bottom = if input.prompt {
            Id::Prompt
        } else {
            Id::HelpLine
        };
        areas.extend([
            (Id::SelectorFrame, overlay),
            (Id::HarnessTabs, selector.tabs),
            (Id::ProjectList, selector.projects),
            (Id::SessionList, selector.sessions),
            (Id::WorkingDir, selector.working_dir),
            (bottom, selector.help_line),
        ]);
    }
    if let Some(wanted) = input.help {
        areas.push((Id::HelpOverlay, help_overlay_rect(area, wanted)));
    }
    if let Some(wanted) = input.details {
        areas.push((Id::DetailsOverlay, help_overlay_rect(area, wanted)));
    }
    if let Some(wanted) = input.theme_selector {
        areas.push((Id::ThemeSelector, help_overlay_rect(area, wanted)));
    }
    if let Some(text_width) = input.toast_text_width {
        areas.push((Id::Toast, toast_rect(area, text_width)));
    }
    ScreenLayout { areas }
}

/// The areas of the session browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrowserAreas {
    /// The header: the first [`HEADER_HEIGHT`] rows, full width.
    pub header: Rect,
    /// The transcript: from the row under the header to the row above the
    /// help line's blank separator row.
    pub transcript: Rect,
    /// The help line: the last two rows, full width. Its text is on the last
    /// row; the row above it is blank.
    pub help_line: Rect,
}

/// Places the browser's components in a screen of `area`, which is at least
/// [`MIN_WIDTH`] x [`MIN_HEIGHT`]: the header on rows 0 to 4, the transcript
/// on rows 5 to `height - 3` and the help line on the last two rows.
pub fn browser_areas(area: Rect) -> BrowserAreas {
    let header_height = HEADER_HEIGHT.min(area.height);
    let help_height = BROWSER_HELP_HEIGHT.min(area.height - header_height);
    let transcript_height = area.height - header_height - help_height;
    BrowserAreas {
        header: Rect::new(area.x, area.y, area.width, header_height),
        transcript: Rect::new(
            area.x,
            area.y + header_height,
            area.width,
            transcript_height,
        ),
        help_line: Rect::new(area.x, area.bottom() - help_height, area.width, help_height),
    }
}

/// The areas inside the selector overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectorAreas {
    /// The tab labels and, below them, the underline: two rows.
    pub tabs: Rect,
    pub projects: Rect,
    pub sessions: Rect,
    pub working_dir: Rect,
    pub help_line: Rect,
}

/// Places the components inside the selector `overlay` (SPEC §4.1).
///
/// The content area runs from column 4 to column `width - 5` of the overlay,
/// so its width is `width - 8`. The panes start on row 5 and are
/// `height - 14` rows high; the Projects pane is a third of the content
/// width (at least 20, at most 36 columns), then one column of gap, then the
/// Sessions pane. The Working Directory box follows directly, five rows
/// high.
pub fn selector_areas(overlay: Rect) -> SelectorAreas {
    let content_x = overlay.x + CONTENT_INSET;
    let content_width = overlay.width.saturating_sub(2 * CONTENT_INSET);
    let projects_width = (content_width / 3)
        .clamp(MIN_PROJECTS_WIDTH, MAX_PROJECTS_WIDTH)
        .min(content_width);
    let sessions_width = content_width.saturating_sub(projects_width + PANE_GAP);
    let panes_y = overlay.y + PANES_ROW;
    let panes_height = overlay.height.saturating_sub(PANES_HEIGHT_CHROME);
    SelectorAreas {
        tabs: Rect::new(
            overlay.x + TABS_COLUMN,
            overlay.y + TABS_ROW,
            overlay.width.saturating_sub(2 * TABS_COLUMN),
            2,
        ),
        projects: Rect::new(content_x, panes_y, projects_width, panes_height),
        sessions: Rect::new(
            content_x + projects_width + PANE_GAP,
            panes_y,
            sessions_width,
            panes_height,
        ),
        working_dir: Rect::new(
            content_x,
            panes_y + panes_height,
            content_width,
            WORKING_DIR_HEIGHT,
        ),
        help_line: selector_help_line(overlay),
    }
}

/// The help line inside the selector `overlay`.
fn selector_help_line(overlay: Rect) -> Rect {
    Rect {
        x: overlay.x + HELP_LINE_INSET,
        y: overlay.y + overlay.height - HELP_LINE_ROWS_FROM_BOTTOM,
        width: overlay.width.saturating_sub(2 * HELP_LINE_INSET),
        height: 1,
    }
}

/// The help overlay and the theme selector: `wanted` (its widest entry and its content, with the
/// border) but at most the screen less two columns either side and one row
/// above and below, and centred.
pub fn help_overlay_rect(area: Rect, wanted: Size) -> Rect {
    let width = wanted.width.min(area.width.saturating_sub(4));
    let height = wanted.height.min(area.height.saturating_sub(2));
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

/// The toast box for text of `text_width` columns: the text plus four columns
/// (borders and padding), at most `min(60, width - 4)`, with its last column
/// one column from the screen's right edge and its bottom border on row
/// `height - 2`.
fn toast_rect(area: Rect, text_width: usize) -> Rect {
    let most = MAX_TOAST_WIDTH.min(area.width.saturating_sub(4));
    let wanted = u16::try_from(text_width).unwrap_or(u16::MAX);
    let width = wanted.saturating_add(TOAST_CHROME).min(most);
    Rect {
        x: area.right().saturating_sub(TOAST_RIGHT_MARGIN + width),
        y: area
            .bottom()
            .saturating_sub(TOAST_BOTTOM_MARGIN + TOAST_HEIGHT),
        width,
        height: TOAST_HEIGHT,
    }
}

/// The selector overlay rect. `area` is at least [`MIN_WIDTH`] x
/// [`MIN_HEIGHT`].
fn selector_overlay(area: Rect) -> Rect {
    let width = area.width.min(MAX_OVERLAY_WIDTH);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + 1,
        width,
        height: area.height - 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(width: u16, height: u16) -> ScreenLayout {
        layout(&LayoutInput::default(), Rect::new(0, 0, width, height))
    }

    #[test]
    fn narrow_terminal_shows_only_the_empty_state() {
        let l = at(59, 24);
        assert_eq!(l.areas(), &[(Id::EmptyState, Rect::new(0, 0, 59, 24))]);
    }

    #[test]
    fn short_terminal_shows_only_the_empty_state() {
        let l = at(80, 19);
        assert_eq!(l.areas(), &[(Id::EmptyState, Rect::new(0, 0, 80, 19))]);
    }

    #[test]
    fn smallest_usable_terminal_shows_the_overlay() {
        let l = at(60, 20);
        assert_eq!(l.area(Id::SelectorFrame), Some(Rect::new(0, 1, 60, 18)));
    }

    #[test]
    fn overlay_at_80x24() {
        let l = at(80, 24);
        assert_eq!(l.area(Id::SelectorFrame), Some(Rect::new(0, 1, 80, 22)));
        assert_eq!(l.area(Id::EmptyState), None);
    }

    #[test]
    fn overlay_at_120x40_is_full_width() {
        assert_eq!(
            at(120, 40).area(Id::SelectorFrame),
            Some(Rect::new(0, 1, 120, 38))
        );
    }

    #[test]
    fn overlay_at_160_columns_is_full_width() {
        assert_eq!(
            at(160, 30).area(Id::SelectorFrame),
            Some(Rect::new(0, 1, 160, 28))
        );
    }

    #[test]
    fn overlay_at_200x50_is_160_wide_and_centred() {
        assert_eq!(
            at(200, 50).area(Id::SelectorFrame),
            Some(Rect::new(20, 1, 160, 48))
        );
    }

    #[test]
    fn odd_extra_width_puts_the_spare_column_on_the_right() {
        assert_eq!(
            at(201, 24).area(Id::SelectorFrame),
            Some(Rect::new(20, 1, 160, 22))
        );
    }

    #[test]
    fn degenerate_sizes_do_not_panic() {
        for (w, h) in [(0, 0), (1, 1), (0, 24), (80, 0), (80, 1), (80, 2)] {
            let l = at(w, h);
            assert_eq!(l.areas(), &[(Id::EmptyState, Rect::new(0, 0, w, h))]);
        }
    }

    #[test]
    fn the_layout_follows_the_area_origin() {
        let l = layout(&LayoutInput::default(), Rect::new(3, 2, 80, 24));
        assert_eq!(l.area(Id::SelectorFrame), Some(Rect::new(3, 3, 80, 22)));
    }

    fn areas(width: u16, height: u16) -> SelectorAreas {
        selector_areas(at(width, height).area(Id::SelectorFrame).unwrap())
    }

    #[test]
    fn panes_at_120x40_are_36_and_75_columns_and_24_rows() {
        // Overlay 120x38: content width 112.
        let a = areas(120, 40);
        assert_eq!(a.projects.width, 36);
        assert_eq!(a.sessions.width, 75);
        assert_eq!(a.projects.width + PANE_GAP + a.sessions.width, 112);
        assert_eq!(a.projects.height, 24);
        assert_eq!(a.sessions.height, 24);
        assert_eq!(a.working_dir.width, 112);
    }

    #[test]
    fn panes_at_80x24_are_24_and_47_columns() {
        let a = areas(80, 24);
        assert_eq!(a.projects.width, 24);
        assert_eq!(a.sessions.width, 47);
        assert_eq!(a.projects.height, 8);
    }

    #[test]
    fn projects_width_is_a_third_clamped_to_20_and_36() {
        // Content width is the terminal width - 8 (below 160 columns).
        assert_eq!(areas(60, 24).projects.width, 20);
        assert_eq!(areas(68, 24).projects.width, 20);
        assert_eq!(areas(80, 24).projects.width, 24);
        assert_eq!(areas(116, 24).projects.width, 36);
        assert_eq!(areas(200, 50).projects.width, 36);
        assert_eq!(areas(200, 50).sessions.width, 160 - 8 - 36 - 1);
    }

    #[test]
    fn components_are_placed_at_the_planner_offsets() {
        let overlay = at(120, 40).area(Id::SelectorFrame).unwrap();
        let a = selector_areas(overlay);
        assert_eq!(a.tabs, Rect::new(5, 3, 110, 2));
        assert_eq!(a.projects, Rect::new(4, 6, 36, 24));
        assert_eq!(a.sessions, Rect::new(41, 6, 75, 24));
        assert_eq!(a.working_dir, Rect::new(4, 30, 112, 5));
        assert_eq!(a.sessions.right(), overlay.right() - 4);
    }

    #[test]
    fn the_working_directory_box_is_followed_by_a_blank_row_and_the_help_line() {
        for (w, h) in [(60, 20), (80, 24), (120, 40), (200, 50)] {
            let a = areas(w, h);
            assert_eq!(a.working_dir.y, a.projects.bottom(), "{w}x{h}");
            assert_eq!(a.working_dir.height, 5, "{w}x{h}");
            assert_eq!(a.help_line.y, a.working_dir.bottom() + 1, "{w}x{h}");
        }
    }

    #[test]
    fn the_layout_follows_a_centred_overlay_for_the_panes() {
        let a = areas(200, 50);
        assert_eq!(a.projects.x, 20 + 4);
        assert_eq!(a.tabs.x, 20 + 5);
    }

    #[test]
    fn the_layout_lists_the_selector_components_in_drawing_order() {
        let ids: Vec<Id> = at(80, 24).areas().iter().map(|(id, _)| *id).collect();
        assert_eq!(
            ids,
            [
                Id::SelectorFrame,
                Id::HarnessTabs,
                Id::ProjectList,
                Id::SessionList,
                Id::WorkingDir,
                Id::HelpLine
            ]
        );
    }

    #[test]
    fn the_smallest_terminal_has_a_pane_with_one_list_row() {
        let a = areas(60, 20);
        assert_eq!(a.projects.height, 4);
        assert_eq!(crate::components::visible_rows(a.projects.height), 1);
    }

    fn browser(width: u16, height: u16, empty: bool) -> ScreenLayout {
        let input = LayoutInput {
            screen: Screen::Browser,
            session_empty: empty,
            ..LayoutInput::default()
        };
        layout(&input, Rect::new(0, 0, width, height))
    }

    #[test]
    fn the_browser_is_header_transcript_and_a_two_row_help_line() {
        let l = browser(120, 40, false);
        assert_eq!(l.area(Id::BrowserHeader), Some(Rect::new(0, 0, 120, 5)));
        assert_eq!(l.area(Id::Transcript), Some(Rect::new(0, 5, 120, 33)));
        assert_eq!(l.area(Id::BrowserHelpLine), Some(Rect::new(0, 38, 120, 2)));
        assert_eq!(l.area(Id::EmptySession), None);
        assert_eq!(l.area(Id::SelectorFrame), None);
        let ids: Vec<Id> = l.areas().iter().map(|(id, _)| *id).collect();
        assert_eq!(
            ids,
            [Id::BrowserHeader, Id::Transcript, Id::BrowserHelpLine]
        );
    }

    #[test]
    fn the_transcript_runs_from_row_5_to_row_height_minus_3() {
        for (w, h) in [(60, 20), (80, 24), (120, 40), (200, 50)] {
            let l = browser(w, h, false);
            let t = l.area(Id::Transcript).unwrap();
            assert_eq!((t.y, t.bottom()), (5, h - 2), "{w}x{h}: rows 5..=h-3");
            let help = l.area(Id::BrowserHelpLine).unwrap();
            assert_eq!(
                (help.y, help.bottom(), help.width),
                (h - 2, h, w),
                "{w}x{h}"
            );
            let rows: u16 = [Id::BrowserHeader, Id::Transcript, Id::BrowserHelpLine]
                .iter()
                .map(|id| l.area(*id).unwrap().height)
                .sum();
            assert_eq!(rows, h, "every row belongs to a component");
        }
    }

    #[test]
    fn an_empty_session_puts_the_message_where_the_transcript_was() {
        let l = browser(80, 24, true);
        assert_eq!(l.area(Id::Transcript), None);
        assert_eq!(l.area(Id::EmptySession), Some(Rect::new(0, 5, 80, 17)));
    }

    #[test]
    fn the_browser_follows_the_area_origin() {
        let input = LayoutInput {
            screen: Screen::Browser,
            ..LayoutInput::default()
        };
        let l = layout(&input, Rect::new(3, 2, 80, 24));
        assert_eq!(l.area(Id::BrowserHeader), Some(Rect::new(3, 2, 80, 5)));
        assert_eq!(l.area(Id::BrowserHelpLine), Some(Rect::new(3, 24, 80, 2)));
    }

    #[test]
    fn a_too_small_terminal_replaces_the_browser_too() {
        let input = LayoutInput {
            screen: Screen::Browser,
            ..LayoutInput::default()
        };
        let l = layout(&input, Rect::new(0, 0, 59, 24));
        assert_eq!(l.areas(), &[(Id::EmptyState, Rect::new(0, 0, 59, 24))]);
    }

    #[test]
    fn the_selector_over_the_browser_is_drawn_after_the_browser() {
        let input = LayoutInput {
            screen: Screen::BrowserSelector,
            toast_text_width: Some(5),
            ..LayoutInput::default()
        };
        let l = layout(&input, Rect::new(0, 0, 200, 50));
        let ids: Vec<Id> = l.areas().iter().map(|(id, _)| *id).collect();
        assert_eq!(
            ids,
            [
                Id::BrowserHeader,
                Id::Transcript,
                Id::BrowserHelpLine,
                Id::SelectorFrame,
                Id::HarnessTabs,
                Id::ProjectList,
                Id::SessionList,
                Id::WorkingDir,
                Id::HelpLine,
                Id::Toast
            ]
        );
        // The overlay leaves the first and the last row, and the side columns.
        assert_eq!(l.area(Id::SelectorFrame), Some(Rect::new(20, 1, 160, 48)));
        assert_eq!(l.area(Id::BrowserHeader), Some(Rect::new(0, 0, 200, 5)));
        assert_eq!(l.area(Id::BrowserHelpLine), Some(Rect::new(0, 48, 200, 2)));
    }

    #[test]
    fn a_prompt_over_the_browser_takes_the_selectors_help_line_not_the_browsers() {
        let input = LayoutInput {
            screen: Screen::BrowserSelector,
            prompt: true,
            ..LayoutInput::default()
        };
        let l = layout(&input, Rect::new(0, 0, 80, 24));
        assert_eq!(l.area(Id::HelpLine), None);
        assert!(l.area(Id::Prompt).is_some());
        assert!(l.area(Id::BrowserHelpLine).is_some());
    }

    #[test]
    fn a_prompt_over_the_browser_alone_takes_the_browsers_help_line() {
        let input = LayoutInput {
            screen: Screen::Browser,
            prompt: true,
            ..LayoutInput::default()
        };
        let l = layout(&input, Rect::new(0, 0, 80, 24));
        assert_eq!(l.area(Id::BrowserHelpLine), None);
        assert_eq!(l.area(Id::HelpLine), None);
        let without = layout(
            &LayoutInput {
                prompt: false,
                ..input
            },
            Rect::new(0, 0, 80, 24),
        );
        assert_eq!(l.area(Id::Prompt), without.area(Id::BrowserHelpLine));
        assert_eq!(l.area(Id::Prompt), Some(Rect::new(0, 22, 80, 2)));
    }

    #[test]
    fn the_help_overlay_is_centred_and_drawn_after_the_screen_and_before_the_toast() {
        let input = LayoutInput {
            screen: Screen::Browser,
            help: Some(Size::new(72, 52)),
            toast_text_width: Some(10),
            ..LayoutInput::default()
        };
        let l = layout(&input, Rect::new(0, 0, 200, 60));
        assert_eq!(l.area(Id::HelpOverlay), Some(Rect::new(64, 4, 72, 52)));
        let ids: Vec<Id> = l.areas().iter().map(|(id, _)| *id).collect();
        let help = ids.iter().position(|i| *i == Id::HelpOverlay).unwrap();
        assert_eq!(ids.len(), help + 2);
        assert_eq!(ids[help + 1], Id::Toast);
        assert!(ids[..help].contains(&Id::Transcript));
    }

    #[test]
    fn the_details_overlay_is_centred_and_drawn_after_the_screen_and_before_the_toast() {
        let input = LayoutInput {
            screen: Screen::Browser,
            details: Some(Size::new(60, 20)),
            toast_text_width: Some(10),
            ..LayoutInput::default()
        };
        let l = layout(&input, Rect::new(0, 0, 120, 40));
        assert_eq!(l.area(Id::DetailsOverlay), Some(Rect::new(30, 10, 60, 20)));
        assert_eq!(l.area(Id::HelpOverlay), None);
        let ids: Vec<Id> = l.areas().iter().map(|(id, _)| *id).collect();
        let at = ids.iter().position(|i| *i == Id::DetailsOverlay).unwrap();
        assert_eq!(ids.len(), at + 2);
        assert_eq!(ids[at + 1], Id::Toast);
        assert!(ids[..at].contains(&Id::Transcript));
    }

    #[test]
    fn the_details_overlay_is_capped_to_the_screen_like_the_help_overlay() {
        let input = LayoutInput {
            screen: Screen::Browser,
            details: Some(Size::new(150, 80)),
            ..LayoutInput::default()
        };
        let l = layout(&input, Rect::new(0, 0, 80, 24));
        assert_eq!(l.area(Id::DetailsOverlay), Some(Rect::new(2, 1, 76, 22)));
    }

    #[test]
    fn a_too_small_screen_shows_no_details_overlay() {
        let input = LayoutInput {
            screen: Screen::Browser,
            details: Some(Size::new(60, 20)),
            ..LayoutInput::default()
        };
        assert_eq!(
            layout(&input, Rect::new(0, 0, 50, 15)).area(Id::DetailsOverlay),
            None
        );
    }

    #[test]
    fn the_help_overlay_is_over_the_selector_too() {
        let input = LayoutInput {
            screen: Screen::Selector,
            help: Some(Size::new(72, 24)),
            ..LayoutInput::default()
        };
        let l = layout(&input, Rect::new(0, 0, 120, 40));
        assert_eq!(l.areas().last().unwrap().0, Id::HelpOverlay);
        assert!(l.area(Id::SelectorFrame).is_some());
    }

    #[test]
    fn the_help_overlay_leaves_two_columns_and_a_row_either_side() {
        let wanted = Size::new(72, 52);
        assert_eq!(
            help_overlay_rect(Rect::new(0, 0, 80, 24), wanted),
            Rect::new(4, 1, 72, 22)
        );
        assert_eq!(
            help_overlay_rect(Rect::new(0, 0, 70, 24), wanted),
            Rect::new(2, 1, 66, 22)
        );
        assert_eq!(
            help_overlay_rect(Rect::new(0, 0, 120, 40), Size::new(72, 24)),
            Rect::new(24, 8, 72, 24)
        );
    }

    #[test]
    fn the_theme_selector_is_centred_and_drawn_after_the_screen_and_before_the_toast() {
        let input = LayoutInput {
            screen: Screen::Browser,
            theme_selector: Some(Size::new(41, 12)),
            toast_text_width: Some(10),
            ..LayoutInput::default()
        };
        let l = layout(&input, Rect::new(0, 0, 120, 40));
        assert_eq!(l.area(Id::ThemeSelector), Some(Rect::new(39, 14, 41, 12)));
        let ids: Vec<Id> = l.areas().iter().map(|(id, _)| *id).collect();
        let at = ids.iter().position(|i| *i == Id::ThemeSelector).unwrap();
        assert_eq!(ids.len(), at + 2);
        assert_eq!(ids[at + 1], Id::Toast);
        assert!(ids[..at].contains(&Id::Transcript));
    }

    #[test]
    fn the_theme_selector_is_over_the_selector_too() {
        let input = LayoutInput {
            screen: Screen::Selector,
            theme_selector: Some(Size::new(41, 12)),
            ..LayoutInput::default()
        };
        let l = layout(&input, Rect::new(0, 0, 80, 24));
        assert_eq!(l.areas().last().unwrap().0, Id::ThemeSelector);
        assert_eq!(l.area(Id::ThemeSelector), Some(Rect::new(19, 6, 41, 12)));
        assert!(l.area(Id::SelectorFrame).is_some());
    }

    #[test]
    fn the_theme_selector_is_capped_to_the_screen() {
        let input = LayoutInput {
            screen: Screen::Selector,
            theme_selector: Some(Size::new(41, 21)),
            ..LayoutInput::default()
        };
        // 60x20: 18 rows at most, and 41 columns fit.
        let l = layout(&input, Rect::new(0, 0, 60, 20));
        assert_eq!(l.area(Id::ThemeSelector), Some(Rect::new(9, 1, 41, 18)));
        let wide = LayoutInput {
            theme_selector: Some(Size::new(100, 12)),
            ..input
        };
        let l = layout(&wide, Rect::new(0, 0, 60, 20));
        assert_eq!(l.area(Id::ThemeSelector), Some(Rect::new(2, 4, 56, 12)));
    }

    #[test]
    fn a_too_small_screen_shows_no_theme_selector() {
        let input = LayoutInput {
            theme_selector: Some(Size::new(41, 12)),
            ..LayoutInput::default()
        };
        let l = layout(&input, Rect::new(0, 0, 50, 15));
        assert_eq!(l.area(Id::ThemeSelector), None);
    }

    #[test]
    fn a_too_small_screen_shows_no_help_overlay() {
        let input = LayoutInput {
            help: Some(Size::new(72, 52)),
            ..LayoutInput::default()
        };
        let l = layout(&input, Rect::new(0, 0, 50, 15));
        assert_eq!(l.area(Id::HelpOverlay), None);
    }

    #[test]
    fn screens_know_which_parts_they_show() {
        assert!(Screen::Selector.has_selector() && !Screen::Selector.has_browser());
        assert!(!Screen::Browser.has_selector() && Screen::Browser.has_browser());
        assert!(Screen::BrowserSelector.has_selector() && Screen::BrowserSelector.has_browser());
    }

    fn with_toast(width: u16, height: u16, text_width: usize) -> ScreenLayout {
        let input = LayoutInput {
            toast_text_width: Some(text_width),
            ..LayoutInput::default()
        };
        layout(&input, Rect::new(0, 0, width, height))
    }

    #[test]
    fn an_active_prompt_takes_the_help_lines_place_and_the_toast_stays_last() {
        let input = LayoutInput {
            prompt: true,
            toast_text_width: Some(10),
            ..LayoutInput::default()
        };
        let l = layout(&input, Rect::new(0, 0, 80, 24));
        assert_eq!(l.area(Id::HelpLine), None);
        assert_eq!(l.area(Id::Prompt), at(80, 24).area(Id::HelpLine));
        assert_eq!(l.areas().last().map(|(id, _)| *id), Some(Id::Toast));
        assert_eq!(at(80, 24).area(Id::Prompt), None);
    }

    #[test]
    fn help_line_at_80x24_is_on_row_20_from_column_5_to_74() {
        assert_eq!(at(80, 24).area(Id::HelpLine), Some(Rect::new(5, 20, 70, 1)));
    }

    #[test]
    fn help_line_at_120x40_is_on_row_36_from_column_5_to_114() {
        assert_eq!(
            at(120, 40).area(Id::HelpLine),
            Some(Rect::new(5, 36, 110, 1))
        );
    }

    #[test]
    fn help_line_is_two_rows_above_the_bottom_border() {
        for (w, h) in [(60, 20), (80, 24), (120, 40), (200, 50)] {
            let l = at(w, h);
            let overlay = l.area(Id::SelectorFrame).unwrap();
            let help = l.area(Id::HelpLine).unwrap();
            assert_eq!(help.y, overlay.y + overlay.height - 3, "{w}x{h}");
            assert_eq!(help.x, overlay.x + 5, "{w}x{h}");
            assert_eq!(help.right(), overlay.right() - 5, "{w}x{h}");
        }
    }

    #[test]
    fn help_line_follows_a_centred_overlay() {
        assert_eq!(
            at(200, 50).area(Id::HelpLine),
            Some(Rect::new(25, 46, 150, 1))
        );
    }

    #[test]
    fn there_is_no_toast_without_one_in_the_input() {
        assert_eq!(at(80, 24).area(Id::Toast), None);
    }

    #[test]
    fn toast_at_80x24_sits_in_the_bottom_right_corner() {
        let l = with_toast(80, 24, 20);
        // Width 24, right edge at column 78, bottom border on row 22.
        assert_eq!(l.area(Id::Toast), Some(Rect::new(55, 20, 24, 3)));
    }

    #[test]
    fn toast_at_120x40_sits_in_the_bottom_right_corner() {
        let l = with_toast(120, 40, 10);
        assert_eq!(l.area(Id::Toast), Some(Rect::new(105, 36, 14, 3)));
    }

    #[test]
    fn toast_edges_are_one_column_and_two_rows_inside_the_screen() {
        for (w, h) in [(60, 20), (80, 24), (120, 40), (200, 50)] {
            let toast = with_toast(w, h, 8).area(Id::Toast).unwrap();
            assert_eq!(toast.right(), w - 1, "{w}x{h}");
            assert_eq!(toast.bottom(), h - 1, "bottom border on row h-2: {w}x{h}");
        }
    }

    #[test]
    fn long_toast_text_is_capped_at_60_or_width_minus_4() {
        assert_eq!(with_toast(80, 24, 500).area(Id::Toast).unwrap().width, 60);
        assert_eq!(with_toast(120, 40, 500).area(Id::Toast).unwrap().width, 60);
        assert_eq!(with_toast(62, 24, 500).area(Id::Toast).unwrap().width, 58);
        assert_eq!(with_toast(60, 24, 500).area(Id::Toast).unwrap().width, 56);
    }

    #[test]
    fn toast_is_drawn_last() {
        let l = with_toast(80, 24, 5);
        assert_eq!(l.areas().last().map(|(id, _)| *id), Some(Id::Toast));
    }

    #[test]
    fn too_small_screen_shows_no_toast() {
        assert_eq!(with_toast(59, 24, 5).area(Id::Toast), None);
    }
}
