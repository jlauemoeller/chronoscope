//! The details overlay in the app (SPEC §1.4, §5): opening it gives it the
//! focus, closing it returns the focus to who had it. The content is made by
//! the transcript when the key is pressed and never changes while the overlay
//! is open.

use ratatui::layout::Rect;
use tuirealm::component::AppComponent;

use super::layout::{layout, overlay_max_width, too_small};
use super::redraw::RedrawRequest;
use super::{App, Id, Msg, UserEvent};
use crate::components::{
    DESIRED_SIZE_ATTR, DetailsContent, DetailsOverlay, INTERIOR_ATTR, MAX_WIDTH_ATTR, interior_of,
    interior_value, max_width_value, payload_from_value,
};

impl App {
    /// Mounts the details overlay for `content` and gives it the focus. Does
    /// nothing when it, the help overlay, the theme selector or a prompt is
    /// open, or when the screen is too small to show it.
    pub(super) fn open_details(&mut self, content: DetailsContent) {
        if too_small(Rect::new(0, 0, self.size.0, self.size.1)) {
            return;
        }
        let busy = [
            Id::DetailsOverlay,
            Id::HelpOverlay,
            Id::ThemeSelector,
            Id::Prompt,
        ];
        if busy.iter().any(|id| self.application.mounted(id)) {
            return;
        }
        let overlay = DetailsOverlay::new(content, self.theme.clone(), self.keymap.clone());
        // Wrapped at the width the screen leaves it, the content is as high
        // as it will be drawn.
        let wanted = overlay.desired_size(overlay_max_width(self.size.0));
        let component: Box<dyn AppComponent<Msg, UserEvent>> = Box::new(overlay);
        if let Err(err) = self.open_overlay(Id::DetailsOverlay, component) {
            log::error!("could not mount the details overlay: {err}");
            return;
        }
        self.details_size = Some(wanted);
        self.refresh_details_size();
        self.refresh_routing();
        self.push_details_interior();
        self.request_redraw(RedrawRequest::Dirty);
    }

    /// Unmounts the details overlay and returns the focus to who had it.
    pub(super) fn close_details(&mut self) {
        if !self.application.mounted(&Id::DetailsOverlay) {
            return;
        }
        self.details_size = None;
        self.close_overlay(Id::DetailsOverlay);
    }

    /// Unmounts the details overlay without returning the focus (the screen
    /// under it is being replaced).
    pub(super) fn drop_details(&mut self) {
        self.details_size = None;
        if self.application.mounted(&Id::DetailsOverlay)
            && let Err(err) = self.application.umount(&Id::DetailsOverlay)
        {
            log::warn!("could not unmount the details overlay: {err}");
        }
    }

    /// Asks the details overlay for the size it wants at the current screen
    /// width: as wide as its widest row, but at most the screen less the
    /// margin, and as high as its content wrapped at that width. Called when
    /// the size changes, before the routing layout is refreshed, so that the
    /// overlay follows the screen it is on.
    pub(super) fn refresh_details_size(&mut self) {
        if self.details_size.is_none() {
            return;
        }
        self.set_attr(
            Id::DetailsOverlay,
            MAX_WIDTH_ATTR,
            max_width_value(overlay_max_width(self.size.0)),
        );
        let wanted = self
            .application
            .query(&Id::DetailsOverlay, DESIRED_SIZE_ATTR)
            .ok()
            .flatten()
            .and_then(|value| payload_from_value(&value.into_attr()));
        if wanted.is_some() {
            self.details_size = wanted;
        }
    }

    /// Tells the details overlay how many rows and columns its interior has
    /// at the current size.
    pub(super) fn push_details_interior(&mut self) {
        if self.details_size.is_none() {
            return;
        }
        let area = Rect::new(0, 0, self.size.0, self.size.1);
        let Some(overlay) = layout(&self.layout_input(), area).area(Id::DetailsOverlay) else {
            return;
        };
        self.set_attr(
            Id::DetailsOverlay,
            INTERIOR_ATTR,
            interior_value(interior_of(overlay)),
        );
    }
}
