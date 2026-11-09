//! The screen layout (product §3, architecture.md#events-focus-and-keys): a
//! pure function from the screen state and size to the area of every visible
//! component.
//!
//! `App::view` draws from it, and the pointer router asks [`hit_test`] which
//! component on it gets a pointer event, so both always agree on where a
//! component is.

use ratatui::layout::{Rect, Size};
use tuirealm::event::{MouseButton, MouseEvent, MouseEventKind};

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
/// The Session Time panel is this many rows high: two borders and two text
/// rows.
pub const SESSION_TIME_HEIGHT: u16 = 4;
/// The Session Tokens panel is this many rows high: two borders and five text
/// rows.
pub const SESSION_TOKENS_HEIGHT: u16 = 7;
/// The rows the two panels take from the Sessions pane.
pub const SESSION_PANELS_HEIGHT: u16 = SESSION_TIME_HEIGHT + SESSION_TOKENS_HEIGHT;
/// The Sessions pane keeps at least this many rows when the panels are shown:
/// two borders and three list rows.
pub const MIN_SESSIONS_HEIGHT_WITH_PANELS: u16 = 6;
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
/// The turns sidebar is this many columns wide (product §5.1).
pub const SIDEBAR_WIDTH: u16 = 30;
/// The columns between the sidebar and the transcript.
pub const SIDEBAR_GAP: u16 = 1;
/// The sidebar is hidden on a screen narrower than this: 90 columns leave the
/// transcript 59.
pub const SIDEBAR_MIN_SCREEN_WIDTH: u16 = 90;

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
    /// Whether the selector's search is under way. Its prompt leaves the
    /// Sessions pane, which lists the results, to the pointer (see
    /// [`hit_test`]); the layout itself does not depend on it.
    pub selector_search: bool,
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
/// [`Id::EmptyState`] is visible, filling the screen. Otherwise, when the
/// selector is the base screen, [`Id::ScreenBackground`] comes first and
/// fills the screen. The selector overlay is [`Id::SelectorFrame`]: as wide
/// as the screen up to 160 columns and centred, with one empty row above and
/// below. Inside it come the tabs, the two panes, the Working Directory box
/// (see [`selector_areas`]) and [`Id::HelpLine`]: one row, two rows above the
/// bottom border, from column 5 to width - 6. An active prompt ([`LayoutInput::prompt`]) is [`Id::Prompt`]
/// in the same place instead of the help line; over the browser alone it
/// takes the browser's help line.
///
/// The browser is the header, the turns sidebar (from 90 columns on, and not
/// while the empty-session message replaces the transcript), the transcript
/// and the help line; see [`browser_areas`].
///
/// The help overlay and the details overlay, when `input` has one, come after
/// the screen (see [`help_overlay_rect`]). The toast, when `input` has one,
/// comes last (on top) in the bottom-right corner of the screen, except on the
/// too-small screen.
pub fn layout(input: &LayoutInput, area: Rect) -> ScreenLayout {
    if too_small(area) {
        return ScreenLayout {
            areas: vec![(Id::EmptyState, area)],
        };
    }
    let mut areas = Vec::new();
    if input.screen.has_browser() {
        let browser = browser_areas(area);
        // The empty-session message takes the whole width and the sidebar is
        // not drawn; the transcript's own area is the narrow one either way
        // (see `browser_areas`).
        let (body, body_area) = if input.session_empty {
            (Id::EmptySession, browser.body)
        } else {
            (Id::Transcript, browser.transcript)
        };
        // A prompt takes the browser's help line only when no selector is
        // over it: then it takes the selector's.
        let help = if input.prompt && !input.screen.has_selector() {
            Id::Prompt
        } else {
            Id::BrowserHelpLine
        };
        areas.push((Id::BrowserHeader, browser.header));
        if let Some(sidebar) = browser.sidebar.filter(|_| !input.session_empty) {
            areas.push((Id::TurnSidebar, sidebar));
            areas.extend(browser.sidebar_gap.map(|gap| (Id::SidebarGap, gap)));
        }
        areas.extend([(body, body_area), (help, browser.help_line)]);
    }
    if input.screen == Screen::Selector {
        areas.push((Id::ScreenBackground, area));
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
        ]);
        if let Some(panels) = selector.panels {
            areas.extend([
                (Id::SessionTime, panels.session_time),
                (Id::SessionTokens, panels.session_tokens),
            ]);
        }
        areas.extend([
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
    /// The turns sidebar: [`SIDEBAR_WIDTH`] columns at the left of the body,
    /// as tall as the transcript. `None` on a screen narrower than
    /// [`SIDEBAR_MIN_SCREEN_WIDTH`].
    pub sidebar: Option<Rect>,
    /// The [`SIDEBAR_GAP`] columns between the sidebar and the transcript, as
    /// tall as both. They belong to no pane: [`Id::SidebarGap`] only paints
    /// their background, and the mouse router finds nothing there.
    pub sidebar_gap: Option<Rect>,
    /// The whole body: from the row under the header to the row above the
    /// help line's blank separator row, full width. The empty-session message
    /// takes it.
    pub body: Rect,
    /// The transcript: the body, less the sidebar and the gap after it when
    /// the sidebar is shown. Its size is the transcript's viewport.
    pub transcript: Rect,
    /// The help line: the last two rows, full width. Its text is on the last
    /// row; the row above it is blank.
    pub help_line: Rect,
}

/// Places the browser's components in a screen of `area`, which is at least
/// [`MIN_WIDTH`] x [`MIN_HEIGHT`]: the header on rows 0 to 4, the body on
/// rows 5 to `height - 3` and the help line on the last two rows.
///
/// From [`SIDEBAR_MIN_SCREEN_WIDTH`] columns on the body starts with the
/// 30-column sidebar and a 1-column gap; the transcript is the rest. Whether
/// the sidebar is shown depends on the width alone, so the transcript's
/// viewport is the same whether or not the session turns out to be empty.
pub fn browser_areas(area: Rect) -> BrowserAreas {
    let header_height = HEADER_HEIGHT.min(area.height);
    let help_height = BROWSER_HELP_HEIGHT.min(area.height - header_height);
    let body_height = area.height - header_height - help_height;
    let body = Rect::new(area.x, area.y + header_height, area.width, body_height);
    let (sidebar, sidebar_gap, transcript) = if area.width >= SIDEBAR_MIN_SCREEN_WIDTH {
        let taken = SIDEBAR_WIDTH + SIDEBAR_GAP;
        (
            Some(Rect::new(body.x, body.y, SIDEBAR_WIDTH, body.height)),
            Some(Rect::new(
                body.x + SIDEBAR_WIDTH,
                body.y,
                SIDEBAR_GAP,
                body.height,
            )),
            Rect::new(body.x + taken, body.y, body.width - taken, body.height),
        )
    } else {
        (None, None, body)
    };
    BrowserAreas {
        header: Rect::new(area.x, area.y, area.width, header_height),
        sidebar,
        sidebar_gap,
        body,
        transcript,
        help_line: Rect::new(area.x, area.bottom() - help_height, area.width, help_height),
    }
}

/// The two panels below the Sessions pane (product §3.1). They come as a pair, so
/// one cannot be present without the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionPanelAreas {
    /// `Session Time`, directly below the Sessions pane.
    pub session_time: Rect,
    /// `Session Tokens`, directly below `Session Time`; its bottom is the
    /// Projects pane's.
    pub session_tokens: Rect,
}

