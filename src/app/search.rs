//! Fuzzy session search in the selector (SPEC §4.1): the matching, and the
//! `App`'s handling of the prompt that types the query.
//!
//! `/` mounts the [`Prompt`] over the help line and gives it the focus; the
//! Sessions pane keeps its focused look and, once the query is not empty,
//! lists the sessions of every project of the harness that match it. `↑` /
//! `↓` in the prompt move the selection in the list (the `App` forwards them
//! as an attribute), `Enter` asks to open the selected result and `Esc`
//! clears the query and puts the selector back as the search found it.

use std::sync::Arc;

use nucleo::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32Str};
use tuirealm::command::Direction;
use tuirealm::props::{AttrValue, Attribute};

use super::search_worker::{SearchRequest, SearchResult, SearchUpdate, SearchWorker};
use super::selector::SelectorState;
use super::{App, Id, Msg, Pane};
use crate::components::{
    Prompt, PromptMode, SEARCH_NAV_ATTR, SELECTED_ATTR, SESSION_ROWS_ATTR, SessionRow, SessionRows,
    payload_value, selected_from_value,
};
use crate::index::SessionMap;
use crate::model::{Project, SessionId, SessionSummary, is_active};

/// The label of the search prompt.
pub const SEARCH_LABEL: &str = "/";

/// One matching session.
#[derive(Debug, Clone, Copy)]
pub struct Hit<'a> {
    /// The session.
    pub session: &'a SessionSummary,
    /// The project it belongs to.
    pub project: &'a Project,
    /// How well it matches; higher is better.
    pub score: u32,
    /// Where it is: the project's index in the projects searched and the
    /// session's index in that project's sessions.
    pub position: (usize, usize),
}

/// The sessions of `projects` (found in `sessions`) that fuzzy-match `query`,
/// best match first, newest first among equal scores.
///
/// A session matches when the query matches its title, its first prompt or
/// its project's display name (smart case: an upper-case letter in the query
/// makes it case-sensitive). A query of several words needs every word to
/// match, in the same field or spread over the three. A query without any
/// word matches nothing.
pub fn search_sessions<'a>(
    query: &str,
    projects: &'a [Project],
    sessions: &'a SessionMap,
) -> Vec<Hit<'a>> {
    let pattern = Pattern::new(
        query,
        CaseMatching::Smart,
        Normalization::Smart,
        AtomKind::Fuzzy,
    );
    if pattern.atoms.is_empty() {
        return Vec::new();
    }
    let several_words = pattern.atoms.len() > 1;
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut buf = Vec::new();
    let mut score = |text: &str| {
        let haystack = Utf32Str::new(text, &mut buf);
        pattern.score(haystack, &mut matcher)
    };
    let mut hits = Vec::new();
    for (project_index, project) in projects.iter().enumerate() {
        let Some(listed) = sessions.get(&project.id) else {
            continue;
        };
        let of_project = score(&project.name);
        for (session_index, session) in listed.iter().enumerate() {
            let mut best = of_project;
            let fields = [
                Some(session.title.as_str()),
                session.first_prompt.as_deref(),
            ];
            for field in fields.into_iter().flatten() {
                best = best.max(score(field));
            }
            if best.is_none() && several_words {
                let all = format!(
                    "{}\n{}\n{}",
                    project.name,
                    session.title,
                    session.first_prompt.as_deref().unwrap_or_default()
                );
                best = score(&all);
            }
            if let Some(score) = best {
                hits.push(Hit {
                    session,
                    project,
                    score,
                    position: (project_index, session_index),
                });
            }
        }
    }
    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| b.session.started_at.cmp(&a.session.started_at))
            .then_with(|| a.session.id.cmp(&b.session.id))
    });
    hits
}

/// A search in progress.
#[derive(Debug)]
pub(super) struct SearchState {
    /// The pane that had the focus when the search started.
    origin_pane: Pane,
    /// The session that was selected then.
    origin_session: Option<SessionId>,
    /// What has been typed.
    query: String,
    /// The listed results, in row order; empty while the query is.
    hits: Vec<SessionId>,
    /// The request the worker has not answered yet.
    pending: Option<PendingSearch>,
    /// `Enter` was pressed while the results were still being searched.
    submit_when_ready: bool,
}

