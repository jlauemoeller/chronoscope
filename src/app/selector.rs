//! The session selector's part of the application: the index data of every
//! harness, the selection, the focused pane, and the handlers that keep the
//! selector components in step with them (SPEC §1.2, §1.4, §4.1, §7.3).
//!
//! The `App` owns the data and the selection. The lists own their cursor and
//! scroll offset and report the user's moves as [`SelectorMsg`]s; when the
//! data changes the `App` works out the new selection (by id, falling back to
//! the same index) and pushes rows and selection into the components.

use std::collections::HashMap;
use std::sync::Arc;

use tuirealm::props::{AttrValue, Attribute};

use super::index_port::IndexUpdate;
use super::layout::{Screen, selector_areas};
use super::redraw::RedrawRequest;
use super::{App, Id};
use crate::components::{
    ACTIVE_ATTR, ActiveSessions, CWD_ATTR, PROJECT_ROWS_ATTR, ProjectRow, ProjectRows,
    SELECTED_ATTR, SESSION_ROWS_ATTR, SESSION_STATS_ATTR, SessionRow, SessionRows,
    SessionStatsUpdate, TABS_ATTR, Tab, Tabs, VISIBLE_ROWS_ATTR, payload_value, selected_value,
    visible_rows,
};
use crate::index::{IndexEvent, SessionMap};
use crate::model::{
    HarnessId, Project, ProjectId, SessionId, SessionStats, SessionSummary, is_active,
};

/// The two panes that take focus.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Pane {
    /// The Projects pane (focused at startup).
    #[default]
    Projects,
    /// The Sessions pane.
    Sessions,
}

/// What happened in the session selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectorMsg {
    /// The user moved the selection in Projects to this project.
    ProjectSelected(ProjectId),
    /// The user moved the selection in Sessions to this session.
    SessionSelected(SessionId),
    /// The user asked for focus on a pane.
    Focus(Pane),
    /// The user pressed the number of a harness.
    SwitchHarness(HarnessId),
}

/// What the last snapshot of a harness held.
#[derive(Debug, Clone)]
pub(super) struct HarnessData {
    available: bool,
    projects: Arc<Vec<Project>>,
    sessions: Arc<SessionMap>,
}

/// The selector's domain state.
#[derive(Debug, Default)]
pub(super) struct SelectorState {
    /// The harnesses with a tab, in tab order.
    registered: Vec<HarnessId>,
    /// The last snapshot of each harness that sent one.
    data: HashMap<HarnessId, HarnessData>,
    /// The stats received so far; they outlive snapshots.
    stats: HashMap<(HarnessId, SessionId), SessionStats>,
    /// The harness shown. `None` until one is available.
    current: Option<HarnessId>,
    project: Option<ProjectId>,
    session: Option<SessionId>,
    pane: Pane,
    /// The sessions of the selected project pushed as active last.
    active: Vec<SessionId>,
}

impl SelectorState {
    pub(super) fn new(registered: Vec<HarnessId>) -> Self {
        Self {
            registered,
            ..Self::default()
        }
    }

    fn current_data(&self) -> Option<&HarnessData> {
        self.data.get(&self.current?)
    }

    fn projects(&self) -> &[Project] {
        self.current_data().map_or(&[], |d| d.projects.as_slice())
    }

    /// The sessions of the selected project, newest first.
    fn sessions(&self) -> &[SessionSummary] {
        let sessions = self.current_data().zip(self.project.as_ref());
        sessions
            .and_then(|(data, project)| data.sessions.get(project))
            .map_or(&[], Vec::as_slice)
    }

    fn project_index(&self) -> Option<usize> {
        let id = self.project.as_ref()?;
        self.projects().iter().position(|p| &p.id == id)
    }

    fn session_index(&self) -> Option<usize> {
        let id = self.session.as_ref()?;
        self.sessions().iter().position(|s| &s.id == id)
    }

    fn is_available(&self, harness: HarnessId) -> bool {
        self.registered.contains(&harness) && self.data.get(&harness).is_some_and(|d| d.available)
    }

