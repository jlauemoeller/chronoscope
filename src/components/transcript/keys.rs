//! The browser actions the transcript handles, as `Cmd`s.

use tuirealm::command::{Cmd, CmdResult, Direction, Position};

use super::Transcript;
use super::folding::FoldOp;
use super::nav::Step;
use super::paging::Amount;
use super::search::{CLEAR_SEARCH, SEARCH_NEXT, SEARCH_PREV};
use crate::keymap::Action;

/// `Cmd::Custom` name: jump to the next turn block.
pub const NEXT_TURN: &str = "next_turn";
/// `Cmd::Custom` name: jump to the previous turn block.
pub const PREV_TURN: &str = "prev_turn";
/// `Cmd::Custom` name: scroll half a page down.
pub const HALF_PAGE_DOWN: &str = "half_page_down";
/// `Cmd::Custom` name: scroll half a page up.
pub const HALF_PAGE_UP: &str = "half_page_up";
/// `Cmd::Custom` name: scroll a page down.
pub const PAGE_DOWN: &str = "page_down";
/// `Cmd::Custom` name: scroll a page up.
pub const PAGE_UP: &str = "page_up";
/// `Cmd::Custom` name: fold the selected section.
pub const FOLD: &str = "fold";
/// `Cmd::Custom` name: unfold the selected section.
pub const UNFOLD: &str = "unfold";
/// `Cmd::Custom` name: fold every section.
pub const FOLD_ALL: &str = "fold_all";
/// `Cmd::Custom` name: unfold every section.
pub const UNFOLD_ALL: &str = "unfold_all";
/// `Cmd::Custom` name: show or hide system sections.
pub const TOGGLE_SYSTEM: &str = "toggle_system";

/// The `Cmd` that performs `action`, for the actions this component owns.
pub(super) fn cmd_for(action: Action) -> Option<Cmd> {
    Some(match action {
        Action::Next => Cmd::Move(Direction::Down),
        Action::Prev => Cmd::Move(Direction::Up),
        Action::NextTurn => Cmd::Custom(NEXT_TURN),
        Action::PrevTurn => Cmd::Custom(PREV_TURN),
        Action::HalfPageDown => Cmd::Custom(HALF_PAGE_DOWN),
        Action::HalfPageUp => Cmd::Custom(HALF_PAGE_UP),
        Action::PageDown => Cmd::Custom(PAGE_DOWN),
        Action::PageUp => Cmd::Custom(PAGE_UP),
        Action::Top => Cmd::GoTo(Position::Begin),
        Action::Bottom => Cmd::GoTo(Position::End),
        Action::FoldToggle => Cmd::Submit,
        Action::Fold => Cmd::Custom(FOLD),
        Action::Unfold => Cmd::Custom(UNFOLD),
        Action::FoldAll => Cmd::Custom(FOLD_ALL),
        Action::UnfoldAll => Cmd::Custom(UNFOLD_ALL),
        Action::ToggleSystem => Cmd::Custom(TOGGLE_SYSTEM),
        Action::SearchNext => Cmd::Custom(SEARCH_NEXT),
        Action::SearchPrev => Cmd::Custom(SEARCH_PREV),
        Action::ClearSearch => Cmd::Custom(CLEAR_SEARCH),
        _ => return None,
    })
}

impl Transcript {
    /// Performs a `Cmd::Custom`; unknown names are invalid.
    pub(super) fn custom(&mut self, name: &'static str) -> CmdResult {
        match name {
            NEXT_TURN => self.jump_turn(Step::Next),
            PREV_TURN => self.jump_turn(Step::Previous),
            HALF_PAGE_DOWN => self.scroll_pages(Step::Next, Amount::Half),
            HALF_PAGE_UP => self.scroll_pages(Step::Previous, Amount::Half),
            PAGE_DOWN => self.scroll_pages(Step::Next, Amount::Full),
            PAGE_UP => self.scroll_pages(Step::Previous, Amount::Full),
            FOLD => self.fold_command(FoldOp::Fold),
            UNFOLD => self.fold_command(FoldOp::Unfold),
            FOLD_ALL => self.set_all_folded(true),
            UNFOLD_ALL => self.set_all_folded(false),
            TOGGLE_SYSTEM => self.toggle_system(),
            SEARCH_NEXT => self.step_search(Step::Next),
            SEARCH_PREV => self.step_search(Step::Previous),
            CLEAR_SEARCH => self.clear_search(),
            _ => CmdResult::Invalid(Cmd::Custom(name)),
        }
    }
}
