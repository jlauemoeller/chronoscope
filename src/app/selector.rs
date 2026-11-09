//! The session selector's part of the application: the index data of every
//! harness, the selection, the focused pane, and the handlers that keep the
//! selector components in step with them
//! (architecture.md#application-structure,
//! architecture.md#events-focus-and-keys, product §3.1, §6.3).
//!
//! The `App` owns the data and the selection. The lists own their cursor and
//! scroll offset and report the user's moves as [`SelectorMsg`]s; when the
//! data changes the `App` works out the new selection (by id, falling back to
//! the same index) and pushes rows and selection into the components. It
//! keeps what it gave the Sessions pane ([`ListedSessions`]), so it never
//! reads a list back.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use tuirealm::application::{Application, ApplicationError};
use tuirealm::command::Direction;
use tuirealm::props::{AttrValue, Attribute};

use super::hints::{filtered_selector_hints, selector_hints};
use super::index_port::IndexUpdate;
use super::layout::{browser_areas, selector_areas};
use super::redraw::RedrawRequest;
use super::search::SearchState;
use super::{App, Id, Msg, UserEvent};
use crate::components::{
    ACTIVE_ATTR, ActiveSessions, BackgroundFill, CWD_ATTR, ClickClock, HINTS_ATTR, HarnessTabs,
    HelpLine, ListCursor, PROJECT_ROWS_ATTR, PROJECTS_FILTERED_ATTR, ProjectList, ProjectRow,
    ProjectRows, SELECTED_ATTR, SESSION_FILTERED_ATTR, SESSION_OPEN_ATTR, SESSION_ROWS_ATTR,
    SESSION_STATS_ATTR, SelectorFrame, SessionList, SessionRow, SessionRows, SessionStatsUpdate,
    SessionTime, SessionTokens, TABS_ATTR, Tab, Tabs, VIEWPORT_ATTR, VISIBLE_ROWS_ATTR, WorkingDir,
    hints_value, payload_value, selected_value, viewport_value, visible_rows,
};
use crate::keymap::Keymap;
use crate::launch::{HarnessIndex, Preselect, deepest_ancestor};
use crate::theme::{Role, Theme};

use crate::index::{IndexEvent, SessionMap};
use crate::model::{
    HarnessId, Project, ProjectId, SessionId, SessionStats, SessionSummary, is_active,
};

/// The components of the selector, in drawing order: the screen's
/// background (mounted on the base screen only) and the overlay's.
pub(super) const SELECTOR_IDS: [Id; 9] = [
    Id::ScreenBackground,
    Id::SelectorFrame,
    Id::HarnessTabs,
    Id::ProjectList,
    Id::SessionList,
    Id::SessionTime,
    Id::SessionTokens,
    Id::WorkingDir,
    Id::HelpLine,
];

