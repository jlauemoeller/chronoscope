//! The theme selector in the app (product §7.4): opening it gives it the focus,
//! moving its cursor previews a theme on everything that is mounted, `Enter`
//! keeps the theme and saves it, `Esc` gives back the one that was active.
//! It handles the theme concern, [`ThemeMsg`].

use ratatui::layout::{Rect, Size};
use tuirealm::component::AppComponent;

use super::layout::layout;
use super::redraw::RedrawRequest;
use super::{App, Id, Msg, UserEvent};
use crate::components::{THEME_ATTR, ThemeSelector, VISIBLE_ROWS_ATTR, theme_items, theme_value};
use crate::config::{SaveThemeError, save_theme};
use crate::theme::{Theme, resolve_by_id};
use tuirealm::props::AttrValue;

/// Something happened in the theme selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeMsg {
    /// The theme key: open the theme selector.
    Open,
    /// The cursor of the theme selector moved to the theme with this id:
    /// show it as a preview.
    Preview(String),
    /// `Enter` in the theme selector on the theme with this id: keep it,
    /// save it to the config and close the selector.
    Select(String),
    /// A left press on the theme with this id in the theme selector (its
    /// cursor is there now): show it as a preview, or, with `keep` (the
    /// press completed a double-click), keep it as `Enter` does.
    Choose { id: String, keep: bool },
    /// `Esc` in the theme selector: restore the theme that was active when
    /// it opened and close it.
    Cancel,
}

/// What the app remembers while the theme selector is open.
#[derive(Debug, Clone)]
pub(super) struct ThemeSelection {
    /// The size the overlay wants, borders included.
    pub size: Size,
    /// The id of the theme that was active when it opened.
    original_id: String,
    /// That theme, exactly as it was.
    original: Theme,
}

/// The components that draw with the theme (every one but `GlobalEvents`).
/// The transcripts of subagents are added for the depths that are open.
pub(super) const THEMED: [Id; 21] = [
    Id::ScreenBackground,
    Id::SelectorFrame,
    Id::EmptyState,
    Id::HarnessTabs,
    Id::ProjectList,
    Id::SessionList,
    Id::SessionTime,
    Id::SessionTokens,
    Id::WorkingDir,
    Id::HelpLine,
    Id::BrowserHeader,
    Id::TurnSidebar,
    Id::SidebarGap,
    Id::Transcript,
    Id::BrowserHelpLine,
    Id::EmptySession,
    Id::Prompt,
    Id::HelpOverlay,
    Id::DetailsOverlay,
    Id::ThemeSelector,
    Id::Toast,
];

impl App {
    /// Hands a message of the theme selector to its handler.
    pub(super) fn theme_msg(&mut self, msg: ThemeMsg) {
        match msg {
            ThemeMsg::Open => self.open_theme_selector(),
            ThemeMsg::Preview(id) => self.preview_theme(&id),
            ThemeMsg::Select(id) => self.select_theme(&id),
            ThemeMsg::Choose { id, keep } => self.choose_theme(&id, keep),
            ThemeMsg::Cancel => self.cancel_theme(),
        }
    }

    /// Mounts the theme selector with the cursor on the active theme and
    /// gives it the focus, remembering the active theme for `Esc`. Does
    /// nothing when it, the help overlay, the details overlay or a prompt is
    /// open.
    pub(super) fn open_theme_selector(&mut self) {
        let busy = [
            Id::ThemeSelector,
            Id::HelpOverlay,
            Id::DetailsOverlay,
            Id::Prompt,
        ];
        if busy.iter().any(|id| self.application.mounted(id)) {
            return;
        }
        let items = theme_items(&self.registry);
        let size = items.desired_size();
        let selector = ThemeSelector::new(
            items,
            &self.theme_id,
            self.theme.clone(),
            self.keymap.clone(),
        )
        .with_click_clock(self.mouse.click_clock().clone());
        let component: Box<dyn AppComponent<Msg, UserEvent>> = Box::new(selector);
        if let Err(err) = self.open_overlay(Id::ThemeSelector, component) {
            log::error!("could not mount the theme selector: {err}");
            return;
        }
        self.theme_selector = Some(ThemeSelection {
            size,
            original_id: self.theme_id.clone(),
            original: self.theme.clone(),
        });
        self.push_theme_selector_rows();
        self.request_redraw(RedrawRequest::Dirty);
    }

