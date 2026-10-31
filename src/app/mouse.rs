//! Mouse routing (SPEC §1.4, §6.9): which component is under the pointer,
//! and what the selector does with the clicks and the wheel.
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

use super::layout::{ScreenLayout, layout};
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
const OVERLAYS: [Id; 3] = [Id::SelectorFrame, Id::HelpOverlay, Id::ThemeSelector];

/// The component drawn topmost at (`column`, `row`) and its area, or `None`
/// if no component is there.
///
/// Components are drawn in `layout`'s order, so the last one that contains
/// the point is on top. Components under the topmost overlay are not
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
        .find(|(_, area)| area.contains((column, row).into()))
        .copied()
}

/// The last left press, for double-clicks.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ClickTarget {
    Project(ProjectId),
    Session(SessionId),
}

/// What the `App` keeps for the mouse: the layout it routes with, the last
/// press, and the clock the double-click is measured with.
pub(super) struct MouseState {
    /// The areas events are routed with.
    pub(super) routing: ScreenLayout,
    last_press: Option<(ClickTarget, Instant)>,
    clock: Box<dyn Fn() -> Instant>,
}

impl Default for MouseState {
    fn default() -> Self {
        Self {
            routing: ScreenLayout::default(),
            last_press: None,
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

    /// Recomputes the routing areas. Called when the screen, an overlay, the
    /// size or the toast changes; `view` never calls it.
    pub(super) fn refresh_routing(&mut self) {
        let area = Rect::new(0, 0, self.size.0, self.size.1);
        self.mouse.routing = layout(&self.layout_input(), area);
    }

    /// A left press or a wheel step at a position of the screen: gives it to
    /// the component under the pointer, wherever the focus is.
    pub(super) fn mouse(&mut self, event: MouseEvent) {
        if !self.config.mouse {
            return;
        }
        let Some((id, area)) = hit_test(&self.mouse.routing, event.column, event.row) else {
            return;
        };
        if !self.takes_mouse(id) {
            return;
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

    /// Whether `id` reacts to the mouse now. The selector's tabs and panes
    /// do, unless a prompt has the input: the search prompt lets the
    /// Sessions pane's results be clicked and scrolled, any other (the
    /// resume confirmation) is modal.
    fn takes_mouse(&self, id: Id) -> bool {
        let prompt = self.application.mounted(&Id::Prompt);
        match id {
            Id::SessionList => !prompt || self.selector.search.is_some(),
            Id::ProjectList | Id::HarnessTabs => !prompt,
            _ => false,
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
        self.focus_pane(Pane::Sessions);
        self.session_selected(id.clone());
        if double && let Some(harness) = self.selector.current {
            self.emit(Msg::OpenSession(harness, id));
        }
    }
}
