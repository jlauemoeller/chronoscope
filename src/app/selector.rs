//! The session selector's part of the application: the index data of every
//! harness, the selection, the focused pane, and the handlers that keep the
//! selector components in step with them (SPEC §1.2, §1.4, §4.1, §7.3).
//!
//! The `App` owns the data and the selection. The lists own their cursor and
//! scroll offset and report the user's moves as [`SelectorMsg`]s; when the
//! data changes the `App` works out the new selection (by id, falling back to
//! the same index) and pushes rows and selection into the components.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use tuirealm::application::{Application, ApplicationError};
use tuirealm::props::{AttrValue, Attribute};

use super::hints::selector_hints;
use super::index_port::IndexUpdate;
use super::layout::{browser_areas, selector_areas};
use super::redraw::RedrawRequest;
use super::search::SearchState;
use super::{App, Id, Msg, UserEvent};
use crate::components::{
    ACTIVE_ATTR, ActiveSessions, CWD_ATTR, HarnessTabs, HelpLine, PROJECT_ROWS_ATTR, ProjectList,
    ProjectRow, ProjectRows, SELECTED_ATTR, SESSION_OPEN_ATTR, SESSION_ROWS_ATTR,
    SESSION_STATS_ATTR, SelectorFrame, SessionList, SessionRow, SessionRows, SessionStatsUpdate,
    TABS_ATTR, Tab, Tabs, VIEWPORT_ATTR, VISIBLE_ROWS_ATTR, WorkingDir, payload_value,
    selected_value, viewport_value, visible_rows,
};
use crate::keymap::Keymap;
use crate::theme::{Role, Theme};

use crate::index::{IndexEvent, SessionMap};
use crate::model::{
    HarnessId, Project, ProjectId, SessionId, SessionStats, SessionSummary, is_active,
};

/// The components of the selector overlay, in drawing order.
pub(super) const SELECTOR_IDS: [Id; 6] = [
    Id::SelectorFrame,
    Id::HarnessTabs,
    Id::ProjectList,
    Id::SessionList,
    Id::WorkingDir,
    Id::HelpLine,
];

/// Mounts the components of the session selector. `base` says whether the
/// selector is the base screen (it paints the whole screen) or an overlay
/// over the browser. Focus is the caller's business.
pub(super) fn mount_selector(
    application: &mut Application<Id, Msg, UserEvent>,
    keymap: &Keymap,
    theme: &Theme,
    base: bool,
) -> Result<(), ApplicationError> {
    application.mount(
        Id::SelectorFrame,
        Box::new(SelectorFrame::new(theme.clone()).base(base)),
        Vec::new(),
    )?;
    application.mount(
        Id::HarnessTabs,
        Box::new(HarnessTabs::new(theme.clone())),
        Vec::new(),
    )?;
    application.mount(
        Id::ProjectList,
        Box::new(ProjectList::new(keymap.clone(), theme.clone())),
        Vec::new(),
    )?;
    application.mount(
        Id::SessionList,
        Box::new(SessionList::new(keymap.clone(), theme.clone())),
        Vec::new(),
    )?;
    application.mount(
        Id::WorkingDir,
        Box::new(WorkingDir::new(String::new(), theme.clone())),
        Vec::new(),
    )?;
    application.mount(
        Id::HelpLine,
        Box::new(HelpLine::new(
            selector_hints(keymap),
            Vec::new(),
            theme.clone(),
            Role::Surface,
        )),
        Vec::new(),
    )?;
    Ok(())
}

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
    /// The user pressed the search key.
    Search,
    /// The user pressed the number of a harness.
    SwitchHarness(HarnessId),
}

/// What the last snapshot of a harness held.
#[derive(Debug, Clone)]
pub(super) struct HarnessData {
    available: bool,
    pub(super) projects: Arc<Vec<Project>>,
    pub(super) sessions: Arc<SessionMap>,
}

/// Where the selector starts (see [`SelectorState::preselection`]).
struct Preselection {
    harness: HarnessId,
    project: Option<ProjectId>,
    /// Whether the project contains the working directory; false for the
    /// fallback to the first project.
    matched: bool,
}