/// A request sent to the search worker.
#[derive(Debug, Clone, Copy)]
struct PendingSearch {
    seq: u64,
    /// Whether the selected result stays selected if it is still found (the
    /// data changed under the same query) or the first one is selected (the
    /// query changed).
    keep_selection: bool,
}

impl SelectorState {
    /// Whether the Sessions pane lists search results: a search is on and
    /// its query is not empty. Otherwise it shows the selected project.
    pub(super) fn search_shows_results(&self) -> bool {
        self.search
            .as_ref()
            .is_some_and(|search| !search.query.trim().is_empty())
    }
}

impl App {
    /// The query being typed, while a search is on.
    pub fn search_query(&self) -> Option<&str> {
        self.selector.search.as_ref().map(|s| s.query.as_str())
    }

    /// The sessions listed as search results, best first.
    pub fn search_results(&self) -> &[SessionId] {
        self.selector
            .search
            .as_ref()
            .map_or(&[], |search| search.hits.as_slice())
    }

    /// The selected search result, if there is one.
    pub fn selected_result(&self) -> Option<SessionId> {
        let index = self.selected_row()?;
        self.search_results().get(index).cloned()
    }

    /// The row selected in the Sessions pane, asked of the list. While the
    /// results are listed the list moves its own cursor (`SEARCH_NAV_ATTR`)
    /// and reports no message, so this is how the app learns the selection.
    fn selected_row(&self) -> Option<usize> {
        let value = self
            .application
            .query(&Id::SessionList, SELECTED_ATTR)
            .ok()??
            .into_attr();
        selected_from_value(&value)?
    }

    /// `/`: opens the prompt. The Sessions pane gets the focus (and the
    /// newest session when none is selected, as for any other way of
    /// focusing it); what the selector was like is remembered for `Esc`.
    pub(super) fn start_search(&mut self) {
        if self.selector.search.is_some() {
            return;
        }
        let prompt = Prompt::new(SEARCH_LABEL, "", PromptMode::Text, self.theme.clone());
        if let Err(err) = self.open_overlay(Id::Prompt, Box::new(prompt)) {
            log::warn!("could not open the search prompt: {err}");
            return;
        }
        self.selector.search = Some(SearchState {
            origin_pane: self.selector.pane,
            origin_session: self.selector.session.clone(),
            query: String::new(),
            hits: Vec::new(),
            pending: None,
            submit_when_ready: false,
        });
        self.selector.pane = Pane::Sessions;
        self.selector.select_newest_if_focused();
        self.push_session_selection();
        self.sync_pane_focus();
    }

    /// The text of the prompt changed: lists the new matches, or the
    /// project's sessions again when the query is empty.
    pub(super) fn search_changed(&mut self, text: String) {
        let Some(search) = self.selector.search.as_mut() else {
            return;
        };
        search.query = text;
        // An `Enter` waiting for an answer was for the old query.
        search.submit_when_ready = false;
        if self.selector.search_shows_results() {
            self.push_search_results(false);
            return;
        }
        // Nothing to search for: an answer still on its way is stale.
        if let Some(search) = self.selector.search.as_mut() {
            search.hits.clear();
            search.pending = None;
        }
        self.push_sessions();
    }

    /// `↑` / `↓` in the prompt: the list moves its selection.
    pub(super) fn search_nav(&mut self, direction: Direction) {
        if self.selector.search_shows_results() {
            self.set_attr(Id::SessionList, SEARCH_NAV_ATTR, payload_value(direction));
        }
    }

    /// `Enter` in the prompt: asks to open the selected result. With no
    /// result nothing happens.
    pub(super) fn search_submit(&mut self) {
        let Some(harness) = self.selector.current else {
            return;
        };
        if let Some(search) = self.selector.search.as_mut()
            && search.pending.is_some()
        {
            // The list shows the results of an older query: wait for the
            // answer and open its first result then.
            search.submit_when_ready = true;
            return;
        }
        if let Some(session) = self.selected_result() {
            self.emit(Msg::OpenSession(harness, session));
        }
    }

