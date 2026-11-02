//! The help overlay in the app (SPEC §1.4, §4.3): opening it gives it the
//! focus, closing it returns the focus to who had it.

use ratatui::layout::Rect;
use tuirealm::component::AppComponent;

use super::layout::{Screen, layout};
use super::redraw::RedrawRequest;
use super::{App, Id, Msg, UserEvent};
use crate::components::{
    HELP_CONTEXT_ATTR, HelpContent, HelpContext, HelpOverlay, INTERIOR_ATTR, context_value,
    interior_value,
};

impl App {
    /// The context the help key opens the overlay for on the current screen.
    fn help_context(screen: Screen) -> HelpContext {
        if screen.has_selector() {
            HelpContext::Selector
        } else {
            HelpContext::Browser
        }
    }

    /// Switches to `screen` and tells the global keys which help the `?` key
    /// opens there.
    pub(super) fn set_screen(&mut self, screen: Screen) {
        self.screen = screen;
        self.set_attr(
            Id::GlobalEvents,
            HELP_CONTEXT_ATTR,
            context_value(Self::help_context(screen)),
        );
    }

    /// Mounts the help overlay for `context` and gives it the focus. Does
    /// nothing when it is open already.
    pub(super) fn open_help(&mut self, context: HelpContext) {
        if self.application.mounted(&Id::HelpOverlay)
            || self.application.mounted(&Id::DetailsOverlay)
        {
            return;
        }
        let content = HelpContent::new(
            &self.keymap,
            context,
            self.selector.registered(),
            self.config.mouse,
        );
        let wanted = content.desired_size();
        let overlay = HelpOverlay::new(content, self.theme.clone(), self.keymap.clone());
        let component: Box<dyn AppComponent<Msg, UserEvent>> = Box::new(overlay);
        if let Err(err) = self.open_overlay(Id::HelpOverlay, component) {
            log::error!("could not mount the help overlay: {err}");
            return;
        }
        self.help_size = Some(wanted);
        self.push_help_interior();
        self.request_redraw(RedrawRequest::Dirty);
    }

    /// Unmounts the help overlay and returns the focus to who had it.
    pub(super) fn close_help(&mut self) {
        if !self.application.mounted(&Id::HelpOverlay) {
            return;
        }
        self.help_size = None;
        self.close_overlay(Id::HelpOverlay);
    }

    /// Unmounts the help overlay without returning the focus (the screen
    /// under it is being replaced).
    pub(super) fn drop_help(&mut self) {
        self.help_size = None;
        if self.application.mounted(&Id::HelpOverlay)
            && let Err(err) = self.application.umount(&Id::HelpOverlay)
        {
            log::warn!("could not unmount the help overlay: {err}");
        }
    }

    /// Tells the help overlay how many rows its interior has at the current
    /// size.
    pub(super) fn push_help_interior(&mut self) {
        if self.help_size.is_none() {
            return;
        }
        let area = Rect::new(0, 0, self.size.0, self.size.1);
        let Some(overlay) = layout(&self.layout_input(), area).area(Id::HelpOverlay) else {
            return;
        };
        let interior = ratatui::layout::Size::new(
            overlay.width.saturating_sub(2),
            overlay.height.saturating_sub(2),
        );
        self.set_attr(Id::HelpOverlay, INTERIOR_ATTR, interior_value(interior));
    }
}
