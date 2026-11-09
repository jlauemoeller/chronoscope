//! The overlay concern (architecture.md#application-structure): the
//! messages of the help and details overlays, and the dispatcher that hands
//! them to the handlers in [`help`](super::help) and
//! [`details`](super::details).

use super::App;
use super::redraw::RedrawRequest;
use crate::components::{DetailsRequest, HelpContext};

/// What happened to the help or details overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayMsg {
    /// The help key: open the keybindings overlay for this screen.
    OpenHelp(HelpContext),
    /// `Esc` or the help key in the overlay: close it and return the focus.
    CloseHelp,
    /// A key scrolled the help overlay: nothing but a redraw is needed.
    HelpScrolled,
    /// The details key on a section: open the details overlay with what the
    /// model knows of it, a snapshot made when the key was pressed, and the
    /// section's references, which the harness reads when the overlay opens.
    OpenDetails(DetailsRequest),
    /// `Esc` or the details key in the details overlay: close it and return
    /// the focus.
    CloseDetails,
    /// A key or the wheel scrolled the details overlay: nothing but a
    /// redraw is needed.
    DetailsScrolled,
}

impl App {
    /// Hands an overlay message to its handler.
    pub(super) fn overlay_msg(&mut self, msg: OverlayMsg) {
        match msg {
            OverlayMsg::OpenHelp(context) => self.open_help(context),
            OverlayMsg::CloseHelp => self.close_help(),
            OverlayMsg::OpenDetails(request) => self.open_details(request),
            OverlayMsg::CloseDetails => self.close_details(),
            OverlayMsg::HelpScrolled | OverlayMsg::DetailsScrolled => {
                self.request_redraw(RedrawRequest::Dirty);
            }
        }
    }
}
