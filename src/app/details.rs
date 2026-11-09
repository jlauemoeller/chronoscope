//! The details overlay in the app (architecture.md#events-focus-and-keys,
//! product §4): opening it gives it the focus, closing it returns the focus
//! to who had it. The content is made by the transcript when the key is
//! pressed and never changes while the overlay is open. It handles the
//! details overlay's part of the overlay concern
//! ([`OverlayMsg`](super::OverlayMsg)), which `overlay.rs` dispatches.

use ratatui::layout::{Rect, Size};
use tuirealm::component::AppComponent;

use super::layout::{layout, overlay_max_width, too_small};
use super::redraw::RedrawRequest;
use super::{App, Id, Msg, UserEvent};
use crate::components::{
    DetailsContent, DetailsOverlay, DetailsRequest, INTERIOR_ATTR, compose, interior_of,
    interior_value,
};
use crate::model::SourceGroup;

/// What the app keeps while the details overlay is open: the content it
/// shows, which never changes while it is open, and the size the overlay
/// wants at the current screen width.
#[derive(Debug, Clone)]
pub(super) struct DetailsSizing {
    content: DetailsContent,
    /// The size the overlay wants, borders included.
    pub size: Size,
}

impl DetailsSizing {
    /// The sizing of `content` on a screen `screen_width` columns wide.
    fn new(content: DetailsContent, screen_width: u16) -> Self {
        let size = Self::size_of(&content, screen_width);
        Self { content, size }
    }

    /// As wide as the content's widest row, but at most the screen less the
    /// margin, and as high as the content wrapped at that width.
    fn size_of(content: &DetailsContent, screen_width: u16) -> Size {
        content.desired_size(overlay_max_width(screen_width))
    }

    /// Recomputes the size for a screen `screen_width` columns wide.
    fn resize(&mut self, screen_width: u16) {
        self.size = Self::size_of(&self.content, screen_width);
    }
}

impl App {
    /// Mounts the details overlay for `request` and gives it the focus. Does
    /// nothing when it, the help overlay, the theme selector or a prompt is
    /// open, or when the screen is too small to show it.
    ///
    /// The open session's harness reads the section's source here, on the UI
    /// thread, and its rows are composed with the ones the model knows. A
    /// section without references, or a harness that provides nothing, leaves
    /// the model's rows alone.
    pub(super) fn open_details(&mut self, request: DetailsRequest) {
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
        let content = self.compose_details(request);
        // Wrapped at the width the screen leaves it, the content is as high
        // as it will be drawn.
        let sizing = DetailsSizing::new(content.clone(), self.size.0);
        let overlay = DetailsOverlay::new(content, self.theme.clone(), self.keymap.clone());
        let component: Box<dyn AppComponent<Msg, UserEvent>> = Box::new(overlay);
        if let Err(err) = self.open_overlay(Id::DetailsOverlay, component) {
            log::error!("could not mount the details overlay: {err}");
            return;
        }
        self.details = Some(sizing);
        self.refresh_routing();
        self.push_details_interior();
        self.request_redraw(RedrawRequest::Dirty);
    }

    /// The content of the overlay: what the model knows of the section, with
    /// what the open session's harness reads from its source.
    fn compose_details(&self, request: DetailsRequest) -> DetailsContent {
        let read: Vec<SourceGroup> = request
            .source
            .as_ref()
            .zip(self.browser.open.as_ref())
            .and_then(|(source, open)| Some((source, self.harnesses.get(&open.harness)?)))
            .map(|(source, harness)| harness.section_details(source))
            .unwrap_or_default();
        compose(request.content, &read, &self.timestamps)
    }

    /// Unmounts the details overlay and returns the focus to who had it.
    pub(super) fn close_details(&mut self) {
        if !self.application.mounted(&Id::DetailsOverlay) {
            return;
        }
        self.details = None;
        self.close_overlay(Id::DetailsOverlay);
    }

    /// Unmounts the details overlay without returning the focus (the screen
    /// under it is being replaced).
    pub(super) fn drop_details(&mut self) {
        self.details = None;
        if self.application.mounted(&Id::DetailsOverlay)
            && let Err(err) = self.application.umount(&Id::DetailsOverlay)
        {
            log::warn!("could not unmount the details overlay: {err}");
        }
    }

    /// Recomputes the size the details overlay wants from its content at
    /// the current screen width (see [`DetailsSizing`]). Called when the size
    /// changes, before the routing layout is refreshed, so that the overlay
    /// follows the screen it is on.
    pub(super) fn refresh_details_size(&mut self) {
        let width = self.size.0;
        if let Some(details) = &mut self.details {
            details.resize(width);
        }
    }

    /// Tells the details overlay how many rows and columns its interior has
    /// at the current size.
    pub(super) fn push_details_interior(&mut self) {
        if self.details.is_none() {
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
