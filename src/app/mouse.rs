//! Mouse routing (SPEC §1.4, §6.9): which component is under the pointer,
//! what the selector and the theme selector do with the clicks and the wheel,
//! and what a click outside an open overlay does.
//!
//! `GlobalEvents` turns every left press and wheel step into
//! [`Msg::Mouse`] (only when `mouse = true`). [`App::update`] finds the
//! component under the pointer with [`hit_test`] over the routing layout
//! (the pure [`layout`](super::layout::layout) function that `App::view`
//! uses too, cached in the `App` and refreshed whenever the screen, an
//! overlay or the size changes, never from `view`) and gives that component a
//! [`RoutedMouse`]: the same event with the position relative to the
//! component's area. The component turns it into a `Msg`.
//!
//! Why not hand the component the event itself? The application forwards
//! every raw `Event::Mouse` to the focused component, in screen coordinates,
//! and a component cannot tell those from routed ones.

use std::time::{Duration, Instant};

use ratatui::layout::{Rect, Size};
use tuirealm::event::{Event, MouseButton, MouseEvent, MouseEventKind};

use super::layout::{Screen, ScreenLayout, layout};
use super::{App, Id, Msg, Pane, UserEvent};
use crate::model::{ProjectId, SessionId};

/// Two presses on the same row at most this far apart are a double-click.
pub const DOUBLE_CLICK: Duration = Duration::from_millis(400);

/// A mouse event for the component under the pointer: the position is
/// relative to the component's area (its top-left cell is 0, 0), and `area`
/// is that area's size. Built by the `App`; no Port delivers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutedMouse {
    pub kind: MouseEventKind,
    pub column: u16,
    pub row: u16,
    pub area: Size,
}

impl RoutedMouse {
    /// `event` as seen from `area`, which must contain it.
    pub fn new(event: MouseEvent, area: Rect) -> Self {
        Self {
            kind: event.kind,
            column: event.column.saturating_sub(area.x),
            row: event.row.saturating_sub(area.y),
            area: area.as_size(),
        }
    }

    /// Whether this is the left button being pressed.
    pub fn is_left_press(&self) -> bool {
        self.kind == MouseEventKind::Down(MouseButton::Left)
    }
}

/// The ids of the overlays: the components under the topmost of them get no
/// mouse events.
const OVERLAYS: [Id; 4] = [
    Id::SelectorFrame,
    Id::HelpOverlay,
    Id::DetailsOverlay,
    Id::ThemeSelector,
];

/// The topmost overlay and its area, or `None` if none is open.
pub fn topmost_overlay(layout: &ScreenLayout) -> Option<(Id, Rect)> {
    layout
        .areas()
        .iter()
        .rfind(|(id, _)| OVERLAYS.contains(id))
        .copied()
}

/// The component drawn topmost at (`column`, `row`) and its area, or `None`
/// if no component is there.
///
/// Components are drawn in `layout`'s order, so the last one that contains
/// the point is on top. [`Id::SidebarGap`] only paints, so it is never found.
/// Components under the topmost overlay are not
/// candidates, even where the overlay does not cover them.
pub fn hit_test(layout: &ScreenLayout, column: u16, row: u16) -> Option<(Id, Rect)> {
    let areas = layout.areas();
    let first = areas
        .iter()
        .rposition(|(id, _)| OVERLAYS.contains(id))
        .unwrap_or(0);
    areas[first..]
        .iter()
        .rev()
        .find(|(id, area)| *id != Id::SidebarGap && area.contains((column, row).into()))
        .copied()
}

/// The last left press, for double-clicks.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ClickTarget {
    Project(ProjectId),
    Session(SessionId),
    Theme(String),
}

/// What the `App` keeps for the mouse: the layout it routes with, the last
/// press, and the clock the double-click is measured with.
pub(super) struct MouseState {
    /// The areas events are routed with.
    pub(super) routing: ScreenLayout,
    last_press: Option<(ClickTarget, Instant)>,
    /// Whether the left press being handled was on a row.
    row_pressed: bool,
    clock: Box<dyn Fn() -> Instant>,
}

