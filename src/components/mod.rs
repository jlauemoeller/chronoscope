//! The UI components (SPEC §1.3). Each is its own type and module, can be
//! built from plain data and the theme, and rendered into a `TestBackend`
//! frame without an `Application`.

mod empty_state;
mod global_events;
mod selector_frame;

use tuirealm::props::{AttrValue, Attribute, PropPayload};

use crate::theme::Theme;

pub use empty_state::{EmptyState, TOO_SMALL};
pub use global_events::{GlobalEvents, quit_key_events};
pub use selector_frame::SelectorFrame;

/// The attribute that carries the theme to a component.
pub const THEME_ATTR: Attribute = Attribute::Custom("theme");

/// The attribute value that sets a component's theme.
pub fn theme_value(theme: &Theme) -> AttrValue {
    AttrValue::Payload(PropPayload::Any(Box::new(theme.clone())))
}

/// The theme in an attribute value made by [`theme_value`].
pub fn theme_from_value(value: &AttrValue) -> Option<Theme> {
    match value {
        AttrValue::Payload(PropPayload::Any(any)) => any.as_any().downcast_ref::<Theme>().cloned(),
        _ => None,
    }
}