    /// `Esc` in the prompt: clears the query and closes the prompt. The
    /// selector is as the search found it: the same pane has the focus and
    /// the same session is selected (the newest one, in the Sessions pane,
    /// if it is gone).
    pub(super) fn search_cancel(&mut self) {
        let Some(search) = self.selector.search.take() else {
            return;
        };
        self.close_overlay(Id::Prompt);
        self.selector.pane = search.origin_pane;
        let known = |id: &SessionId| self.selector.sessions().iter().any(|s| &s.id == id);
        self.selector.session = search.origin_session.filter(known);
        self.selector.select_newest_if_focused();
        self.sync_pane_focus();
        self.push_sessions();
    }

    /// Searches the harness for the query: on the worker when there is one,
    /// else right here. The results are listed when they are in. The
    /// selected result stays selected when `keep_selection` and it is still
    /// found; else the first one is.
    pub(super) fn push_search_results(&mut self, keep_selection: bool) {
        let Some(query) = self.selector.search.as_ref().map(|s| s.query.clone()) else {
            return;
        };
        let (projects, sessions) = self
            .selector
            .current_data()
            .map(|d| (Arc::clone(&d.projects), Arc::clone(&d.sessions)))
            .unwrap_or_default();
        self.search_seq += 1;
        let request = SearchRequest {
            seq: self.search_seq,
            query,
            projects,
            sessions,
        };
        let sent = self
            .search_worker
            .as_ref()
            .is_some_and(|worker| worker.request(request.clone()));
        if sent {
            if let Some(search) = self.selector.search.as_mut() {
                // A refresh (the data changed) never turns a request for a
                // new query into one that keeps the old selection.
                let keep_selection =
                    keep_selection && search.pending.is_none_or(|pending| pending.keep_selection);
                search.pending = Some(PendingSearch {
                    seq: request.seq,
                    keep_selection,
                });
            }
        } else {
            let result = SearchResult::of(request);
            self.list_search_results(&result, keep_selection);
        }
    }

    /// Hands the app the thread that runs its searches; without one it
    /// searches on the calling thread. The app owns it from now on;
    /// [`shutdown`](Self::shutdown) stops it.
    pub fn set_search_worker(&mut self, worker: SearchWorker) {
        self.search_worker = Some(worker);
    }

    /// The worker answered a request.
    pub(super) fn search_result_arrived(&mut self, update: &SearchUpdate) {
        let Some(search) = self.selector.search.as_mut() else {
            return;
        };
        let Some(pending) = search.pending.filter(|p| p.seq == update.seq()) else {
            // The answer to a request that a newer one has replaced.
            return;
        };
        search.pending = None;
        self.list_search_results(update.result(), pending.keep_selection);
        let submit = self
            .selector
            .search
            .as_mut()
            .is_some_and(|search| std::mem::take(&mut search.submit_when_ready));
        if submit {
            self.search_submit();
        }
    }

    /// Lists what a search found in the Sessions pane.
    fn list_search_results(&mut self, result: &SearchResult, keep_selection: bool) {
        let previous = keep_selection.then(|| self.selected_result()).flatten();
        let now = (self.clock)();
        let harness = self.selector.current;
        let mut ids = Vec::with_capacity(result.hits.len());
        let rows: Vec<SessionRow> = result
            .found()
            .map(|(project, s)| {
                ids.push(s.id.clone());
                SessionRow {
                    id: s.id.clone(),
                    started: self.timestamps.format_short(s.started_at),
                    title: s.title.clone(),
                    project: Some(project.name.clone()),
                    active: is_active(s, now),
                    stats: harness
                        .and_then(|h| self.selector.stats.get(&(h, s.id.clone())))
                        .copied(),
                }
            })
            .collect();
        let selected = previous
            .and_then(|id| ids.iter().position(|hit| *hit == id))
            .or_else(|| (!ids.is_empty()).then_some(0));
        self.selector.active = self.active_ids();
        if let Some(search) = self.selector.search.as_mut() {
            search.hits = ids;
        }
        let rows = SessionRows {
            project: String::new(),
            search: Some(result.query.clone()),
            harness,
            rows: Arc::new(rows),
            selected,
            harness_empty: false,
        };
        self.set_attr(Id::SessionList, SESSION_ROWS_ATTR, payload_value(rows));
    }