impl Default for MouseState {
    fn default() -> Self {
        Self {
            routing: ScreenLayout::default(),
            last_press: None,
            row_pressed: false,
            clock: Box::new(Instant::now),
        }
    }
}

impl MouseState {
    pub(super) fn set_clock(&mut self, clock: impl Fn() -> Instant + 'static) {
        self.clock = Box::new(clock);
    }

    /// Records a left press on `target`. Returns whether it completes a
    /// double-click: the previous press was on the same target at most
    /// [`DOUBLE_CLICK`] ago. A completed double-click is forgotten, so a
    /// third press starts a new one.
    fn press(&mut self, target: ClickTarget) -> bool {
        self.row_pressed = true;
        let now = (self.clock)();
        let double = self.last_press.as_ref().is_some_and(|(last, at)| {
            *last == target && now.saturating_duration_since(*at) <= DOUBLE_CLICK
        });
        self.last_press = (!double).then_some((target, now));
        double
    }
}

impl App {
    /// The areas mouse events are routed with.
    pub fn routing(&self) -> &ScreenLayout {
        &self.mouse.routing
    }

    /// Replaces the clock double-clicks are measured with (tests inject a
    /// fixed one).
    pub fn set_click_clock(&mut self, clock: impl Fn() -> Instant + 'static) {
        self.mouse.set_clock(clock);
    }

    /// Recomputes the routing areas from the current state. `update` calls
    /// it after every message (any of them can open an overlay or a prompt,
    /// or change the screen), `push_viewports` when the screen or the size
    /// changes, and `show_toast_for` when a toast appears; `view` never
    /// calls it. It is a handful of rectangles, so it is not worth finding
    /// out which messages can change the layout.
    pub(super) fn refresh_routing(&mut self) {
        let area = Rect::new(0, 0, self.size.0, self.size.1);
        self.mouse.routing = layout(&self.layout_input(), area);
    }

    /// A left press or a wheel step at a position of the screen: gives it to
    /// the component under the pointer, wherever the focus is.
    ///
    /// A left press that is not on a row (a tab, empty space, anywhere
    /// else) ends a double-click that was under way.
    pub(super) fn mouse(&mut self, event: MouseEvent) {
        if !self.config.mouse {
            return;
        }
        self.mouse.row_pressed = false;
        self.route_mouse(event);
        if event.kind == MouseEventKind::Down(MouseButton::Left) && !self.mouse.row_pressed {
            self.mouse.last_press = None;
        }
    }

    fn route_mouse(&mut self, event: MouseEvent) {
        if let Some((overlay, area)) = topmost_overlay(&self.mouse.routing)
            && !area.contains((event.column, event.row).into())
        {
            self.outside_overlay(overlay, event.kind);
            return;
        }
        let Some((id, area)) = hit_test(&self.mouse.routing, event.column, event.row) else {
            return;
        };
        // The layout places "the transcript"; the one on top gets the event.
        let id = if id == Id::Transcript {
            self.top_transcript()
        } else {
            id
        };
        if !self.takes_mouse(id, event.kind) {
            return;
        }
        if id == self.top_transcript() && event.kind == MouseEventKind::Down(MouseButton::Left) {
            self.focus_transcript(id);
        }
        let routed = Event::User(UserEvent::Mouse(RoutedMouse::new(event, area)));
        let msg = self
            .application
            .get_component_mut(&id)
            .and_then(|component| component.on(&routed));
        if let Some(msg) = msg {
            self.update(msg);
        }
    }

