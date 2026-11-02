//! The details overlay in the app (SPEC §1.4, §5): opening it gives it the
//! focus, closing it returns the focus to who had it. The content is made by
//! the transcript when the key is pressed and never changes while the overlay
//! is open.

use ratatui::layout::Rect;
use tuirealm::component::AppComponent;

use super::layout::layout;
use super::redraw::RedrawRequest;
use super::{App, Id, Msg, UserEvent};
use crate::components::{
    DetailsContent, DetailsOverlay, INTERIOR_ATTR, interior_of, interior_value,
};

/// The columns of the screen the overlay leaves free: two either side (see
/// `help_overlay_rect`).
const SCREEN_MARGIN: u16 = 4;

impl App {
    /// Mounts the details overlay for `content` and gives it the focus. Does
    /// nothing when it, the help overlay, the theme selector or a prompt is
    /// open.
    pub(super) fn open_details(&mut self, content: DetailsContent) {
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
        let wanted = overlay.desired_size(self.size.0.saturating_sub(SCREEN_MARGIN));
        let component: Box<dyn AppComponent<Msg, UserEvent>> = Box::new(overlay);
        if let Err(err) = self.open_overlay(Id::DetailsOverlay, component) {
            log::error!("could not mount the details overlay: {err}");
            return;
        }
        self.details_size = Some(wanted);
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