    /// Shows the theme `id` on everything that is mounted. Ignored unless the
    /// selector is open (a key pressed after `Esc` in the same batch).
    pub(super) fn preview_theme(&mut self, id: &str) {
        if self.theme_selector.is_none() {
            return;
        }
        self.apply_theme_id(id);
    }

    /// A left press on the theme `id`, where the theme selector's cursor is
    /// now: it is previewed, or, with `keep` (a double-click), kept as on
    /// `Enter`.
    pub(super) fn choose_theme(&mut self, id: &str, keep: bool) {
        if keep {
            self.select_theme(id);
        } else {
            self.preview_theme(id);
        }
    }

    /// Keeps the theme `id`, closes the selector and saves the choice. The
    /// theme stays applied for this run whether or not it could be saved.
    pub(super) fn select_theme(&mut self, id: &str) {
        if self.theme_selector.take().is_none() {
            return;
        }
        if self.theme_id != id {
            self.apply_theme_id(id);
        }
        self.close_overlay(Id::ThemeSelector);
        self.config.theme.clone_from(&self.theme_id);
        self.save_chosen_theme();
    }

    /// Gives back the theme that was active when the selector opened and
    /// closes it.
    pub(super) fn cancel_theme(&mut self) {
        let Some(selection) = self.theme_selector.take() else {
            return;
        };
        self.apply_theme(&selection.original_id, selection.original);
        self.close_overlay(Id::ThemeSelector);
    }

    /// Closes the selector without returning the focus (the screen under it
    /// is being replaced), giving back the original theme.
    pub(super) fn drop_theme_selector(&mut self) {
        let Some(selection) = self.theme_selector.take() else {
            return;
        };
        self.apply_theme(&selection.original_id, selection.original);
        if self.application.mounted(&Id::ThemeSelector)
            && let Err(err) = self.application.umount(&Id::ThemeSelector)
        {
            log::warn!("could not unmount the theme selector: {err}");
        }
    }

    /// Tells the theme selector how many theme rows it shows at the current
    /// size.
    pub(super) fn push_theme_selector_rows(&mut self) {
        if self.theme_selector.is_none() {
            return;
        }
        let area = Rect::new(0, 0, self.size.0, self.size.1);
        let Some(overlay) = layout(&self.layout_input(), area).area(Id::ThemeSelector) else {
            return;
        };
        let rows = ThemeSelector::rows_in(overlay.height);
        self.set_attr(
            Id::ThemeSelector,
            VISIBLE_ROWS_ATTR,
            AttrValue::Length(rows),
        );
    }

    /// Resolves the theme `id` as at startup and makes it the active theme.
    pub(super) fn apply_theme_id(&mut self, id: &str) {
        match resolve_by_id(
            &self.registry,
            id,
            &self.theme_env,
            self.config.transparent_background,
        ) {
            Ok(theme) => self.apply_theme(id, theme),
            Err(err) => log::warn!("cannot show theme \"{id}\": {err}"),
        }
    }

    /// Makes `theme` the active theme (so components mounted later get it)
    /// and pushes it to every component that is mounted now.
    fn apply_theme(&mut self, id: &str, theme: Theme) {
        self.theme_id = id.to_owned();
        self.theme = theme;
        let value = theme_value(&self.theme);
        let subagents = (1..=self.browser.depth()).map(Id::Subagent);
        let targets: Vec<Id> = THEMED.into_iter().chain(subagents).collect();
        for target in targets {
            self.set_attr(target, THEME_ATTR, value.clone());
        }
        self.request_redraw(RedrawRequest::Dirty);
    }

    /// Writes the active theme's id to the config file; a toast says if that
    /// failed.
    fn save_chosen_theme(&mut self) {
        let result = match &self.config_path {
            Some(path) => save_theme(path, &self.theme_id),
            None => Err(SaveThemeError::Io(
                "there is no config file location".to_owned(),
            )),
        };
        if let Err(err) = result {
            log::warn!("could not save the theme: {err}");
            self.toast_quiet(format!("Could not save theme: {err}"));
        }
    }
}