/// The selector's domain state.
#[derive(Debug, Default)]
pub(super) struct SelectorState {
    /// The harnesses with a tab, in tab order.
    registered: Vec<HarnessId>,
    /// The last snapshot of each harness that sent one.
    data: HashMap<HarnessId, HarnessData>,
    /// The stats received so far; they outlive snapshots.
    pub(super) stats: HashMap<(HarnessId, SessionId), SessionStats>,
    /// The harness shown. `None` until one is available.
    pub(super) current: Option<HarnessId>,
    project: Option<ProjectId>,
    pub(super) session: Option<SessionId>,
    pub(super) pane: Pane,
    /// The sessions pushed as active last.
    pub(super) active: Vec<SessionId>,
    /// The search in progress, if any.
    pub(super) search: Option<SearchState>,
    /// The directory the user started in.
    working_dir: PathBuf,
    /// Whether the harness and project are still the startup preselection:
    /// true until the user acts or every harness has reported once. While it
    /// is, a snapshot of another harness may change the preselection.
    preselecting: bool,
}

impl SelectorState {
    pub(super) fn new(registered: Vec<HarnessId>, working_dir: PathBuf) -> Self {
        Self {
            registered,
            working_dir,
            preselecting: true,
            ..Self::default()
        }
    }

    /// Whether the harness shown has nothing to list: it has no projects, or
    /// no harness is available, once every registered harness has reported
    /// (before that the lists are still loading). A project without sessions
    /// in a harness that has projects is an ordinary empty list.
    fn harness_empty(&self) -> bool {
        match self.current_data() {
            Some(data) => data.projects.is_empty(),
            None => self.current.is_none() && self.all_reported(),
        }
    }

    /// The harnesses the application registered.
    pub(super) fn registered(&self) -> &[HarnessId] {
        &self.registered
    }

    /// Whether every registered harness has sent a snapshot.
    fn all_reported(&self) -> bool {
        self.registered.iter().all(|h| self.data.contains_key(h))
    }

    /// The harness and project to start on (SPEC §4.1): the project whose
    /// cwd is the working directory or its deepest ancestor, in the lowest
    /// numbered harness on a tie and the most recently active project of a
    /// harness on a tie; else the first project of the first available
    /// harness. Only available harnesses take part.
    fn preselection(&self) -> Option<Preselection> {
        let mut harnesses: Vec<HarnessId> = self
            .registered
            .iter()
            .copied()
            .filter(|&h| self.is_available(h))
            .collect();
        harnesses.sort_by_key(|h| h.number());
        let mut best: Option<(usize, HarnessId, &ProjectId)> = None;
        for &harness in &harnesses {
            let projects = self
                .data
                .get(&harness)
                .map_or(&[][..], |d| d.projects.as_slice());
            for project in projects {
                let Some(cwd) = project.cwd.as_ref().filter(|cwd| cwd.is_absolute()) else {
                    continue;
                };
                // `starts_with` compares whole path components.
                if !self.working_dir.starts_with(cwd) {
                    continue;
                }
                let depth = cwd.components().count();
                if best.is_none_or(|(deepest, ..)| depth > deepest) {
                    best = Some((depth, harness, &project.id));
                }
            }
        }
        if let Some((_, harness, project)) = best {
            return Some(Preselection {
                harness,
                project: Some(project.clone()),
                matched: true,
            });
        }
        let first = *harnesses.first()?;
        let project = self
            .data
            .get(&first)
            .and_then(|d| d.projects.first())
            .map(|p| p.id.clone());
        Some(Preselection {
            harness: first,
            project,
            matched: false,
        })
    }

    /// The summary of the session `id` of `harness`, from the last snapshot.
    pub(super) fn find_session(
        &self,
        harness: HarnessId,
        id: &SessionId,
    ) -> Option<&SessionSummary> {
        self.data
            .get(&harness)?
            .sessions
            .values()
            .flatten()
            .find(|s| &s.id == id)
    }

    /// The project `id` of `harness`, from the last snapshot.
    pub(super) fn find_project(&self, harness: HarnessId, id: &ProjectId) -> Option<&Project> {
        self.data
            .get(&harness)?
            .projects
            .iter()
            .find(|p| &p.id == id)
    }