/// Mounts the components of the session selector. `base` says whether the
/// selector is the base screen, where [`Id::ScreenBackground`] paints the
/// whole screen under it, or an overlay over the browser. Focus is the
/// caller's business.
pub(super) fn mount_selector(
    application: &mut Application<Id, Msg, UserEvent>,
    keymap: &Keymap,
    theme: &Theme,
    click_clock: &ClickClock,
    base: bool,
) -> Result<(), ApplicationError> {
    if base {
        application.mount(
            Id::ScreenBackground,
            Box::new(BackgroundFill::new(theme.clone())),
            Vec::new(),
        )?;
    }
    let frame = SelectorFrame::new(theme.clone());
    let frame = if base { frame } else { frame.over_session() };
    application.mount(Id::SelectorFrame, Box::new(frame), Vec::new())?;
    application.mount(
        Id::HarnessTabs,
        Box::new(HarnessTabs::new(theme.clone())),
        Vec::new(),
    )?;
    application.mount(
        Id::ProjectList,
        Box::new(
            ProjectList::new(keymap.clone(), theme.clone()).with_click_clock(click_clock.clone()),
        ),
        Vec::new(),
    )?;
    application.mount(
        Id::SessionList,
        Box::new(
            SessionList::new(keymap.clone(), theme.clone()).with_click_clock(click_clock.clone()),
        ),
        Vec::new(),
    )?;
    application.mount(
        Id::SessionTime,
        Box::new(SessionTime::new(None, theme.clone())),
        Vec::new(),
    )?;
    application.mount(
        Id::SessionTokens,
        Box::new(SessionTokens::new(None, theme.clone())),
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
    /// The user pressed the resume key in the Projects pane: resume the
    /// session that is selected, if any.
    Resume,
    /// `Esc` in the filtered selector (the sessions whose ids start with
    /// the argument of the command line): back to the normal start.
    LeaveFilter,
    /// A left press on this project's row: focus Projects and select it;
    /// `open` (the press completed a double-click): then, as `Enter` does,
    /// focus Sessions.
    ChooseProject { id: ProjectId, open: bool },
    /// A left press on this session's row (the list's cursor is there now):
    /// select it and focus Sessions; `open` (the press completed a
    /// double-click): then open it.
    ChooseSession { id: SessionId, open: bool },
    /// A click on a pane where there is no row.
    PaneClicked(Pane),
    /// The wheel scrolled the Sessions pane without moving a selection: a
    /// redraw is all that is needed.
    ListScrolled,
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

/// The filter of the Sessions pane (see [`SelectorState::filtered`]).
#[derive(Debug)]
pub(super) struct Filter {
    /// The id prefix of the command line.
    pub(super) prefix: String,
    /// The sessions the command line's lookup found (`--harness` already
    /// applied): the pane lists these and no others.
    matches: HashSet<(HarnessId, SessionId)>,
}

/// What the App last gave the Sessions pane: the sessions of its rows, in
/// row order, and the row selected there.
///
/// The App keeps it so that it never reads the list back (cycle 5 spec,
/// §9.1). It records every push of rows or of a selection, and every move of
/// the cursor the list reports (a key, a click or the wheel). While search
/// results are listed it is the App's own search state: `↑` / `↓` in the
/// prompt move its selection, which the App then pushes to the list.
#[derive(Debug, Default)]
pub(super) struct ListedSessions {
    ids: Vec<SessionId>,
    /// The selected row, kept by the same rules as the list's cursor (only
    /// its selection is used: the scroll offset is the list's).
    cursor: ListCursor,
}

impl ListedSessions {
    /// Records the rows `rows` with `selected` selected, which the list
    /// ignores when it is out of range.
    fn set(&mut self, rows: &[SessionRow], selected: Option<usize>) {
        self.ids = rows.iter().map(|row| row.id.clone()).collect();
        self.cursor.set(self.ids.len(), selected);
    }

    /// Records `selected` as the selected row, as the list takes it: out of
    /// range, nothing is selected.
    pub(super) fn select(&mut self, selected: Option<usize>) {
        self.cursor.select(selected);
    }

    /// Records that the row of session `id` is selected. A session that is
    /// not listed changes nothing.
    fn select_id(&mut self, id: &SessionId) {
        if let Some(row) = self.ids.iter().position(|listed| listed == id) {
            self.cursor.select(Some(row));
        }
    }

    /// The selected row, if any.
    pub(super) fn selected(&self) -> Option<usize> {
        self.cursor.selected()
    }

    /// The session of the selected row, if any.
    pub(super) fn selected_id(&self) -> Option<&SessionId> {
        self.ids.get(self.selected()?)
    }

    /// Moves the selection one row in `direction`, as the list's own keys
    /// do ([`ListCursor`]); a direction other than `Up` or `Down` changes
    /// nothing.
    pub(super) fn step(&mut self, direction: Direction) {
        match direction {
            Direction::Up => {
                self.cursor.step_up();
            }
            Direction::Down => {
                self.cursor.step_down();
            }
            Direction::Left | Direction::Right => {}
        }
    }
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
    /// What the command line says about where to start.
    hint: Preselect,
    /// The id prefix whose sessions the Sessions pane lists, when the
    /// command line gave one that several sessions start with.
    pub(super) filtered: Option<Filter>,
    /// The sessions listed while filtered, in row order.
    pub(super) shown: Vec<SessionId>,
    /// What the Sessions pane was last given, and its selection.
    pub(super) listed: ListedSessions,
    /// What the Session Time and Session Tokens panels were last given.
    pub(super) panels: super::session_panels::PanelValues,
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

    /// The harness and project to start on (product §3.1): the project whose
    /// cwd is the working directory or its deepest ancestor, in the lowest
    /// numbered harness on a tie and the most recently active project of a
    /// harness on a tie; else the first project of the first available
    /// harness. Only available harnesses take part.
    fn preselection(&self) -> Option<Preselection> {
        self.preselection_in(None)
    }

    /// The project the selector starts on in `harness`.
    fn startup_project(&self, harness: HarnessId) -> Option<ProjectId> {
        self.preselection_in(Some(harness))
            .and_then(|want| want.project)
    }

    /// [`preselection`](Self::preselection), restricted to `only` when given.
    fn preselection_in(&self, only: Option<HarnessId>) -> Option<Preselection> {
        let available: Vec<HarnessId> = {
            let mut available: Vec<HarnessId> = self
                .registered
                .iter()
                .copied()
                .filter(|&h| self.is_available(h))
                .collect();
            available.sort_by_key(|h| h.number());
            available
        };
        // `--project` names the project outright; `--harness` the tab.
        if let Some((harness, project)) = &self.hint.project
            && available.contains(harness)
            && only.is_none_or(|only| only == *harness)
            && self.find_project(*harness, project).is_some()
        {
            return Some(Preselection {
                harness: *harness,
                project: Some(project.clone()),
                matched: true,
            });
        }
        let wanted = only.or(self.hint.harness);
        let harnesses: Vec<HarnessId> = match wanted.filter(|h| available.contains(h)) {
            Some(only) => vec![only],
            None => available,
        };
        let candidates = harnesses.iter().flat_map(|&harness| {
            let projects = self
                .data
                .get(&harness)
                .map_or(&[][..], |d| d.projects.as_slice());
            projects.iter().map(move |project| (harness, project))
        });
        if let Some((harness, project)) = deepest_ancestor(candidates, &self.working_dir) {
            return Some(Preselection {
                harness,
                project: Some(project.id.clone()),
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

    /// The sessions of the current harness that the filter found, newest
    /// first, with the name of their project.
    pub(super) fn filtered_hits(&self) -> Vec<(&str, &SessionSummary)> {
        let (Some(filter), Some(harness), Some(data)) =
            (self.filtered.as_ref(), self.current, self.current_data())
        else {
            return Vec::new();
        };
        let mut hits: Vec<(&str, &SessionSummary)> = data
            .projects
            .iter()
            .flat_map(|project| {
                let sessions = data
                    .sessions
                    .get(&project.id)
                    .map_or(&[][..], Vec::as_slice);
                sessions
                    .iter()
                    .filter(|s| filter.matches.contains(&(harness, s.id.clone())))
                    .map(|s| (project.name.as_str(), s))
            })
            .collect();
        hits.sort_by(|a, b| {
            b.1.started_at
                .cmp(&a.1.started_at)
                .then_with(|| a.1.id.cmp(&b.1.id))
        });
        hits
    }

    /// Selects `session` of `harness` with Sessions focused, as the selector
    /// is when it is opened over that session. The session is left
    /// unselected if the last snapshot no longer lists it; the newest one is
    /// selected then (Sessions has focus).
    pub(super) fn show_session(&mut self, harness: HarnessId, session: &SessionSummary) {
        self.preselecting = false;
        self.search = None;
        self.filtered = None;
        self.shown.clear();
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

    /// Tells the selector where the command line says to start. It is
    /// honoured while the selector is still on its startup preselection.
    pub fn set_preselection(&mut self, preselect: Preselect) {
        self.selector.hint = preselect;
    }

    /// Gives the selector the lists the command line was resolved against, as
    /// if the index worker had reported them.
    pub fn seed_index(&mut self, index: &[HarnessIndex]) {
        for harness in index {
            if !self.selector.registered.contains(&harness.harness) {
                continue;
            }
            let data = HarnessData {
                available: harness.available,
                projects: Arc::clone(&harness.projects),
                sessions: Arc::clone(&harness.sessions),
            };
            self.snapshot(harness.harness, data);
        }
    }

    /// Whether the Sessions pane lists the sessions whose ids start with a
    /// prefix given on the command line.
    pub fn filter_prefix(&self) -> Option<&str> {
        self.selector.filtered.as_ref().map(|f| f.prefix.as_str())
    }

    /// The sessions listed while the selector is filtered, in row order.
    pub fn filtered_sessions(&self) -> &[SessionId] {
        &self.selector.shown
    }

    /// Starts the selector filtered to `matches`, the sessions found for the
    /// id prefix `prefix`: the tab of the first one is shown, the Sessions
    /// pane has the focus and lists them as results (the first selected),
    /// and no others, whichever tab is shown. The Projects pane is as at a
    /// start without the prefix. `Esc` leaves the filter.
    pub fn show_filtered(&mut self, prefix: &str, matches: &[(HarnessId, SessionId)]) {
        let Some(&(harness, _)) = matches.first() else {
            return;
        };
        if !self.screen.has_selector() {
            return;
        }
        self.selector.preselecting = false;
        self.selector.search = None;
        self.selector.filtered = Some(Filter {
            prefix: prefix.to_owned(),
            matches: matches.iter().cloned().collect(),
        });
        self.selector.current = Some(harness);
        self.selector.project = self.selector.startup_project(harness);
        self.selector.session = None;
        self.selector.pane = Pane::Sessions;
        if let Err(err) = self.application.active(&Id::SessionList) {
            log::warn!("could not focus the sessions: {err}");
        }
        self.push_filtered(true);
        self.push_selector_hints();
        self.sync_pane_focus();
        self.push_all();
        self.request_redraw(RedrawRequest::Dirty);
    }

    /// Tells the two panes whether the selector is `filtered`.
    fn push_filtered(&mut self, filtered: bool) {
        self.set_attr(
            Id::SessionList,
            SESSION_FILTERED_ATTR,
            AttrValue::Flag(filtered),
        );
        self.set_attr(
            Id::ProjectList,
            PROJECTS_FILTERED_ATTR,
            AttrValue::Flag(filtered),
        );
    }

    /// Shows the hints that fit the selector: those of the filtered one
    /// (`Esc` shows all sessions) while it is filtered.
    fn push_selector_hints(&mut self) {
        let hints = if self.selector.filtered.is_some() {
            filtered_selector_hints(&self.keymap)
        } else {
            selector_hints(&self.keymap)
        };
        self.set_attr(Id::HelpLine, HINTS_ATTR, hints_value(hints));
    }

    /// `Esc` in the filtered selector: the selector is as at a start without
    /// the prefix: the preselection, Projects focused, no session selected.
    fn leave_filtered(&mut self) {
        if self.selector.filtered.take().is_none() {
            return;
        }
        self.selector.shown.clear();
        self.push_filtered(false);
        self.push_selector_hints();
        self.selector.pane = Pane::Projects;
        if let Some(want) = self.selector.preselection() {
            self.selector.current = Some(want.harness);
            self.selector.project = want
                .project
                .or_else(|| self.selector.projects().first().map(|p| p.id.clone()));
        }
        self.selector.session = None;
        if let Err(err) = self.application.active(&Id::ProjectList) {
            log::warn!("could not focus the projects: {err}");
        }
        self.sync_pane_focus();
        self.push_all();
        self.request_redraw(RedrawRequest::Dirty);
    }

    /// Lists the filtered sessions in the Sessions pane, as search results
    /// are, keeping the selected one when it is still there.
    fn push_filtered_results(&mut self) {
        let Some(prefix) = self.selector.filtered.as_ref().map(|f| f.prefix.clone()) else {
            return;
        };
        let previous = self
            .selector
            .listed
            .selected()
            .and_then(|row| self.selector.shown.get(row).cloned());
        let now = (self.clock)();
        let harness = self.selector.current;
        let mut ids = Vec::new();
        let rows: Vec<SessionRow> = self
            .selector
            .filtered_hits()
            .into_iter()
            .map(|(project, s)| {
                ids.push(s.id.clone());
                SessionRow {
                    id: s.id.clone(),
                    started: self.timestamps.format_short(s.started_at),
                    title: s.title.clone(),
                    project: Some(project.to_owned()),
                    active: is_active(s, now),
                    stats: harness
                        .and_then(|h| self.selector.stats.get(&(h, s.id.clone())))
                        .copied(),
                }
            })
            .collect();
        let selected = previous
            .and_then(|id| ids.iter().position(|shown| *shown == id))
            .or_else(|| (!ids.is_empty()).then_some(0));
        self.selector.active = ids
            .iter()
            .zip(&rows)
            .filter(|(_, row)| row.active)
            .map(|(id, _)| id.clone())
            .collect();
        self.selector.shown = ids;
        let rows = SessionRows {
            project: String::new(),
            search: Some(prefix),
            harness,
            rows: Arc::new(rows),
            selected,
            harness_empty: false,
        };
        self.push_session_rows(rows);
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
            // An unavailable harness cannot stay selected (product §5.6): show
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
                || self.selector.filtered.is_some()
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
        if msg == SelectorMsg::Resume {
            // Asking changes nothing in the selector.
            let target = self.selector.current.zip(self.selector.session.clone());
            if target.is_some() {
                self.request_resume(target);
            }
            return;
        }
        // The user has taken over from the startup preselection.
        self.selector.preselecting = false;
        match msg {
            SelectorMsg::ProjectSelected(id) => self.project_selected(id),
            SelectorMsg::SessionSelected(id) => self.session_selected(id),
            SelectorMsg::Focus(pane) => self.focus_pane(pane),
            SelectorMsg::Search => {
                // The search lists its own results: the filter ends.
                self.leave_filtered();
                self.start_search();
            }
            SelectorMsg::SwitchHarness(harness) => self.switch_harness(harness),
            SelectorMsg::LeaveFilter => self.leave_filtered(),
            SelectorMsg::ChooseProject { id, open } => self.choose_project(id, open),
            SelectorMsg::ChooseSession { id, open } => self.choose_session(id, open),
            SelectorMsg::PaneClicked(pane) => self.focus_pane(pane),
            SelectorMsg::ListScrolled => self.request_redraw(RedrawRequest::Dirty),
            SelectorMsg::Resume => {}
        }
    }

    /// A left press on a project row: Projects gets the focus and the
    /// project is selected. With `open` (a double-click) it is `Enter` on it:
    /// Sessions gets the focus and, with none selected, its newest session.
    fn choose_project(&mut self, id: ProjectId, open: bool) {
        // Focus first: in the filtered selector it ends the filter, which
        // resets the project.
        self.focus_pane(Pane::Projects);
        self.project_selected(id);
        if open {
            self.focus_pane(Pane::Sessions);
        }
    }

    /// A left press on a session row: the session is selected and Sessions
    /// gets the focus. With `open` (a double-click) the session opens.
    fn choose_session(&mut self, id: SessionId, open: bool) {
        // Select first: focusing Sessions with nothing selected would select
        // the newest session, which resets the scroll of the list and moves
        // the clicked row from under the pointer.
        self.session_selected(id.clone());
        self.focus_pane(Pane::Sessions);
        if open && let Some(harness) = self.selector.current {
            self.emit(Msg::OpenSession(harness, id));
        }
    }

    /// Another project was selected: its sessions are shown with none
    /// selected.
    pub(super) fn project_selected(&mut self, id: ProjectId) {
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

    /// The Sessions pane's cursor is on the row of session `id` now: the
    /// list reported a key, a click or the wheel that moved it there.
    pub(super) fn session_selected(&mut self, id: SessionId) {
        self.selector.listed.select_id(&id);
        if self.selector.filtered.is_some() || self.selector.search_shows_results() {
            // The rows are sessions of any project: the selection is the
            // row's, which the App has just recorded. Pushing it back keeps
            // the list right when the message came from elsewhere.
            self.push_listed_selection();
            self.request_redraw(RedrawRequest::Dirty);
            return;
        }
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
    pub(super) fn focus_pane(&mut self, pane: Pane) {
        if self.selector.pane == pane {
            return;
        }
        if self.selector.filtered.is_some() {
            // Only the Projects pane can be asked for: leaving the Sessions
            // pane ends the filter.
            self.leave_filtered();
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
        if self.selector.filtered.is_some() {
            return self
                .selector
                .filtered_hits()
                .into_iter()
                .filter(|(_, s)| is_active(s, now))
                .map(|(_, s)| s.id.clone())
                .collect();
        }
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
        // Last: the panels follow the session the lists now show.
        self.push_session_panels();
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
        if self.selector.filtered.is_some() {
            self.push_filtered_results();
            return;
        }
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
        self.push_session_rows(rows);
    }

    /// Gives the Sessions pane `rows`, and records what it was given.
    pub(super) fn push_session_rows(&mut self, rows: SessionRows) {
        self.selector.listed.set(&rows.rows, rows.selected);
        self.set_attr(Id::SessionList, SESSION_ROWS_ATTR, payload_value(rows));
    }

    /// Selects the selected session's row in the Sessions pane. The filtered
    /// selector keeps the row it has: its selection is not the selected
    /// session of a project.
    pub(super) fn push_session_selection(&mut self) {
        if self.selector.filtered.is_some() {
            self.request_redraw(RedrawRequest::Dirty);
            return;
        }
        self.selector.listed.select(self.selector.session_index());
        self.push_listed_selection();
    }

    /// Pushes the selection the App keeps for the Sessions pane to it.
    pub(super) fn push_listed_selection(&mut self) {
        let selected = self.selector.listed.selected();
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
        self.refresh_details_size();
        self.refresh_routing();
        self.push_help_interior();
        self.push_details_interior();
        self.push_theme_selector_rows();
        let area = ratatui::layout::Rect::new(0, 0, self.size.0, self.size.1);
        if super::layout::too_small(area) {
            return;
        }
        if self.screen.has_selector() {
            // The routing layout was refreshed above.
            let overlay = self
                .mouse
                .routing
                .area(Id::SelectorFrame)
                .unwrap_or_default();
            let areas = selector_areas(overlay);
            // Each list gets the height of its own pane: the Sessions pane
            // is shorter while the panels are shown.
            for (id, pane) in [
                (Id::ProjectList, areas.projects),
                (Id::SessionList, areas.sessions),
            ] {
                let rows = visible_rows(pane.height);
                self.set_attr(id, VISIBLE_ROWS_ATTR, AttrValue::Length(rows));
            }
        }
        if self.screen.has_browser() {
            let areas = browser_areas(area);
            self.push_sidebar_viewport(areas.sidebar);
            // The transcript's area is the narrow one when the sidebar is
            // shown, the one drawing and mouse routing use.
            let transcript = areas.transcript;
            // Every transcript, the covered ones too: they are drawn at this
            // size again when the ones over them are closed.
            for depth in 0..=self.browser.depth() {
                self.set_attr(
                    super::browser::transcript_id(depth),
                    VIEWPORT_ATTR,
                    viewport_value(transcript.as_size()),
                );
            }
            // A new size can show the end again, which resumes following,
            // and can move the selection.
            self.sync_view_after_push();
        }
    }

    /// Tells the turns sidebar how many list rows its pane shows. A sidebar
    /// that is hidden (`sidebar` is `None`) has nothing to tell; if it has the
    /// focus, [`release_hidden_sidebar`](App::release_hidden_sidebar) takes it
    /// away.
    fn push_sidebar_viewport(&mut self, sidebar: Option<ratatui::layout::Rect>) {
        let Some(sidebar) = sidebar else {
            return;
        };
        self.set_attr(
            Id::TurnSidebar,
            VISIBLE_ROWS_ATTR,
            AttrValue::Length(crate::components::sidebar_list_rows(sidebar.height)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(rows: usize, selected: Option<usize>) -> ListedSessions {
        let mut listed = ListedSessions::default();
        let rows: Vec<SessionRow> = (0..rows)
            .map(|n| SessionRow {
                id: SessionId::from(format!("s{n}")),
                started: String::new(),
                title: String::new(),
                project: None,
                active: false,
                stats: None,
            })
            .collect();
        listed.set(&rows, selected);
        listed
    }

    #[test]
    fn a_step_of_the_listed_selection_does_not_wrap() {
        let mut l = listed(3, Some(0));
        let step = |l: &mut ListedSessions, direction| {
            l.step(direction);
            l.selected()
        };
        assert_eq!(step(&mut l, Direction::Up), Some(0));
        step(&mut l, Direction::Down);
        assert_eq!(step(&mut l, Direction::Down), Some(2));
        assert_eq!(step(&mut l, Direction::Down), Some(2));
        assert_eq!(step(&mut l, Direction::Up), Some(1));
        // Other directions change nothing.
        assert_eq!(step(&mut l, Direction::Left), Some(1));
        assert_eq!(step(&mut l, Direction::Right), Some(1));
        assert_eq!(l.selected_id().map(SessionId::as_str), Some("s1"));
        // `Down` with nothing selected selects the first row, `Up` nothing;
        // an empty list has nothing to select.
        let mut l = listed(3, None);
        assert_eq!(step(&mut l, Direction::Up), None);
        assert_eq!(step(&mut l, Direction::Down), Some(0));
        let mut l = listed(0, None);
        assert_eq!(step(&mut l, Direction::Down), None);
    }

    #[test]
    fn the_listed_selection_is_dropped_out_of_range_and_follows_reported_rows() {
        let mut l = listed(3, Some(7));
        assert_eq!(l.selected(), None, "as the list drops it");
        l.select_id(&SessionId::from("s2"));
        assert_eq!(l.selected(), Some(2));
        l.select_id(&SessionId::from("elsewhere"));
        assert_eq!(l.selected(), Some(2), "a session not listed moves nothing");
        l.select(Some(3));
        assert_eq!(l.selected_id(), None);
    }

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
