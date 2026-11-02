//! The frame and the scrolling that the help overlay and the details overlay
//! share (SPEC §4.3): the rounded `ring` border with a title, the `surface`
//! fill, the ` ↑ more ` and ` ↓ more ` indicators, the scroll offset, and the
//! overlay-context keys and the wheel that move it.
//!
//! The frame does not know the content. The overlay that owns it passes the
//! number of rows the content takes whenever that matters, because it can
//! depend on the width (the details overlay wraps long values).

use ratatui::Frame;
use ratatui::layout::{Rect, Size};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use tuirealm::event::{KeyEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute};

use super::list_pane::titled_block;
use super::payload_value;
use crate::app::RoutedMouse;
use crate::keymap::{Action, Context, Keymap};
use crate::theme::{Role, Theme};

/// The attribute that announces the size of the overlay's interior (a [`Size`]
/// payload, see [`interior_value`]): its rows decide how far a page scrolls
/// and how far the content can scroll.
pub const INTERIOR_ATTR: Attribute = Attribute::Custom("overlay_interior");

/// The attribute value that announces the interior size.
pub fn interior_value(size: Size) -> AttrValue {
    payload_value(size)
}

/// How many rows one step of the mouse wheel scrolls.
pub const WHEEL_ROWS: usize = 3;

/// The text of the indicator on the top border when rows are hidden above.
const MORE_ABOVE: &str = " ↑ more ";
/// The text of the indicator on the bottom border when rows are hidden below.
const MORE_BELOW: &str = " ↓ more ";

/// What an overlay's keys did, as far as the frame can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKey {
    /// The `close` key of the overlay context.
    Close,
    /// A scroll key moved the view.
    Scrolled,
    /// An overlay-context key that did not move anything (a scroll key at
    /// the end of the content, or an action that does not scroll).
    Unmoved,
    /// A key that is not bound in the overlay context. The overlay may
    /// still have a key of its own for it.
    Unbound,
}

/// The border, the fill and the scroll state of a scrolling overlay.
#[derive(Debug, Clone)]
pub struct OverlayFrame {
    title: &'static str,
    theme: Theme,
    keymap: Keymap,
    /// The first row shown.
    offset: usize,
    /// The rows of the interior; unknown until announced, when nothing
    /// scrolls.
    interior: Option<Size>,
}

impl OverlayFrame {
    /// A frame titled `title`, resolving keys with `keymap`.
    pub fn new(title: &'static str, theme: Theme, keymap: Keymap) -> Self {
        Self {
            title,
            theme,
            keymap,
            offset: 0,
            interior: None,
        }
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    /// The first row shown.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// The interior announced with [`set_interior`](Self::set_interior).
    pub fn interior(&self) -> Option<Size> {
        self.interior
    }

    /// Announces the interior size. `total` is the number of rows the
    /// content takes at that width. A taller overlay can show rows that were
    /// out of reach, so the offset is pulled back into range.
    pub fn set_interior(&mut self, size: Size, total: usize) {
        self.interior = Some(size);
        self.scroll_to(self.offset, total);
    }

    /// The last row that can be the first shown with `rows` rows visible.
    fn max_offset(rows: usize, total: usize) -> usize {
        total.saturating_sub(rows)
    }

    /// Scrolls to `offset`, kept in range. Returns whether the view moved.
    pub fn scroll_to(&mut self, offset: usize, total: usize) -> bool {
        let rows = self.interior.map_or(usize::MAX, |s| usize::from(s.height));
        let offset = offset.min(Self::max_offset(rows, total));
        let moved = offset != self.offset;
        self.offset = offset;
        moved
    }

    fn scroll_by(&mut self, rows: usize, down: bool, total: usize) -> bool {
        let target = if down {
            self.offset.saturating_add(rows)
        } else {
            self.offset.saturating_sub(rows)
        };
        self.scroll_to(target, total)
    }

    /// A page: the rows of the interior.
    fn page(&self) -> usize {
        self.interior.map_or(1, |s| usize::from(s.height)).max(1)
    }

    /// Resolves `key` in the overlay context: `close` closes, the up and
    /// down keys scroll a row and the page keys a page. `total` is the
    /// number of rows of the content.
    pub fn on_key(&mut self, key: &KeyEvent, total: usize) -> FrameKey {
        let Some(action) = self.keymap.resolve(Context::Overlay, key) else {
            return FrameKey::Unbound;
        };
        let moved = match action {
            Action::Close => return FrameKey::Close,
            Action::Up => self.scroll_by(1, false, total),
            Action::Down => self.scroll_by(1, true, total),
            Action::PageUp => self.scroll_by(self.page(), false, total),
            Action::PageDown => self.scroll_by(self.page(), true, total),
            _ => false,
        };
        if moved {
            FrameKey::Scrolled
        } else {
            FrameKey::Unmoved
        }
    }

    /// The wheel scrolls [`WHEEL_ROWS`] rows; nothing else does anything
    /// (the `App` closes the overlay for a click outside it). Returns
    /// whether the view moved.
    pub fn on_mouse(&mut self, mouse: &RoutedMouse, total: usize) -> bool {
        match mouse.kind {
            MouseEventKind::ScrollUp => self.scroll_by(WHEEL_ROWS, false, total),
            MouseEventKind::ScrollDown => self.scroll_by(WHEEL_ROWS, true, total),
            _ => false,
        }
    }

    /// The border block, with the indicators for the rows hidden above and
    /// below.
    fn block(&self, above: bool, below: bool) -> Block<'static> {
        let mut block = titled_block(&self.theme, self.title, Role::Ring);
        if !self.theme.transparent_background() {
            block = block.style(Style::new().bg(self.theme.role(Role::Surface)));
        }
        if above {
            block = block.title(self.indicator(MORE_ABOVE));
        }
        if below {
            block = block.title_bottom(self.indicator(MORE_BELOW));
        }
        block
    }

    /// `text` in `muted_foreground`, right-aligned with one `─` before the
    /// corner.
    fn indicator(&self, text: &'static str) -> Line<'static> {
        let muted = Style::new().fg(self.theme.role(Role::MutedForeground));
        let ring = Style::new().fg(self.theme.role(Role::Ring));
        Line::from(vec![Span::styled(text, muted), Span::styled("─", ring)]).right_aligned()
    }

    /// Draws the overlay in `area` (cut to the frame): clears it, fills it
    /// with `surface` (only clears with a transparent theme), draws the
    /// border and the rows from the offset on. `rows` makes the rows of the
    /// content for an interior of the given width; it is called once.
    pub fn draw(
        &self,
        frame: &mut Frame,
        area: Rect,
        rows: impl FnOnce(u16) -> Vec<Line<'static>>,
    ) {
        let area = area.intersection(frame.area());
        frame.render_widget(Clear, area);
        let interior_rows = usize::from(area.height.saturating_sub(2));
        let lines = rows(area.width.saturating_sub(2));
        let offset = self
            .offset
            .min(Self::max_offset(interior_rows, lines.len()));
        let above = offset > 0;
        let below = offset + interior_rows < lines.len();
        let block = self.block(above, below);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let lines: Vec<Line> = lines.into_iter().skip(offset).take(interior_rows).collect();
        frame.render_widget(Paragraph::new(lines), inner);
    }
}

/// The interior of an overlay drawn in `overlay`: the border takes one cell
/// each side.
pub fn interior_of(overlay: Rect) -> Size {
    Size::new(
        overlay.width.saturating_sub(2),
        overlay.height.saturating_sub(2),
    )
}