    /// Selects `session` of `harness` with Sessions focused, as the selector
    /// is when it is opened over that session. The session is left
    /// unselected if the last snapshot no longer lists it; the newest one is
    /// selected then (Sessions has focus).
    pub(super) fn show_session(&mut self, harness: HarnessId, session: &SessionSummary) {
        self.preselecting = false;
        self.search = None;
        self.pane = Pane::Sessions;
        self.current = Some(harness);
        let project = self
            .find_project(harness, &session.project)
            .map(|p| p.id.clone())
            .or_else(|| self.projects().first().map(|p| p.id.clone()));
        self.project = project;
        self.session = None;
        if self.sessions().iter().any(|s| s.id == session.id) {
            self.session = Some(session.id.clone());
        }
        self.select_newest_if_focused();
    }

    pub(super) fn current_data(&self) -> Option<&HarnessData> {
        self.data.get(&self.current?)
    }

    fn projects(&self) -> &[Project] {
        self.current_data().map_or(&[], |d| d.projects.as_slice())
    }

    /// The sessions of the selected project, newest first.
    pub(super) fn sessions(&self) -> &[SessionSummary] {
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
    pub(super) fn select_newest_if_focused(&mut self) {
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

    /// Tells the selector whether a session is open behind it, which decides
    /// what `Esc` does: close the selector, or quit.
    pub fn set_session_open(&mut self, open: bool) {
        for id in [Id::ProjectList, Id::SessionList] {
            self.set_attr(id, SESSION_OPEN_ATTR, AttrValue::Flag(open));
        }
    }

    /// The sessions that show the `●` marker: those of the selected project
    /// (of the whole harness while search results are listed).
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
        // Stats of sessions that are gone are dropped.
        let live: HashSet<&SessionId> = data.sessions.values().flatten().map(|s| &s.id).collect();
        self.selector
            .stats
            .retain(|(h, id), _| *h != harness || live.contains(id));
        let available = data.available;
        let old = self.selector.data.insert(harness, data);
        if !available && self.selector.current == Some(harness) {
            // An unavailable harness cannot stay selected (SPEC §6.6): show
            // the preselection among the others, or nothing.
            self.selector.current = None;
            self.selector.project = None;
            self.selector.session = None;
            if self.selector.first_available().is_none() {
                self.push_all();
                return;
            }
        }
        if self.preselect_again() {
            return;
        }
        if self.selector.current == Some(harness) {
            self.keep_selection(old.as_ref());
        } else if self.selector.current.is_none() {
            // The last harness may just have reported that none is available.
            self.push_all();
        } else {
            self.push_tabs();
        }
        self.settle_preselection();
    }

    /// Before the user has acted, a snapshot can change the harness and
    /// project: the first one that holds data picks them, one that arrives
    /// from another harness may hold a better match for the working
    /// directory, and so may a later snapshot of the shown harness (the
    /// first can be empty when its build failed). Live updates of the shown
    /// harness never move a selection the working directory did not pick.
    /// Returns whether the selection was replaced.
    fn preselect_again(&mut self) -> bool {
        if self.selector.current.is_some() && !self.selector.preselecting {
            return false;
        }
        let Some(want) = self.selector.preselection() else {
            return false;
        };
        let same_harness = self.selector.current == Some(want.harness);
        if same_harness && (!want.matched || want.project == self.selector.project) {
            return false;
        }
        self.enter_harness_at(want.harness, want.project);
        self.settle_preselection();
        true
    }

    /// Ends the preselection once every harness has reported and the shown
    /// one has projects: an empty first snapshot (a failed build) does not
    /// count.
    fn settle_preselection(&mut self) {
        if self.selector.all_reported() && !self.selector.projects().is_empty() {
            self.selector.preselecting = false;
        }
    }

    /// The sessions of the current harness were replaced: keep the selected
    /// project and session by id, or the same index if one vanished.
    ///
    /// A project that vanished is a move to another project, so the session
    /// selection goes with it.
    fn keep_selection(&mut self, old: Option<&HarnessData>) {
        let new_projects = self
            .selector
            .current_data()
            .map(|d| Arc::clone(&d.projects))
            .unwrap_or_default();
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
        // Search results come from every project of the harness.
        let shown = self.selector.current == Some(harness)
            && (self.selector.search.is_some()
                || self.selector.sessions().iter().any(|s| &s.id == session));
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
        // The user has taken over from the startup preselection.
        self.selector.preselecting = false;
        match msg {
            SelectorMsg::ProjectSelected(id) => self.project_selected(id),
            SelectorMsg::SessionSelected(id) => self.session_selected(id),
            SelectorMsg::Focus(pane) => self.focus_pane(pane),
            SelectorMsg::Search => self.start_search(),
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
        let selected = self.selector.project_index();
        self.set_attr(Id::ProjectList, SELECTED_ATTR, selected_value(selected));
        self.push_sessions();
        self.push_cwd();
    }

    fn session_selected(&mut self, id: SessionId) {
        let known = self.selector.sessions().iter().any(|s| s.id == id);
        if known && self.selector.session.as_ref() != Some(&id) {
            self.selector.session = Some(id);
            // Usually the list moved its own cursor and this changes nothing
            // there; it keeps the list right when the message came from
            // elsewhere, and asks for the redraw the move needs.
            self.push_session_selection();
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
        self.enter_harness_at(harness, None);
    }

    /// Shows `harness` with `project` selected (its first project when
    /// `None`) and no session, unless Sessions has focus.
    fn enter_harness_at(&mut self, harness: HarnessId, project: Option<ProjectId>) {
        self.selector.current = Some(harness);
        self.selector.project =
            project.or_else(|| self.selector.projects().first().map(|p| p.id.clone()));
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

    /// The sessions that show the marker: those of the selected project, or,
    /// while search results are listed, those of every project of the
    /// harness (sorted, so the list only changes when the set does).
    pub(super) fn active_ids(&self) -> Vec<SessionId> {
        let now = (self.clock)();
        if self.selector.search_shows_results() {
            let mut ids: Vec<SessionId> = self
                .selector
                .current_data()
                .into_iter()
                .flat_map(|d| d.sessions.values().flatten())
                .filter(|s| is_active(s, now))
                .map(|s| s.id.clone())
                .collect();
            ids.sort();
            return ids;
        }
        let sessions = self.selector.sessions();
        sessions
            .iter()
            .filter(|s| is_active(s, now))
            .map(|s| s.id.clone())
            .collect()
    }

    // -- pushing data into the components

    pub(super) fn set_attr(&mut self, id: Id, attr: Attribute, value: AttrValue) {
        if !self.application.mounted(&id) {
            // The selector is unmounted while a session is open (and the
            // browser while the selector is the base screen); the state is
            // kept and pushed when it is mounted again.
            return;
        }
        match self.application.attr(&id, attr, value) {
            Ok(()) => self.request_redraw(RedrawRequest::Dirty),
            Err(err) => log::warn!("could not update {id:?}: {err}"),
        }
    }

    pub(super) fn push_all(&mut self) {
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
    pub(super) fn push_sessions(&mut self) {
        if self.selector.search_shows_results() {
            // The results replace the project's sessions until the search
            // ends.
            self.push_search_results(true);
            return;
        }
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
                project: None,
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
            search: None,
            harness,
            rows: Arc::new(rows),
            selected: self.selector.session_index(),
            harness_empty: self.selector.harness_empty(),
        };
        self.set_attr(Id::SessionList, SESSION_ROWS_ATTR, payload_value(rows));
    }

    pub(super) fn push_session_selection(&mut self) {
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

    /// Tells the lists how many rows their pane shows at the current size,
    /// and the transcript how big its area is.
    pub(super) fn push_viewports(&mut self) {
        self.push_help_interior();
        let area = ratatui::layout::Rect::new(0, 0, self.size.0, self.size.1);
        if super::layout::too_small(area) {
            return;
        }
        if self.screen.has_selector() {
            let overlay = super::layout::layout(&self.layout_input(), area)
                .area(Id::SelectorFrame)
                .unwrap_or_default();
            let rows = visible_rows(selector_areas(overlay).projects.height);
            for id in [Id::ProjectList, Id::SessionList] {
                self.set_attr(id, VISIBLE_ROWS_ATTR, AttrValue::Length(rows));
            }
        }
        if self.screen.has_browser() {
            let transcript = browser_areas(area).transcript;
            // Every transcript, the covered ones too: they are drawn at this
            // size again when the ones over them are closed.
            for depth in 0..=self.browser.depth() {
                self.set_attr(
                    super::browser::transcript_id(depth),
                    VIEWPORT_ATTR,
                    viewport_value(transcript.as_size()),
                );
            }
            // A new size can show the end again, which resumes following.
            self.sync_new_sections();
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
