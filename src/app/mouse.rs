//! Pointer routing (architecture.md#events-focus-and-keys as amended by the
//! cycle 5 spec, §0 item 6, §8; product §5.9).
//!
//! `GlobalEvents` turns every left press and wheel step into
//! [`Msg::Pointer`] (only when `mouse = true`). [`App::update`] routes it in
//! one step, [`App::pointer`]:
//!
//! - **Hit testing** (§8.1). [`hit_test`] (in `layout.rs`, beside `layout`)
//!   says where the event goes, over the routing layout: the pure [`layout`]
//!   function that `App::view` uses too, cached here with its input and
//!   refreshed whenever the screen, an overlay or the size changes, never
//!   from `view`. It is the one place z-order is defined, and it blocks
//!   structurally: an open overlay or prompt blocks what is under it, and a
//!   component that does not take the pointer (the toast, a help line, a
//!   panel, the selector's frame) blocks what is under it.
//! - **The press-elsewhere notice** (§8.3). The router remembers which
//!   component got the last left press, and nothing else. When a later left
//!   press goes anywhere else (another component, outside an overlay,
//!   nothing at all), that component, if still mounted, first gets
//!   [`UserEvent::PointerElsewhere`] through its `on()`, so a double-click
//!   under way there ends. The wheel sends no notice.
//! - **Delivery** (§8.2). The component found gets a [`RoutedMouse`] (the
//!   event with the position relative to its area) as
//!   [`UserEvent::Pointer`]. The overlay on top gets a left press outside it
//!   as [`UserEvent::PointerOutside`], and closes itself as on `Esc` unless
//!   it is modal. The component's answer is queued with `emit`, never
//!   handled by calling `update` again: [`App::handle`] handles it before any
//!   later event.
//!
//! The component alone decides what an event means: selection, folding,
//! focus, closing. Double-clicks are the components' too: `ProjectList`,
//! `SessionList`, `ThemeSelector` and `TurnSidebar` each keep their own last
//! press, measured with the clock this module hands them when they are
//! mounted ([`App::set_click_clock`]), and answer a press on a row with an
//! intent that says whether it completed a double-click
//! (`SelectorMsg::ChooseSession { id, open }` and the like). The handlers of
//! those intents live with their concern; the `App` interprets no click and
//! keeps no click state.
//!
//! Why not hand the component the event itself? The application forwards
//! every raw `Event::Mouse` to the focused component, in screen coordinates,
//! and a component cannot tell those from routed ones.

use std::time::Instant;

use ratatui::layout::{Rect, Size};
use tuirealm::event::{Event, MouseButton, MouseEvent, MouseEventKind};

use super::layout::{LayoutInput, PointerTarget, ScreenLayout, hit_test, layout};
use super::{App, Id, UserEvent};
use crate::components::{CLICK_CLOCK_ATTR, ClickClock};

/// The components that keep double-clicks, and so the click clock.
const CLICK_CLOCK_IDS: [Id; 4] = [
    Id::ProjectList,
    Id::SessionList,
    Id::ThemeSelector,
    Id::TurnSidebar,
];

/// A mouse event for the component under the pointer: the position is
/// relative to the component's area (its top-left cell is 0, 0), and `area`
/// is that area's size. Built by the router; no Port delivers it.
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

/// What the `App` keeps for the pointer: the layout it routes with and the
/// input it was laid out from, the component that got the last left press,
/// and the clock the components measure double-clicks with.
#[derive(Default)]
pub(super) struct PointerState {
    /// The areas events are routed with.
    pub(super) routing: ScreenLayout,
    /// What `routing` was laid out from.
    routing_input: LayoutInput,
    /// The component that got the last left press, if one did.
    pressed: Option<Id>,
    click_clock: ClickClock,
}

impl PointerState {
    /// The clock the components measure double-clicks with, for one that is
    /// being mounted.
    pub(super) fn click_clock(&self) -> &ClickClock {
        &self.click_clock
    }
}

impl App {
    /// The areas mouse events are routed with.
    pub fn routing(&self) -> &ScreenLayout {
        &self.mouse.routing
    }

    /// Replaces the clock double-clicks are measured with (tests inject a
    /// fixed one): the components that keep double-clicks get it when they
    /// are mounted, and those mounted now get it at once.
    pub fn set_click_clock(&mut self, clock: impl Fn() -> Instant + Send + Sync + 'static) {
        self.mouse.click_clock = ClickClock::new(clock);
        let value = self.mouse.click_clock.value();
        for id in CLICK_CLOCK_IDS {
            if self.application.mounted(&id) {
                self.set_attr(id, CLICK_CLOCK_ATTR, value.clone());
            }
        }
    }

    /// Recomputes the routing areas from the current state. `update` calls
    /// it after every message (any of them can open an overlay or a prompt,
    /// or change the screen), `push_viewports` when the screen or the size
    /// changes, and `show_toast_for` when a toast appears; `view` never
    /// calls it. It is a handful of rectangles, so it is not worth finding
    /// out which messages can change the layout.
    pub(super) fn refresh_routing(&mut self) {
        let area = Rect::new(0, 0, self.size.0, self.size.1);
        let input = self.layout_input();
        self.mouse.routing = layout(&input, area);
        self.mouse.routing_input = input;
    }

    /// A left press or a wheel step at a position of the screen: gives it to
    /// the component [`hit_test`] finds, wherever the focus is, and queues
    /// its answer. A left press first tells the component that got the last
    /// one, if it goes elsewhere.
    pub(super) fn pointer(&mut self, event: MouseEvent) {
        if !self.config.mouse {
            return;
        }
        let target = match hit_test(&self.mouse.routing, &self.mouse.routing_input, event) {
            // The layout places "the transcript"; the one on top gets it.
            PointerTarget::Component(Id::Transcript, area) => Some((
                self.top_transcript(),
                UserEvent::Pointer(RoutedMouse::new(event, area)),
            )),
            PointerTarget::Component(id, area) => {
                Some((id, UserEvent::Pointer(RoutedMouse::new(event, area))))
            }
            PointerTarget::Outside(overlay, _) => Some((overlay, UserEvent::PointerOutside)),
            PointerTarget::Nothing => None,
        };
        if event.kind == MouseEventKind::Down(MouseButton::Left) {
            // Only a press a component gets as its own can be on its row.
            let pressed = match &target {
                Some((id, UserEvent::Pointer(_))) => Some(*id),
                _ => None,
            };
            self.press_elsewhere(pressed);
        }
        let Some((id, routed)) = target else {
            return;
        };
        let answer = self
            .application
            .get_component_mut(&id)
            .and_then(|component| component.on(&Event::User(routed)));
        if let Some(msg) = answer {
            self.emit(msg);
        }
    }

    /// A left press that the component `pressed` gets as its own (`None`: a
    /// press outside an overlay, or one that reaches no component): the
    /// component that got the last one is told first, if it is another one
    /// and still mounted. Remembers `pressed` for the next press.
    fn press_elsewhere(&mut self, pressed: Option<Id>) {
        let last = std::mem::replace(&mut self.mouse.pressed, pressed);
        let Some(last) = last.filter(|&last| Some(last) != pressed) else {
            return;
        };
        // `None` when it is no longer mounted. A component answers the
        // notice with nothing.
        if let Some(component) = self.application.get_component_mut(&last) {
            component.on(&Event::User(UserEvent::PointerElsewhere));
        }
    }
}