    /// The first available registered harness by number.
    fn first_available(&self) -> Option<HarnessId> {
        self.registered
            .iter()
            .copied()
            .find(|&h| self.is_available(h))
    }

    /// Selects the newest session if the Sessions pane has focus and no
    /// session is selected.
    fn select_newest_if_focused(&mut self) {
        if self.pane == Pane::Sessions && self.session.is_none() {
            self.session = self.sessions().first().map(|s| s.id.clone());
        }
    }
}

/// Where a selection goes when `old` is replaced by `new`: the same item
/// (by `same`) if it is still there, else the same index, clamped to the
/// last row. `None` when `new` is empty or `selected` was not in `old`.
fn reselect<T>(
    old: &[T],
    new: &[T],
    selected: usize,
    same: impl Fn(&T, &T) -> bool,
) -> Option<usize> {
    let last = new.len().checked_sub(1)?;
    let item = old.get(selected)?;
    new.iter()
        .position(|candidate| same(item, candidate))
        .or(Some(selected.min(last)))
}

impl App {
    // -- read access, for tests and later tasks

    /// The harness whose projects are shown, if any is available.
    pub fn selected_harness(&self) -> Option<HarnessId> {
        self.selector.current
    }

    /// The selected project.
    pub fn selected_project(&self) -> Option<&ProjectId> {
        self.selector.project.as_ref()
    }

    /// The selected session; `None` while no session is selected.
    pub fn selected_session(&self) -> Option<&SessionId> {
        self.selector.session.as_ref()
    }

    /// The pane that has focus.
    pub fn focused_pane(&self) -> Pane {
        self.selector.pane
    }

    /// The sessions of the selected project that show the `●` marker.
    pub fn active_sessions(&self) -> &[SessionId] {
        &self.selector.active
    }

    // -- worker results

    /// Applies what the index worker reported.
    pub(super) fn index_update(&mut self, update: &IndexUpdate) {
        match update.event() {
            IndexEvent::Snapshot {
                harness,
                available,
                projects,
                sessions,
            } => {
                let data = HarnessData {
                    available: *available,
                    projects: Arc::clone(projects),
                    sessions: Arc::clone(sessions),
                };
                self.snapshot(*harness, data);
            }
            IndexEvent::Stats {
                harness,
                session,
                stats,
            } => self.stats_ready(*harness, session, *stats),
        }
    }

    /// A harness's lists were rebuilt.
    fn snapshot(&mut self, harness: HarnessId, data: HarnessData) {
        if !self.selector.registered.contains(&harness) {
            log::debug!("ignoring an index snapshot of the unregistered harness {harness}");
            return;
        }
        let old = self.selector.data.insert(harness, data);
        if self.selector.current.is_none() {
            if let Some(first) = self.selector.first_available() {
                self.enter_harness(first);
                return;
            }
        } else if self.selector.current == Some(harness) {
            self.keep_selection(old.as_ref());
            return;
        }
        self.push_tabs();
    }

    /// The sessions of the current harness were replaced: keep the selected
    /// project and session by id, or the same index if one vanished.
    ///
    /// A project that vanished is a move to another project, so the session
    /// selection goes with it.
    fn keep_selection(&mut self, old: Option<&HarnessData>) {
        let new_projects = self.selector.projects().to_vec();
        let old_projects: &[Project] = old.map_or(&[], |d| d.projects.as_slice());
        let old_project = self
            .selector
            .project
            .as_ref()
            .and_then(|id| old_projects.iter().position(|p| &p.id == id));
        let project = match old_project {
            Some(i) => reselect(old_projects, &new_projects, i, |a, b| a.id == b.id),
            None => (!new_projects.is_empty()).then_some(0),
        };
        let project = project.map(|i| new_projects[i].id.clone());
        let old_sessions: &[SessionSummary] = old
            .zip(self.selector.project.as_ref())
            .and_then(|(d, id)| d.sessions.get(id))
            .map_or(&[], Vec::as_slice);
        let old_session = self
            .selector
            .session
            .as_ref()
            .and_then(|id| old_sessions.iter().position(|s| &s.id == id));
        let same_project = project == self.selector.project;
        self.selector.project = project;
        self.selector.session = None;
        if let (true, Some(i)) = (same_project, old_session) {
            let new_sessions = self.selector.sessions();
            let kept = reselect(old_sessions, new_sessions, i, |a, b| a.id == b.id);
            self.selector.session = kept.map(|i| new_sessions[i].id.clone());
        }
        self.selector.select_newest_if_focused();
        self.push_all();
    }