    /// A press or a wheel step outside the topmost overlay `overlay`. The
    /// wheel is ignored. A left press closes the overlay with the message
    /// `Esc` gives: `CloseHelp`, `CloseDetails`, `CancelTheme` (which gives
    /// the previous theme back) and, for the selector over a session,
    /// `CloseSelector` (which also ends the search whose prompt is open). At
    /// startup there is nothing to return to (closing would quit), and the
    /// resume confirmation is modal, so nothing happens then.
    fn outside_overlay(&mut self, overlay: Id, kind: MouseEventKind) {
        if kind != MouseEventKind::Down(MouseButton::Left) {
            return;
        }
        let msg = match overlay {
            Id::HelpOverlay => Msg::CloseHelp,
            Id::DetailsOverlay => Msg::CloseDetails,
            Id::ThemeSelector => Msg::CancelTheme,
            Id::SelectorFrame
                if self.screen == Screen::BrowserSelector && !self.resume.pending() =>
            {
                Msg::CloseSelector
            }
            _ => return,
        };
        self.update(msg);
    }

    /// A click on the transcript `id` gives it the focus, if something else
    /// has it.
    fn focus_transcript(&mut self, id: Id) {
        if self.application.focus() == Some(&id) {
            return;
        }
        if let Err(err) = self.application.active(&id) {
            log::warn!("could not focus the transcript: {err}");
        }
    }

    /// Whether `id` reacts to the mouse now. The overlays do. So does the
    /// transcript, unless a prompt has the input. The selector's tabs and
    /// panes do, unless a prompt has the input: the search prompt lets the
    /// Sessions pane's results be clicked and scrolled, any other (the
    /// resume confirmation) is modal. In the filtered selector the Projects
    /// pane is as at a start without the prefix: the wheel leaves it alone
    /// (a click ends the filter).
    fn takes_mouse(&self, id: Id, kind: MouseEventKind) -> bool {
        let prompt = self.application.mounted(&Id::Prompt);
        let wheel = matches!(kind, MouseEventKind::ScrollUp | MouseEventKind::ScrollDown);
        match id {
            Id::HelpOverlay | Id::DetailsOverlay | Id::ThemeSelector => true,
            Id::Transcript | Id::Subagent(_) => !prompt,
            Id::ProjectList if wheel && self.selector.filtered.is_some() => false,
            Id::SessionList => !prompt || self.selector.search.is_some(),
            Id::ProjectList | Id::HarnessTabs => !prompt,
            _ => false,
        }
    }

    // -- the theme selector's clicks

    /// A click on the theme `id`, where the theme selector's cursor is now: it
    /// is previewed. A double-click keeps it, like `Enter`.
    pub(super) fn theme_clicked(&mut self, id: &str) {
        if self.theme_selector.is_none() {
            return;
        }
        if self.mouse.press(ClickTarget::Theme(id.to_owned())) {
            self.select_theme(id);
        } else {
            self.preview_theme(id);
        }
    }

    // -- the selector's clicks

    /// A click on a project row: Projects gets the focus and the project is
    /// selected. A double-click is `Enter` on it: Sessions gets the focus
    /// and, with none selected, its newest session.
    pub(super) fn project_clicked(&mut self, id: ProjectId) {
        let double = self.mouse.press(ClickTarget::Project(id.clone()));
        // Focus first: in the filtered selector it ends the filter, which
        // resets the project.
        self.focus_pane(Pane::Projects);
        self.project_selected(id);
        if double {
            self.focus_pane(Pane::Sessions);
        }
    }

    /// A click on a session row: Sessions gets the focus and the session is
    /// selected. A double-click opens it.
    pub(super) fn session_clicked(&mut self, id: SessionId) {
        let double = self.mouse.press(ClickTarget::Session(id.clone()));
        // Select first: focusing Sessions with nothing selected would select
        // the newest session, which resets the scroll of the list and moves
        // the clicked row from under the pointer.
        self.session_selected(id.clone());
        self.focus_pane(Pane::Sessions);
        if double && let Some(harness) = self.selector.current {
            self.emit(Msg::OpenSession(harness, id));
        }
    }
}