/// The areas inside the selector overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectorAreas {
    /// The tab labels and, below them, the underline: two rows.
    pub tabs: Rect,
    pub projects: Rect,
    /// The Sessions pane: shorter than the Projects pane by the height of the
    /// panels when they are shown.
    pub sessions: Rect,
    /// The session panels, when the Sessions pane keeps three list rows
    /// without the rows they take.
    pub panels: Option<SessionPanelAreas>,
    pub working_dir: Rect,
    pub help_line: Rect,
}

/// Places the components inside the selector `overlay` (product §3.1).
///
/// The content area runs from column 4 to column `width - 5` of the overlay,
/// so its width is `width - 8`. The panes start on row 5 and are
/// `height - 14` rows high; the Projects pane is a third of the content
/// width (at least 20, at most 36 columns), then one column of gap, then the
/// Sessions pane. The Working Directory box follows directly, five rows
/// high.
///
/// When the panes are at least 17 rows high, the Sessions pane gives its last
/// 11 rows to `Session Time` (4 rows) and `Session Tokens` (7 rows), stacked
/// directly below it at its x and width. Below that neither is placed and the
/// Sessions pane keeps its full height.
pub fn selector_areas(overlay: Rect) -> SelectorAreas {
    let content_x = overlay.x + CONTENT_INSET;
    let content_width = overlay.width.saturating_sub(2 * CONTENT_INSET);
    let projects_width = (content_width / 3)
        .clamp(MIN_PROJECTS_WIDTH, MAX_PROJECTS_WIDTH)
        .min(content_width);
    let sessions_width = content_width.saturating_sub(projects_width + PANE_GAP);
    let panes_y = overlay.y + PANES_ROW;
    let panes_height = overlay.height.saturating_sub(PANES_HEIGHT_CHROME);
    let sessions_x = content_x + projects_width + PANE_GAP;
    let panels =
        (panes_height >= MIN_SESSIONS_HEIGHT_WITH_PANELS + SESSION_PANELS_HEIGHT).then(|| {
            let time_y = panes_y + panes_height - SESSION_PANELS_HEIGHT;
            SessionPanelAreas {
                session_time: Rect::new(sessions_x, time_y, sessions_width, SESSION_TIME_HEIGHT),
                session_tokens: Rect::new(
                    sessions_x,
                    time_y + SESSION_TIME_HEIGHT,
                    sessions_width,
                    SESSION_TOKENS_HEIGHT,
                ),
            }
        });
    let sessions_height = if panels.is_some() {
        panes_height - SESSION_PANELS_HEIGHT
    } else {
        panes_height
    };
    SelectorAreas {
        tabs: Rect::new(
            overlay.x + TABS_COLUMN,
            overlay.y + TABS_ROW,
            overlay.width.saturating_sub(2 * TABS_COLUMN),
            2,
        ),
        projects: Rect::new(content_x, panes_y, projects_width, panes_height),
        sessions: Rect::new(sessions_x, panes_y, sessions_width, sessions_height),
        panels,
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

/// The columns an overlay leaves free on a screen: two either side.
pub const OVERLAY_MARGIN_COLUMNS: u16 = 4;

/// The most columns an overlay can be wide on a screen `screen_width` wide.
pub fn overlay_max_width(screen_width: u16) -> u16 {
    screen_width.saturating_sub(OVERLAY_MARGIN_COLUMNS)
}

/// The help overlay, the details overlay and the theme selector: `wanted`
/// (its widest entry and its content, with the border) but at most the
/// screen less two columns either side and one row above and below, and
/// centred.
pub fn help_overlay_rect(area: Rect, wanted: Size) -> Rect {
    let width = wanted.width.min(overlay_max_width(area.width));
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

/// Where a pointer event goes, as [`hit_test`] decides it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerTarget {
    /// The topmost component that takes the pointer and contains the point,
    /// with its area. [`Id::Transcript`] stands for the transcript on top.
    Component(Id, Rect),
    /// A left press outside the topmost overlay: it goes to that overlay
    /// (with its area), which decides whether it closes.
    Outside(Id, Rect),
    /// No component gets the event.
    Nothing,
}

/// The ids of the overlays: the components under the topmost of them get no
/// pointer events.
const OVERLAYS: [Id; 4] = [
    Id::SelectorFrame,
    Id::HelpOverlay,
    Id::DetailsOverlay,
    Id::ThemeSelector,
];

/// The components that only paint: the pointer goes through them.
const PAINT_ONLY: [Id; 2] = [Id::SidebarGap, Id::ScreenBackground];

/// The topmost overlay and its area, or `None` if none is open.
pub fn topmost_overlay(layout: &ScreenLayout) -> Option<(Id, Rect)> {
    layout
        .areas()
        .iter()
        .rfind(|(id, _)| OVERLAYS.contains(id))
        .copied()
}

/// The component drawn topmost at (`column`, `row`) and its area, or `None`
/// if no component is there, whether or not it takes the pointer.
///
/// Components are drawn in `layout`'s order, so the last one that contains
/// the point is on top. [`Id::SidebarGap`] and [`Id::ScreenBackground`] only
/// paint, so they are never found and hide nothing. Components under the
/// topmost overlay are not candidates, even where the overlay does not cover
/// them.
pub fn component_at(layout: &ScreenLayout, column: u16, row: u16) -> Option<(Id, Rect)> {
    topmost_at(layout.areas(), column, row)
}

/// [`component_at`] over `areas`, in drawing order.
fn topmost_at(areas: &[(Id, Rect)], column: u16, row: u16) -> Option<(Id, Rect)> {
    let first = areas
        .iter()
        .rposition(|(id, _)| OVERLAYS.contains(id))
        .unwrap_or(0);
    areas[first..]
        .iter()
        .rev()
        .find(|(id, area)| !PAINT_ONLY.contains(id) && area.contains((column, row).into()))
        .copied()
}

/// Where the pointer event `event` goes on the screen `layout`, laid out
/// from `input`: the one place that decides which component is on top
/// (architecture.md#events-focus-and-keys, as amended by the cycle 5 spec,
/// §0 item 6).
///
/// - With an overlay open, a left press outside the topmost one goes to that
///   overlay ([`PointerTarget::Outside`]); a wheel step there reaches
///   nothing.
/// - Otherwise the component drawn on top at the point ([`component_at`])
///   gets it, if it takes the pointer. One that does not (the toast, a help
///   line, a panel, the selector's frame) blocks what is under it.
///
/// Which components take the pointer depends on the screen, not on any one
/// component: the overlays always do; while a prompt is open nothing else
/// does, except the Sessions pane during the selector's search
/// ([`LayoutInput::selector_search`]); otherwise the harness tabs, the two
/// panes, the turns sidebar and the transcript do. What the event then
/// means is the component's to decide.
pub fn hit_test(layout: &ScreenLayout, input: &LayoutInput, event: MouseEvent) -> PointerTarget {
    let (column, row) = (event.column, event.row);
    if let Some((overlay, area)) = topmost_overlay(layout)
        && !area.contains((column, row).into())
    {
        return if event.kind == MouseEventKind::Down(MouseButton::Left) {
            PointerTarget::Outside(overlay, area)
        } else {
            PointerTarget::Nothing
        };
    }
    match component_at(layout, column, row) {
        Some((id, area)) if takes_pointer(id, input) => PointerTarget::Component(id, area),
        _ => PointerTarget::Nothing,
    }
}

/// Whether `id` takes the pointer on a screen laid out from `input`.
fn takes_pointer(id: Id, input: &LayoutInput) -> bool {
    match id {
        Id::HelpOverlay | Id::DetailsOverlay | Id::ThemeSelector => true,
        Id::SessionList => !input.prompt || input.selector_search,
        Id::HarnessTabs | Id::ProjectList | Id::TurnSidebar | Id::Transcript => !input.prompt,
        _ => false,
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
        // The two panels take 11 rows of the Sessions pane.
        assert_eq!(a.sessions.height, 13);
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
        assert_eq!(a.sessions, Rect::new(41, 6, 75, 13));
        let panels = a.panels.unwrap();
        assert_eq!(panels.session_time, Rect::new(41, 19, 75, 4));
        assert_eq!(panels.session_tokens, Rect::new(41, 23, 75, 7));
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
        let ids: Vec<Id> = at(80, 40).areas().iter().map(|(id, _)| *id).collect();
        assert_eq!(
            ids,
            [
                Id::ScreenBackground,
                Id::SelectorFrame,
                Id::HarnessTabs,
                Id::ProjectList,
                Id::SessionList,
                Id::SessionTime,
                Id::SessionTokens,
                Id::WorkingDir,
                Id::HelpLine
            ]
        );
    }

    #[test]
    fn the_screen_background_fills_the_base_selector_screen_only() {
        let area = Rect::new(3, 2, 100, 30);
        let on = |screen| {
            let input = LayoutInput {
                screen,
                ..LayoutInput::default()
            };
            layout(&input, area).area(Id::ScreenBackground)
        };
        assert_eq!(on(Screen::Selector), Some(area));
        assert_eq!(on(Screen::BrowserSelector), None);
        assert_eq!(on(Screen::Browser), None);
        assert_eq!(at(59, 24).area(Id::ScreenBackground), None, "too small");
    }

    #[test]
    fn without_the_panels_the_layout_lists_neither() {
        let ids: Vec<Id> = at(80, 32).areas().iter().map(|(id, _)| *id).collect();
        assert_eq!(
            ids,
            [
                Id::ScreenBackground,
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
        assert_eq!(l.area(Id::TurnSidebar), Some(Rect::new(0, 5, 30, 33)));
        assert_eq!(l.area(Id::Transcript), Some(Rect::new(31, 5, 89, 33)));
        assert_eq!(l.area(Id::BrowserHelpLine), Some(Rect::new(0, 38, 120, 2)));
        assert_eq!(l.area(Id::EmptySession), None);
        assert_eq!(l.area(Id::SelectorFrame), None);
        let ids: Vec<Id> = l.areas().iter().map(|(id, _)| *id).collect();
        assert_eq!(
            ids,
            [
                Id::BrowserHeader,
                Id::TurnSidebar,
                Id::SidebarGap,
                Id::Transcript,
                Id::BrowserHelpLine
            ]
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
    fn the_sidebar_is_hidden_at_89_columns_and_shown_at_90() {
        let narrow = browser(89, 24, false);
        assert_eq!(narrow.area(Id::TurnSidebar), None);
        assert_eq!(narrow.area(Id::Transcript), Some(Rect::new(0, 5, 89, 17)));
        let wide = browser(90, 24, false);
        assert_eq!(wide.area(Id::TurnSidebar), Some(Rect::new(0, 5, 30, 17)));
        // 90 columns leave the transcript 59.
        assert_eq!(wide.area(Id::Transcript), Some(Rect::new(31, 5, 59, 17)));
    }

    #[test]
    fn below_90_columns_the_layout_is_what_it_was_without_a_sidebar() {
        for (w, h) in [(60, 20), (80, 24), (89, 40)] {
            let l = browser(w, h, false);
            let ids: Vec<Id> = l.areas().iter().map(|(id, _)| *id).collect();
            assert_eq!(
                ids,
                [Id::BrowserHeader, Id::Transcript, Id::BrowserHelpLine],
                "{w}x{h}"
            );
            assert_eq!(
                l.area(Id::Transcript),
                Some(Rect::new(0, 5, w, h - 7)),
                "{w}x{h}: the full width"
            );
        }
    }

    #[test]
    fn the_sidebar_is_30_columns_with_a_one_column_gap_and_as_tall_as_the_transcript() {
        for (w, h) in [(90, 20), (120, 40), (200, 50)] {
            let l = browser(w, h, false);
            let sidebar = l.area(Id::TurnSidebar).unwrap();
            let transcript = l.area(Id::Transcript).unwrap();
            assert_eq!(sidebar.width, SIDEBAR_WIDTH, "{w}x{h}");
            assert_eq!(SIDEBAR_WIDTH, 30);
            assert_eq!(SIDEBAR_GAP, 1);
            assert_eq!(transcript.x, sidebar.right() + 1, "{w}x{h}: one gap column");
            assert_eq!(transcript.right(), w, "{w}x{h}: to the right edge");
            assert_eq!(
                (sidebar.y, sidebar.height),
                (transcript.y, transcript.height),
                "{w}x{h}: exactly as tall as the transcript"
            );
            assert_eq!((sidebar.y, sidebar.bottom()), (5, h - 2), "{w}x{h}");
        }
    }

    #[test]
    fn the_header_and_the_help_line_stay_full_width_next_to_the_sidebar() {
        for w in [89, 90, 120] {
            let l = browser(w, 30, false);
            assert_eq!(l.area(Id::BrowserHeader).unwrap().width, w, "{w}");
            assert_eq!(l.area(Id::BrowserHelpLine).unwrap().width, w, "{w}");
            assert_eq!(l.area(Id::BrowserHelpLine).unwrap().x, 0, "{w}");
        }
    }

    #[test]
    fn the_gap_is_its_own_area_between_the_sidebar_and_the_transcript() {
        let l = browser(120, 40, false);
        assert_eq!(l.area(Id::SidebarGap), Some(Rect::new(30, 5, 1, 33)));
        assert_eq!(browser(89, 40, false).area(Id::SidebarGap), None);
        assert_eq!(browser(120, 40, true).area(Id::SidebarGap), None);
    }

    #[test]
    fn the_sidebar_is_drawn_after_the_header_and_before_the_transcript() {
        let ids: Vec<Id> = browser(120, 40, false)
            .areas()
            .iter()
            .map(|(id, _)| *id)
            .collect();
        assert_eq!(
            ids,
            [
                Id::BrowserHeader,
                Id::TurnSidebar,
                Id::SidebarGap,
                Id::Transcript,
                Id::BrowserHelpLine
            ]
        );
    }

    #[test]
    fn an_empty_session_has_no_sidebar_and_the_message_keeps_the_full_width() {
        let l = browser(120, 40, true);
        assert_eq!(l.area(Id::TurnSidebar), None);
        assert_eq!(l.area(Id::Transcript), None);
        assert_eq!(l.area(Id::EmptySession), Some(Rect::new(0, 5, 120, 33)));
    }

    #[test]
    fn the_transcript_area_is_the_narrow_one_whether_or_not_the_session_is_empty() {
        // The viewport is announced from `browser_areas`; it must not depend
        // on the session being empty, which can change without a resize.
        let areas = browser_areas(Rect::new(0, 0, 120, 40));
        assert_eq!(areas.transcript, Rect::new(31, 5, 89, 33));
        assert_eq!(areas.body, Rect::new(0, 5, 120, 33));
        assert_eq!(areas.sidebar, Some(Rect::new(0, 5, 30, 33)));
        assert_eq!(areas.sidebar_gap, Some(Rect::new(30, 5, 1, 33)));
        let narrow = browser_areas(Rect::new(0, 0, 89, 40));
        assert_eq!(narrow.sidebar, None);
        assert_eq!(narrow.sidebar_gap, None);
        assert_eq!(narrow.transcript, narrow.body);
    }

    #[test]
    fn the_sidebar_follows_the_area_origin() {
        let input = LayoutInput {
            screen: Screen::Browser,
            ..LayoutInput::default()
        };
        let l = layout(&input, Rect::new(3, 2, 100, 24));
        assert_eq!(l.area(Id::TurnSidebar), Some(Rect::new(3, 7, 30, 17)));
        assert_eq!(l.area(Id::Transcript), Some(Rect::new(34, 7, 69, 17)));
    }

    #[test]
    fn the_sidebar_is_under_the_selector_overlay_in_drawing_order() {
        let input = LayoutInput {
            screen: Screen::BrowserSelector,
            ..LayoutInput::default()
        };
        let l = layout(&input, Rect::new(0, 0, 120, 40));
        let ids: Vec<Id> = l.areas().iter().map(|(id, _)| *id).collect();
        let sidebar = ids.iter().position(|i| *i == Id::TurnSidebar).unwrap();
        let frame = ids.iter().position(|i| *i == Id::SelectorFrame).unwrap();
        assert!(sidebar < frame);
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
                Id::TurnSidebar,
                Id::SidebarGap,
                Id::Transcript,
                Id::BrowserHelpLine,
                Id::SelectorFrame,
                Id::HarnessTabs,
                Id::ProjectList,
                Id::SessionList,
                Id::SessionTime,
                Id::SessionTokens,
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

    // -- the session panels (product §3.1)

    fn panels(width: u16, height: u16) -> SessionPanelAreas {
        areas(width, height).panels.expect("the panels are shown")
    }

    #[test]
    fn the_panels_are_stacked_below_the_sessions_pane_at_its_x_and_width() {
        for (w, h) in [
            (60, 33),
            (80, 40),
            (120, 40),
            (160, 50),
            (200, 50),
            (201, 60),
        ] {
            let a = areas(w, h);
            let p = panels(w, h);
            assert_eq!(p.session_time.x, a.sessions.x, "{w}x{h}");
            assert_eq!(p.session_time.width, a.sessions.width, "{w}x{h}");
            assert_eq!(p.session_tokens.x, a.sessions.x, "{w}x{h}");
            assert_eq!(p.session_tokens.width, a.sessions.width, "{w}x{h}");
            assert_eq!(p.session_time.y, a.sessions.bottom(), "{w}x{h}: no gap");
            assert_eq!(p.session_tokens.y, p.session_time.bottom(), "{w}x{h}");
            assert_eq!(p.session_tokens.bottom(), a.projects.bottom(), "{w}x{h}");
        }
    }

    #[test]
    fn session_time_is_4_rows_and_session_tokens_7_and_the_sessions_pane_loses_11() {
        assert_eq!(SESSION_TIME_HEIGHT, 4);
        assert_eq!(SESSION_TOKENS_HEIGHT, 7);
        for (w, h) in [(60, 33), (80, 40), (120, 40), (200, 50), (160, 60)] {
            let a = areas(w, h);
            let p = panels(w, h);
            assert_eq!(p.session_time.height, 4, "{w}x{h}");
            assert_eq!(p.session_tokens.height, 7, "{w}x{h}");
            assert_eq!(a.sessions.height + 11, a.projects.height, "{w}x{h}");
        }
    }

    #[test]
    fn sizes_of_the_sessions_pane_with_the_panels() {
        let a = areas(160, 50);
        assert_eq!((a.sessions.width, a.sessions.height), (115, 23));
        let a = areas(120, 40);
        assert_eq!((a.sessions.width, a.sessions.height), (75, 13));
        let a = areas(60, 33);
        assert_eq!((a.sessions.width, a.sessions.height), (31, 6));
    }

    #[test]
    fn at_33_rows_both_panels_are_shown_with_3_list_rows() {
        let a = areas(60, 33);
        assert_eq!(a.projects.height, 17);
        assert_eq!(a.sessions.height, 6);
        assert_eq!(crate::components::visible_rows(a.sessions.height), 3);
        assert!(a.panels.is_some());
    }

    #[test]
    fn at_32_rows_neither_panel_is_shown_and_the_sessions_pane_keeps_its_height() {
        let a = areas(60, 32);
        assert_eq!(a.panels, None);
        assert_eq!(a.sessions.height, a.projects.height);
        assert_eq!(a.sessions.height, 16);
        let l = at(60, 32);
        assert_eq!(l.area(Id::SessionTime), None);
        assert_eq!(l.area(Id::SessionTokens), None);
        let l = at(60, 33);
        assert!(l.area(Id::SessionTime).is_some());
        assert!(l.area(Id::SessionTokens).is_some());
    }

    #[test]
    fn one_panel_is_never_present_without_the_other() {
        for w in [60, 120, 200] {
            for h in MIN_HEIGHT..=60 {
                let l = at(w, h);
                assert_eq!(
                    l.area(Id::SessionTime).is_some(),
                    l.area(Id::SessionTokens).is_some(),
                    "{w}x{h}"
                );
                assert_eq!(
                    l.area(Id::SessionTime).is_some(),
                    h >= 33,
                    "{w}x{h}: shown from 33 rows"
                );
            }
        }
    }

    #[test]
    fn the_other_areas_do_not_move_with_the_panels() {
        // The same overlay, with the room for the panels and without: the
        // panes' height is what decides, so compare against the geometry.
        for (w, h) in [(60, 32), (60, 33), (120, 40), (200, 50), (200, 60)] {
            let l = at(w, h);
            let overlay = l.area(Id::SelectorFrame).unwrap();
            let a = selector_areas(overlay);
            let panes_height = overlay.height - PANES_HEIGHT_CHROME;
            assert_eq!(a.projects.height, panes_height, "{w}x{h}");
            assert_eq!(a.projects.y, overlay.y + PANES_ROW, "{w}x{h}");
            assert_eq!(a.working_dir.y, overlay.y + PANES_ROW + panes_height);
            assert_eq!(a.working_dir.height, WORKING_DIR_HEIGHT);
            assert_eq!(
                a.help_line.y,
                overlay.bottom() - HELP_LINE_ROWS_FROM_BOTTOM,
                "{w}x{h}"
            );
            assert_eq!(a.sessions.y, a.projects.y, "{w}x{h}");
        }
    }

    fn selector_rects(a: &SelectorAreas) -> Vec<(&'static str, Rect)> {
        let mut rects = vec![
            ("tabs", a.tabs),
            ("projects", a.projects),
            ("sessions", a.sessions),
            ("working_dir", a.working_dir),
            ("help_line", a.help_line),
        ];
        if let Some(p) = a.panels {
            rects.push(("session_time", p.session_time));
            rects.push(("session_tokens", p.session_tokens));
        }
        rects
    }

    #[test]
    fn no_selector_areas_overlap_at_any_size() {
        for w in MIN_WIDTH..=200 {
            for h in MIN_HEIGHT..=60 {
                let overlay = at(w, h).area(Id::SelectorFrame).unwrap();
                let a = selector_areas(overlay);
                let rects = selector_rects(&a);
                for (i, (name, rect)) in rects.iter().enumerate() {
                    assert!(
                        overlay.contains((rect.x, rect.y).into())
                            && rect.right() <= overlay.right()
                            && rect.bottom() <= overlay.bottom(),
                        "{name} outside the overlay at {w}x{h}"
                    );
                    for (other_name, other) in &rects[i + 1..] {
                        assert!(
                            !rect.intersects(*other),
                            "{name} {rect:?} overlaps {other_name} {other:?} at {w}x{h}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_panels_follow_a_centred_overlay_on_a_wide_terminal() {
        let l = at(200, 50);
        let overlay = l.area(Id::SelectorFrame).unwrap();
        assert_eq!(overlay.x, 20);
        let sessions = l.area(Id::SessionList).unwrap();
        let time = l.area(Id::SessionTime).unwrap();
        let tokens = l.area(Id::SessionTokens).unwrap();
        assert_eq!(sessions.x, 20 + 4 + 36 + 1);
        assert_eq!((time.x, time.width), (sessions.x, sessions.width));
        assert_eq!((tokens.x, tokens.width), (sessions.x, sessions.width));
        assert_eq!(tokens.right(), overlay.right() - 4);
        assert_eq!(tokens.bottom(), l.area(Id::ProjectList).unwrap().bottom());
    }

    #[test]
    fn the_layout_follows_the_area_origin_for_the_panels() {
        let l = layout(&LayoutInput::default(), Rect::new(3, 2, 80, 40));
        let sessions = l.area(Id::SessionList).unwrap();
        let time = l.area(Id::SessionTime).unwrap();
        assert_eq!(
            time,
            Rect::new(sessions.x, sessions.bottom(), sessions.width, 4)
        );
    }

    // -- hit_test

    const PRESS: MouseEventKind = MouseEventKind::Down(MouseButton::Left);

    fn event(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            modifiers: tuirealm::event::KeyModifiers::NONE,
            column,
            row,
        }
    }

    /// `hit_test` of a left press and of a wheel step at (`column`, `row`)
    /// of a screen of `size` laid out from `input`.
    fn targets(
        input: &LayoutInput,
        size: (u16, u16),
        column: u16,
        row: u16,
    ) -> (PointerTarget, PointerTarget) {
        let l = layout(input, Rect::new(0, 0, size.0, size.1));
        (
            hit_test(&l, input, event(PRESS, column, row)),
            hit_test(&l, input, event(MouseEventKind::ScrollDown, column, row)),
        )
    }

    fn press_at(input: &LayoutInput, size: (u16, u16), column: u16, row: u16) -> PointerTarget {
        targets(input, size, column, row).0
    }

    /// The id `target` gives the event to, if it is a component.
    fn component(target: PointerTarget) -> Option<Id> {
        match target {
            PointerTarget::Component(id, _) => Some(id),
            _ => None,
        }
    }

    fn browser_input() -> LayoutInput {
        LayoutInput {
            screen: Screen::Browser,
            ..LayoutInput::default()
        }
    }

    fn sized(width: u16, height: u16) -> Size {
        Size::new(width, height)
    }

    #[test]
    fn a_component_that_only_paints_is_never_found() {
        let screen = Rect::new(0, 0, 20, 10);
        let gap = Rect::new(5, 0, 1, 10);
        for paint_only in [Id::SidebarGap, Id::ScreenBackground] {
            let areas = [(paint_only, screen)];
            assert_eq!(topmost_at(&areas, 3, 3), None, "{paint_only:?}");
            // Nor does it hide what is drawn under it.
            let areas = [(Id::Transcript, screen), (paint_only, gap)];
            assert_eq!(
                topmost_at(&areas, 5, 3),
                Some((Id::Transcript, screen)),
                "{paint_only:?}"
            );
        }
    }

    #[test]
    fn the_component_under_the_pointer_gets_it_with_its_area() {
        let input = browser_input();
        let l = layout(&input, Rect::new(0, 0, 120, 40));
        let transcript = l.area(Id::Transcript).unwrap();
        let sidebar = l.area(Id::TurnSidebar).unwrap();
        for kind in [PRESS, MouseEventKind::ScrollUp, MouseEventKind::ScrollDown] {
            assert_eq!(
                hit_test(&l, &input, event(kind, 60, 20)),
                PointerTarget::Component(Id::Transcript, transcript),
                "{kind:?}"
            );
            assert_eq!(
                hit_test(&l, &input, event(kind, 10, 20)),
                PointerTarget::Component(Id::TurnSidebar, sidebar),
                "{kind:?}"
            );
        }
        let selector = layout(&LayoutInput::default(), Rect::new(0, 0, 80, 24));
        let tabs = selector.area(Id::HarnessTabs).unwrap();
        assert_eq!(
            hit_test(
                &selector,
                &LayoutInput::default(),
                event(PRESS, tabs.x, tabs.y)
            ),
            PointerTarget::Component(Id::HarnessTabs, tabs)
        );
    }

    #[test]
    fn the_topmost_component_wins_where_areas_overlap() {
        // The panes are drawn over the selector's frame.
        let selector = LayoutInput::default();
        let l = layout(&selector, Rect::new(0, 0, 80, 24));
        let projects = l.area(Id::ProjectList).unwrap();
        assert!(
            l.area(Id::SelectorFrame)
                .unwrap()
                .contains(projects.as_position())
        );
        assert_eq!(
            press_at(&selector, (80, 24), projects.x, projects.y),
            PointerTarget::Component(Id::ProjectList, projects)
        );
        // The help overlay is drawn over the transcript.
        let help = LayoutInput {
            help: Some(sized(40, 10)),
            ..browser_input()
        };
        let l = layout(&help, Rect::new(0, 0, 120, 40));
        let overlay = l.area(Id::HelpOverlay).unwrap();
        let (column, row) = (overlay.x + 2, overlay.y + 2);
        assert!(
            l.area(Id::Transcript)
                .unwrap()
                .contains((column, row).into())
        );
        let (press, wheel) = targets(&help, (120, 40), column, row);
        assert_eq!(press, PointerTarget::Component(Id::HelpOverlay, overlay));
        assert_eq!(wheel, PointerTarget::Component(Id::HelpOverlay, overlay));
    }

    #[test]
    fn a_press_on_the_toast_reaches_nothing() {
        // Over the transcript, over the browser's help line and over the
        // selector: what is under the toast does not get it either.
        for (input, size) in [
            (browser_input(), (120, 40)),
            (browser_input(), (80, 24)),
            (LayoutInput::default(), (80, 24)),
        ] {
            let input = LayoutInput {
                toast_text_width: Some(20),
                ..input
            };
            let l = layout(&input, Rect::new(0, 0, size.0, size.1));
            let toast = l.area(Id::Toast).unwrap();
            for row in toast.rows() {
                for column in row.columns() {
                    let (press, wheel) = targets(&input, size, column.x, column.y);
                    assert_eq!(press, PointerTarget::Nothing, "{size:?} {column:?}");
                    assert_eq!(wheel, PointerTarget::Nothing, "{size:?} {column:?}");
                }
            }
        }
        // The toast over the transcript, whose rows it covers.
        let input = LayoutInput {
            toast_text_width: Some(20),
            ..browser_input()
        };
        let l = layout(&input, Rect::new(0, 0, 120, 40));
        let toast = l.area(Id::Toast).unwrap();
        assert!(
            l.area(Id::Transcript)
                .unwrap()
                .contains(toast.as_position())
        );
        assert_eq!(
            component(press_at(&input, (120, 40), toast.x - 1, toast.y)),
            Some(Id::Transcript),
            "beside the toast"
        );
    }

    #[test]
    fn nothing_under_an_overlay_is_hit_even_where_it_does_not_cover_it() {
        let size = (120, 40);
        let overlays = [
            (
                Id::HelpOverlay,
                LayoutInput {
                    help: Some(sized(40, 10)),
                    ..browser_input()
                },
            ),
            (
                Id::DetailsOverlay,
                LayoutInput {
                    details: Some(sized(40, 10)),
                    ..browser_input()
                },
            ),
            (
                Id::ThemeSelector,
                LayoutInput {
                    theme_selector: Some(sized(40, 10)),
                    ..browser_input()
                },
            ),
            (
                Id::SelectorFrame,
                LayoutInput {
                    screen: Screen::BrowserSelector,
                    ..LayoutInput::default()
                },
            ),
        ];
        for (overlay, input) in overlays {
            let l = layout(&input, Rect::new(0, 0, size.0, size.1));
            let area = l.area(overlay).unwrap();
            for row in 0..size.1 {
                for column in 0..size.0 {
                    let (press, wheel) = targets(&input, size, column, row);
                    let hit = [component(press), component(wheel)];
                    if area.contains((column, row).into()) {
                        // The overlay, or what is drawn over it as part of
                        // it (the selector's panes and tabs).
                        for id in hit.into_iter().flatten() {
                            let ok = [overlay, Id::HarnessTabs, Id::ProjectList, Id::SessionList];
                            assert!(ok.contains(&id), "{overlay:?}: {id:?} at ({column}, {row})");
                        }
                    } else {
                        assert_eq!(hit, [None, None], "{overlay:?} at ({column}, {row})");
                    }
                }
            }
            // The browser is under each of them.
            let header = l.area(Id::BrowserHeader).unwrap();
            assert!(!area.contains(header.as_position()), "{overlay:?}");
            assert_eq!(
                press_at(&input, size, header.x, header.y),
                PointerTarget::Outside(overlay, area),
                "{overlay:?}"
            );
        }
    }

    #[test]
    fn a_press_outside_the_topmost_overlay_is_addressed_to_it_and_a_wheel_step_to_nothing() {
        // The help overlay over the selector over the browser.
        let input = LayoutInput {
            screen: Screen::BrowserSelector,
            help: Some(sized(30, 8)),
            ..LayoutInput::default()
        };
        let size = (120, 40);
        let l = layout(&input, Rect::new(0, 0, size.0, size.1));
        let help = l.area(Id::HelpOverlay).unwrap();
        let projects = l.area(Id::ProjectList).unwrap();
        assert!(!help.contains(projects.as_position()));
        // On the Projects pane, inside the selector but not the help.
        let (press, wheel) = targets(&input, size, projects.x, projects.y);
        assert_eq!(press, PointerTarget::Outside(Id::HelpOverlay, help));
        assert_eq!(wheel, PointerTarget::Nothing);
        // Off the selector too.
        let (press, wheel) = targets(&input, size, 0, 0);
        assert_eq!(press, PointerTarget::Outside(Id::HelpOverlay, help));
        assert_eq!(wheel, PointerTarget::Nothing);
        // The selector over the browser alone, with the base selector for
        // comparison: both get a press outside them.
        for screen in [Screen::BrowserSelector, Screen::Selector] {
            let input = LayoutInput {
                screen,
                ..LayoutInput::default()
            };
            let l = layout(&input, Rect::new(0, 0, 200, 50));
            let frame = l.area(Id::SelectorFrame).unwrap();
            let (press, wheel) = targets(&input, (200, 50), 5, 10);
            assert_eq!(press, PointerTarget::Outside(Id::SelectorFrame, frame));
            assert_eq!(wheel, PointerTarget::Nothing);
        }
    }

    #[test]
    fn a_press_on_a_toast_outside_the_overlay_is_outside_it() {
        // At 200 columns the toast is right of the centred selector.
        let input = LayoutInput {
            screen: Screen::BrowserSelector,
            toast_text_width: Some(10),
            ..LayoutInput::default()
        };
        let l = layout(&input, Rect::new(0, 0, 200, 50));
        let toast = l.area(Id::Toast).unwrap();
        let frame = l.area(Id::SelectorFrame).unwrap();
        assert!(!frame.contains(toast.as_position()));
        assert_eq!(
            press_at(&input, (200, 50), toast.x, toast.y),
            PointerTarget::Outside(Id::SelectorFrame, frame)
        );
    }

    #[test]
    fn inside_the_selector_only_the_tabs_and_the_panes_take_the_pointer() {
        let input = LayoutInput::default();
        let size = (200, 50);
        let l = layout(&input, Rect::new(0, 0, size.0, size.1));
        for id in [
            Id::SelectorFrame,
            Id::SessionTime,
            Id::SessionTokens,
            Id::WorkingDir,
            Id::HelpLine,
        ] {
            let area = l.area(id).unwrap();
            // A cell of the area that no other component is drawn over.
            let cell = area
                .positions()
                .find(|p| component_at(&l, p.x, p.y).map(|(found, _)| found) == Some(id))
                .unwrap();
            let (press, wheel) = targets(&input, size, cell.x, cell.y);
            assert_eq!(press, PointerTarget::Nothing, "{id:?}");
            assert_eq!(wheel, PointerTarget::Nothing, "{id:?}");
        }
    }

    #[test]
    fn during_the_selector_search_only_the_sessions_pane_takes_the_pointer() {
        let search = LayoutInput {
            prompt: true,
            selector_search: true,
            ..LayoutInput::default()
        };
        let size = (120, 40);
        let l = layout(&search, Rect::new(0, 0, size.0, size.1));
        let sessions = l.area(Id::SessionList).unwrap();
        let (press, wheel) = targets(&search, size, sessions.x + 2, sessions.y + 2);
        assert_eq!(press, PointerTarget::Component(Id::SessionList, sessions));
        assert_eq!(wheel, PointerTarget::Component(Id::SessionList, sessions));
        for id in [Id::ProjectList, Id::HarnessTabs, Id::Prompt] {
            let area = l.area(id).unwrap();
            let (press, wheel) = targets(&search, size, area.x + 1, area.y);
            assert_eq!(press, PointerTarget::Nothing, "{id:?}");
            assert_eq!(wheel, PointerTarget::Nothing, "{id:?}");
        }
    }

    #[test]
    fn during_any_other_prompt_nothing_under_the_overlays_takes_the_pointer() {
        let size = (120, 40);
        // The resume confirmation over the selector, base or over the
        // browser, and a prompt in the browser (search, save, resume).
        let prompts = [
            LayoutInput {
                prompt: true,
                ..LayoutInput::default()
            },
            LayoutInput {
                prompt: true,
                screen: Screen::BrowserSelector,
                ..LayoutInput::default()
            },
            LayoutInput {
                prompt: true,
                ..browser_input()
            },
        ];
        for input in prompts {
            for row in 0..size.1 {
                for column in 0..size.0 {
                    let (press, wheel) = targets(&input, size, column, row);
                    assert_eq!(component(press), None, "{input:?} at ({column}, {row})");
                    assert_eq!(component(wheel), None, "{input:?} at ({column}, {row})");
                }
            }
        }
        // The overlays still take it.
        let input = LayoutInput {
            prompt: true,
            help: Some(sized(40, 10)),
            ..browser_input()
        };
        let l = layout(&input, Rect::new(0, 0, size.0, size.1));
        let help = l.area(Id::HelpOverlay).unwrap();
        assert_eq!(
            press_at(&input, size, help.x + 1, help.y + 1),
            PointerTarget::Component(Id::HelpOverlay, help)
        );
    }

    #[test]
    fn the_gap_and_the_background_are_never_returned() {
        let screens = [
            (browser_input(), (120, 40)),
            (browser_input(), (90, 24)),
            (LayoutInput::default(), (80, 24)),
            (LayoutInput::default(), (200, 50)),
        ];
        for (input, size) in screens {
            let l = layout(&input, Rect::new(0, 0, size.0, size.1));
            for row in 0..size.1 {
                for column in 0..size.0 {
                    for kind in [PRESS, MouseEventKind::ScrollDown] {
                        let target = hit_test(&l, &input, event(kind, column, row));
                        let found = match target {
                            PointerTarget::Component(id, _) | PointerTarget::Outside(id, _) => {
                                Some(id)
                            }
                            PointerTarget::Nothing => None,
                        };
                        assert!(
                            !matches!(found, Some(Id::SidebarGap | Id::ScreenBackground)),
                            "{found:?} at ({column}, {row}) of {size:?}"
                        );
                    }
                }
            }
        }
        // The gap's own column reaches nothing.
        let input = browser_input();
        let gap = layout(&input, Rect::new(0, 0, 120, 40))
            .area(Id::SidebarGap)
            .unwrap();
        assert_eq!(
            targets(&input, (120, 40), gap.x, gap.y + 3),
            (PointerTarget::Nothing, PointerTarget::Nothing)
        );
    }

    #[test]
    fn a_terminal_that_is_too_small_takes_no_pointer() {
        assert_eq!(
            targets(&browser_input(), (59, 24), 10, 10),
            (PointerTarget::Nothing, PointerTarget::Nothing)
        );
    }

    #[test]
    fn the_message_of_an_empty_session_takes_no_pointer() {
        let input = LayoutInput {
            session_empty: true,
            ..browser_input()
        };
        assert_eq!(
            targets(&input, (120, 40), 60, 20),
            (PointerTarget::Nothing, PointerTarget::Nothing)
        );
    }
}