    /// Gives the lists the look of the pane that has the focus. While the
    /// prompt has the input focus, the Sessions pane keeps its focused look.
    pub(super) fn sync_pane_focus(&mut self) {
        let sessions = self.selector.pane == Pane::Sessions;
        self.set_attr(
            Id::ProjectList,
            Attribute::Focus,
            AttrValue::Flag(!sessions),
        );
        self.set_attr(Id::SessionList, Attribute::Focus, AttrValue::Flag(sessions));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::fixtures::{at, project, session};
    use crate::model::HarnessId;

    fn hit_ids<'a>(hits: &[Hit<'a>]) -> Vec<&'a str> {
        hits.iter().map(|h| h.session.id.as_str()).collect()
    }

    fn with_prompt(mut s: SessionSummary, prompt: &str) -> SessionSummary {
        s.first_prompt = Some(prompt.to_owned());
        s
    }

    /// Three projects of Claude with sessions that match `brio` by their
    /// title, their first prompt and their project's name, and some that do
    /// not (no `b` in them).
    fn claude() -> (Vec<Project>, SessionMap) {
        let at_ = |d| at(2026, 10, d, 10, 0, 0);
        let brio = project(HarnessId::Claude, "BRIO", at_(20));
        let other = project(HarnessId::Claude, "Agentmon", at_(21));
        let third = project(HarnessId::Claude, "Calira", at_(22));
        let mut sessions = SessionMap::new();
        sessions.insert(
            brio.id.clone(),
            vec![session(&brio, "in-brio", "Fix the login", at_(10), at_(10))],
        );
        sessions.insert(
            other.id.clone(),
            vec![
                session(&other, "by-title", "Brio onboarding", at_(11), at_(11)),
                with_prompt(
                    session(&other, "by-prompt", "Quick fix", at_(12), at_(12)),
                    "please look at the brio importer",
                ),
                session(&other, "no-match", "Add dark mode", at_(13), at_(13)),
            ],
        );
        sessions.insert(
            third.id.clone(),
            vec![
                with_prompt(
                    session(&third, "no-prompt-match", "Cleanup", at_(14), at_(14)),
                    "tidy the cache",
                ),
                session(
                    &third,
                    "fuzzy",
                    "Bug report in the old module",
                    at_(15),
                    at_(15),
                ),
            ],
        );
        (vec![brio, other, third], sessions)
    }

    #[test]
    fn the_title_the_first_prompt_and_the_project_name_are_searched() {
        let (projects, sessions) = claude();
        let hits = search_sessions("brio", &projects, &sessions);
        let ids = hit_ids(&hits);
        for expected in ["in-brio", "by-title", "by-prompt", "fuzzy"] {
            assert!(ids.contains(&expected), "{expected} in {ids:?}");
        }
        for unexpected in ["no-match", "no-prompt-match"] {
            assert!(!ids.contains(&unexpected), "{unexpected} in {ids:?}");
        }
        let in_brio = hits.iter().find(|h| h.session.id.as_str() == "in-brio");
        assert_eq!(in_brio.map(|h| h.project.name.as_str()), Some("BRIO"));
    }

    #[test]
    fn matching_is_fuzzy() {
        let (projects, sessions) = claude();
        let hits = search_sessions("bgrpt", &projects, &sessions);
        assert_eq!(hit_ids(&hits), ["fuzzy"]);
    }

    #[test]
    fn smart_case_ignores_case_until_the_query_has_an_upper_case_letter() {
        let (projects, sessions) = claude();
        let lower = search_sessions("onboarding", &projects, &sessions);
        assert_eq!(hit_ids(&lower), ["by-title"]);
        let upper = search_sessions("Onboarding", &projects, &sessions);
        assert!(upper.is_empty(), "{:?}", hit_ids(&upper));
    }

    #[test]
    fn a_session_matches_through_the_first_prompt_alone() {
        let (projects, sessions) = claude();
        let hits = search_sessions("importer", &projects, &sessions);
        assert_eq!(hit_ids(&hits), ["by-prompt"]);
        let hits = search_sessions("tidy cache", &projects, &sessions);
        assert_eq!(hit_ids(&hits), ["no-prompt-match"]);
    }

    #[test]
    fn a_session_matches_through_its_project_name_alone() {
        let (projects, sessions) = claude();
        let hits = search_sessions("agentmon", &projects, &sessions);
        assert_eq!(hit_ids(&hits).len(), 3, "every session of the project");
        assert!(hits.iter().all(|h| h.project.name == "Agentmon"));
    }

    #[test]
    fn several_words_may_match_in_different_fields() {
        let (projects, sessions) = claude();
        let hits = search_sessions("brio login", &projects, &sessions);
        assert_eq!(hit_ids(&hits), ["in-brio"]);
        let hits = search_sessions("agentmon importer", &projects, &sessions);
        assert_eq!(hit_ids(&hits), ["by-prompt"]);
    }

    #[test]
    fn an_empty_or_blank_query_matches_nothing() {
        let (projects, sessions) = claude();
        assert!(search_sessions("", &projects, &sessions).is_empty());
        assert!(search_sessions("   ", &projects, &sessions).is_empty());
    }

    #[test]
    fn only_the_projects_given_are_searched() {
        // The caller passes the projects and sessions of one harness: a
        // session of another harness is never seen.
        let (projects, mut sessions) = claude();
        let opencode = project(
            HarnessId::OpenCode,
            "BRIO-opencode",
            at(2026, 10, 1, 0, 0, 0),
        );
        sessions.insert(
            opencode.id.clone(),
            vec![session(
                &opencode,
                "other-harness",
                "brio brio",
                at(2026, 10, 5, 0, 0, 0),
                at(2026, 10, 5, 0, 0, 0),
            )],
        );
        let hits = search_sessions("brio", &projects, &sessions);
        assert!(!hit_ids(&hits).contains(&"other-harness"));
    }

    #[test]
    fn results_are_sorted_by_score_then_newest_first() {
        let (projects, sessions) = claude();
        let hits = search_sessions("brio", &projects, &sessions);
        for pair in hits.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            assert!(a.score >= b.score);
            if a.score == b.score {
                assert!(a.session.started_at >= b.session.started_at);
            }
        }
        // A session whose title is `Brio onboarding` beats one with the
        // letters scattered over its title.
        let ids = hit_ids(&hits);
        let position = |id| ids.iter().position(|i| *i == id).unwrap();
        assert!(position("by-title") < position("fuzzy"), "{ids:?}");
    }

    #[test]
    fn equal_scores_are_ordered_newest_first_then_by_id() {
        let stamp = |d| at(2026, 10, d, 10, 0, 0);
        let p = project(HarnessId::Claude, "Zed", stamp(1));
        let mut sessions = SessionMap::new();
        sessions.insert(
            p.id.clone(),
            vec![
                session(&p, "old", "same title", stamp(1), stamp(1)),
                session(&p, "new", "same title", stamp(9), stamp(9)),
                session(&p, "mid-b", "same title", stamp(5), stamp(5)),
                session(&p, "mid-a", "same title", stamp(5), stamp(5)),
            ],
        );
        let hits = search_sessions("same", std::slice::from_ref(&p), &sessions);
        assert!(hits.windows(2).all(|w| w[0].score == w[1].score));
        assert_eq!(hit_ids(&hits), ["new", "mid-a", "mid-b", "old"]);
    }

    #[test]
    fn five_thousand_sessions_are_searched_and_sorted() {
        // Not a timing (debug builds are slow): the big data set works.
        let (projects, sessions) = crate::app::fixtures::big_data(5000);
        let hits = search_sessions("importer 42", &projects, &sessions);
        assert!(!hits.is_empty());
        assert!(hits.windows(2).all(|w| w[0].score >= w[1].score));
    }
}