    /// Stores the stats and shows them if their row is on screen.
    fn stats_ready(&mut self, harness: HarnessId, session: &SessionId, stats: SessionStats) {
        self.selector
            .stats
            .insert((harness, session.clone()), stats);
        let shown = self.selector.current == Some(harness)
            && self.selector.sessions().iter().any(|s| &s.id == session);
        if shown {
            let update = SessionStatsUpdate {
                session: session.clone(),
                stats,
            };
            self.set_attr(Id::SessionList, SESSION_STATS_ATTR, payload_value(update));
        }
    }

    // -- user's moves

    /// Applies a message from the selector's components.
    pub(super) fn selector_msg(&mut self, msg: SelectorMsg) {
        match msg {
            SelectorMsg::ProjectSelected(id) => self.project_selected(id),
            SelectorMsg::SessionSelected(id) => self.session_selected(id),
            SelectorMsg::Focus(pane) => self.focus_pane(pane),
            SelectorMsg::SwitchHarness(harness) => self.switch_harness(harness),
        }
    }

    /// Another project was selected: its sessions are shown with none
    /// selected.
    fn project_selected(&mut self, id: ProjectId) {
        let known = self.selector.projects().iter().any(|p| p.id == id);
        if !known || self.selector.project.as_ref() == Some(&id) {
            return;
        }
        self.selector.project = Some(id);
        self.selector.session = None;
        self.push_sessions();
        self.push_cwd();
    }

    fn session_selected(&mut self, id: SessionId) {
        if self.selector.sessions().iter().any(|s| s.id == id) {
            self.selector.session = Some(id);
        }
    }

    /// Moves focus to `pane`. The Sessions pane selects the newest session if
    /// none is selected.
    fn focus_pane(&mut self, pane: Pane) {
        if self.selector.pane == pane {
            return;
        }
        self.selector.pane = pane;
        let id = match pane {
            Pane::Projects => Id::ProjectList,
            Pane::Sessions => Id::SessionList,
        };
        if let Err(err) = self.application.active(&id) {
            log::warn!("could not move the focus to {id:?}: {err}");
        }
        let before = self.selector.session.clone();
        self.selector.select_newest_if_focused();
        if self.selector.session != before {
            self.push_session_selection();
        }
        self.request_redraw(RedrawRequest::Dirty);
    }

    /// Switches to `harness` if it is registered and available.
    fn switch_harness(&mut self, harness: HarnessId) {
        if self.selector.current != Some(harness) && self.selector.is_available(harness) {
            self.enter_harness(harness);
        }
    }

    /// Shows `harness` from its first project.
    fn enter_harness(&mut self, harness: HarnessId) {
        self.selector.current = Some(harness);
        self.selector.project = self.selector.projects().first().map(|p| p.id.clone());
        self.selector.session = None;
        self.selector.select_newest_if_focused();
        self.push_all();
    }

    // -- activity

    /// Recomputes which listed sessions are active and pushes the flags if
    /// they changed.
    pub(super) fn refresh_activity(&mut self) {
        let active = self.active_ids();
        if active != self.selector.active {
            self.selector.active = active.clone();
            self.set_attr(
                Id::SessionList,
                ACTIVE_ATTR,
                payload_value(ActiveSessions(active)),
            );
        }
    }

    fn active_ids(&self) -> Vec<SessionId> {
        let now = (self.clock)();
        let sessions = self.selector.sessions();
        sessions
            .iter()
            .filter(|s| is_active(s, now))
            .map(|s| s.id.clone())
            .collect()
    }

    // -- pushing data into the components

    fn set_attr(&mut self, id: Id, attr: Attribute, value: AttrValue) {
        match self.application.attr(&id, attr, value) {
            Ok(()) => self.request_redraw(RedrawRequest::Dirty),
            Err(err) => log::warn!("could not update {id:?}: {err}"),
        }
    }

    fn push_all(&mut self) {
        self.push_tabs();
        self.push_projects();
        self.push_sessions();
        self.push_cwd();
    }

    pub(super) fn push_tabs(&mut self) {
        let tabs = self
            .selector
            .registered
            .iter()
            .map(|&id| Tab {
                id,
                available: self.selector.is_available(id),
            })
            .collect();
        let tabs = Tabs {
            tabs,
            selected: self.selector.current,
        };
        self.set_attr(Id::HarnessTabs, TABS_ATTR, payload_value(tabs));
    }

    fn push_projects(&mut self) {
        let rows = self
            .selector
            .projects()
            .iter()
            .map(|p| ProjectRow {
                id: p.id.clone(),
                name: p.name.clone(),
            })
            .collect();
        let rows = ProjectRows {
            rows,
            selected: self.selector.project_index(),
        };
        self.set_attr(Id::ProjectList, PROJECT_ROWS_ATTR, payload_value(rows));
    }

    /// Pushes the sessions of the selected project with their stats and
    /// active flags, and records the flags.
    fn push_sessions(&mut self) {
        self.selector.active = self.active_ids();
        let harness = self.selector.current;
        let rows: Vec<SessionRow> = self
            .selector
            .sessions()
            .iter()
            .map(|s| SessionRow {
                id: s.id.clone(),
                started: self.timestamps.format_short(s.started_at),
                title: s.title.clone(),
                active: self.selector.active.contains(&s.id),
                stats: harness
                    .and_then(|h| self.selector.stats.get(&(h, s.id.clone())))
                    .copied(),
            })
            .collect();
        let project = self
            .selector
            .project_index()
            .map(|i| self.selector.projects()[i].name.clone())
            .unwrap_or_default();
        let rows = SessionRows {
            project,
            rows: Arc::new(rows),
            selected: self.selector.session_index(),
        };
        self.set_attr(Id::SessionList, SESSION_ROWS_ATTR, payload_value(rows));
    }

    fn push_session_selection(&mut self) {
        let selected = self.selector.session_index();
        self.set_attr(Id::SessionList, SELECTED_ATTR, selected_value(selected));
    }

    fn push_cwd(&mut self) {
        let cwd = self
            .selector
            .project_index()
            .and_then(|i| self.selector.projects()[i].cwd.as_ref())
            .map(|cwd| cwd.display().to_string())
            .unwrap_or_default();
        self.set_attr(Id::WorkingDir, CWD_ATTR, AttrValue::String(cwd));
    }

    /// Tells both lists how many rows their pane shows at the current size.
    pub(super) fn push_viewports(&mut self) {
        let area = ratatui::layout::Rect::new(0, 0, self.size.0, self.size.1);
        if self.screen != Screen::Selector || super::layout::too_small(area) {
            return;
        }
        let overlay = super::layout::layout(&self.layout_input(), area)
            .area(Id::SelectorFrame)
            .unwrap_or_default();
        let rows = visible_rows(selector_areas(overlay).projects.height);
        for id in [Id::ProjectList, Id::SessionList] {
            self.set_attr(id, VISIBLE_ROWS_ATTR, AttrValue::Length(rows));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reselect_keeps_the_item_when_its_index_moves() {
        let old = ["a", "b", "c"];
        let new = ["x", "a", "b", "c"];
        assert_eq!(reselect(&old, &new, 1, |a, b| a == b), Some(2));
    }

    #[test]
    fn reselect_uses_the_same_index_when_the_item_vanished() {
        let old = ["a", "b", "c"];
        assert_eq!(reselect(&old, &["a", "c"], 1, |a, b| a == b), Some(1));
        assert_eq!(reselect(&old, &["a", "b"], 2, |a, b| a == b), Some(1));
    }

    #[test]
    fn reselect_is_none_for_an_empty_list_or_an_unknown_index() {
        let old = ["a"];
        assert_eq!(reselect(&old, &[], 0, |a: &&str, b| a == b), None);
        assert_eq!(reselect(&old, &["a"], 5, |a, b| a == b), None);
    }
}
