//! What the command line asks for, resolved against the loaded index before
//! the terminal is set up (product §5.8).
//!
//! [`resolve`] is a pure function of the arguments ([`Request`]), the working
//! directory and the index ([`HarnessIndex`]); it gives what to start with
//! ([`Resolution`]) or an error that is printed on stderr.

use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use crate::harness::Harness;
use crate::index::SessionMap;
use crate::model::{HarnessId, Project, ProjectId, SessionId, SessionSummary};

/// What the user asked for, from the command line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Request {
    /// A session id, a prefix of one, or the path of a `.jsonl` file.
    pub target: Option<String>,
    /// `--latest`.
    pub latest: bool,
    /// `--harness`.
    pub harness: Option<HarnessId>,
    /// `--project`: a path or a display name.
    pub project: Option<String>,
}

impl Request {
    /// Whether resolving needs the project and session lists: a positional
    /// argument (an id, or a path, whose project name the header shows), a
    /// `--latest` or a `--project`. (`--harness` alone does not.)
    pub fn needs_index(&self) -> bool {
        self.latest || self.project.is_some() || self.target.is_some()
    }
}

/// The projects and sessions one harness lists.
#[derive(Debug, Clone)]
pub struct HarnessIndex {
    pub harness: HarnessId,
    /// Whether the harness's data store exists.
    pub available: bool,
    pub projects: Arc<Vec<Project>>,
    pub sessions: Arc<SessionMap>,
}

impl HarnessIndex {
    /// Reads the lists of `harness` now, on the calling thread. A harness
    /// whose lists cannot be read is logged and lists nothing.
    pub fn read(harness: &dyn Harness) -> Self {
        let available = harness.available();
        let mut projects = Vec::new();
        let mut sessions = SessionMap::new();
        if available {
            match crate::index::collect(harness) {
                Ok((p, s)) => (projects, sessions) = (p, s),
                Err(err) => log::warn!("could not read the {} sessions: {err}", harness.id()),
            }
        }
        Self {
            harness: harness.id(),
            available,
            projects: Arc::new(projects),
            sessions: Arc::new(sessions),
        }
    }
}

/// Where the selector starts, when the command line says so.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Preselect {
    /// The tab (`--harness`, or the harness of `--project`).
    pub harness: Option<HarnessId>,
    /// The project (`--project`).
    pub project: Option<(HarnessId, ProjectId)>,
}

/// What to start with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// The selector, as at a start without arguments.
    Selector,
    /// The selector listing the sessions whose ids start with `prefix`.
    Filtered {
        prefix: String,
        /// The matches, lowest harness number first, newest first.
        matches: Vec<(HarnessId, SessionId)>,
    },
    /// This session of the index.
    Open(HarnessId, SessionSummary),
    /// The Claude transcript at this path.
    File(PathBuf),
}

/// The result of [`resolve`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    pub target: Target,
    pub preselect: Preselect,
    /// Things the user should know that did not stop the start.
    pub warnings: Vec<String>,
}

/// Why the command line cannot be satisfied. The message is printed after
/// `chronoscope: `.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchError {
    /// No session id starts with the argument.
    NoSessionMatches(String),
    /// A `.jsonl` path that is not a file.
    NoSuchFile(PathBuf),
    /// An existing file that is not a `.jsonl` file.
    NotATranscript(PathBuf),
    /// A path was given with a harness other than Claude.
    PathNeedsClaude(HarnessId),
    /// A path was given while the Claude harness is disabled.
    ClaudeDisabled,
    /// `--latest` found no project.
    NoProject(String),
    /// `--latest` found a project without sessions.
    NoSessions(String),
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSessionMatches(id) => write!(f, "no session matches \"{id}\""),
            Self::NoSuchFile(path) => write!(f, "no such file: {}", path.display()),
            Self::NotATranscript(path) => {
                write!(f, "not a .jsonl transcript: {}", path.display())
            }
            Self::PathNeedsClaude(harness) => write!(
                f,
                "a .jsonl file is a Claude session; it cannot be opened with --harness {}",
                harness.cache_prefix()
            ),
            Self::ClaudeDisabled => f.write_str(
                "the Claude harness is disabled in the config; cannot open a .jsonl file",
            ),
            Self::NoProject(what) => write!(f, "no project {what}"),
            Self::NoSessions(project) => write!(f, "the project {project} has no sessions"),
        }
    }
}

impl std::error::Error for LaunchError {}

/// Whether a positional argument is a path to a transcript rather than a
/// session id: it ends with `.jsonl` or is an existing file.
pub fn is_path(target: &str) -> bool {
    target.ends_with(".jsonl") || Path::new(target).is_file()
}

/// Resolves `request` for a process started in `working_dir`, against
/// `index` (the harnesses with a tab, in tab order; empty when the request
/// does not [need it](Request::needs_index)).
///
/// `registered` lists the harnesses the config enables.
pub fn resolve(
    request: &Request,
    working_dir: &Path,
    registered: &[HarnessId],
    index: &[HarnessIndex],
) -> Result<Resolution, LaunchError> {
    let mut warnings = Vec::new();
    let in_scope: Vec<&HarnessIndex> = index
        .iter()
        .filter(|h| request.harness.is_none_or(|only| h.harness == only))
        .collect();
    if let Some(only) = request.harness
        && !registered.contains(&only)
    {
        warnings.push(format!("--harness: the {only} harness is not enabled"));
    }
    let mut preselect = Preselect {
        harness: request.harness,
        project: None,
    };
    if let Some(spec) = &request.project {
        match find_projects(&in_scope, spec, working_dir).first() {
            Some(&(harness, project)) => {
                preselect = Preselect {
                    harness: Some(harness),
                    project: Some((harness, project.id.clone())),
                };
            }
            None => warnings.push(format!("--project: no project matches \"{spec}\"")),
        }
    }
    let target = if let Some(target) = &request.target {
        resolve_target(target, request.harness, registered, &in_scope)?
    } else if request.latest {
        latest(request, working_dir, &in_scope)?
    } else {
        Target::Selector
    };
    Ok(Resolution {
        target,
        preselect,
        warnings,
    })
}

/// The positional argument: a path or an id.
fn resolve_target(
    target: &str,
    only: Option<HarnessId>,
    registered: &[HarnessId],
    in_scope: &[&HarnessIndex],
) -> Result<Target, LaunchError> {
    if is_path(target) {
        let path = Path::new(target);
        if !path.is_file() {
            return Err(LaunchError::NoSuchFile(path.to_path_buf()));
        }
        if path.extension().is_none_or(|ext| ext != "jsonl") {
            return Err(LaunchError::NotATranscript(path.to_path_buf()));
        }
        if let Some(harness) = only.filter(|&h| h != HarnessId::Claude) {
            return Err(LaunchError::PathNeedsClaude(harness));
        }
        if !registered.contains(&HarnessId::Claude) {
            return Err(LaunchError::ClaudeDisabled);
        }
        return Ok(Target::File(path.to_path_buf()));
    }
    let mut exact = Vec::new();
    let mut prefixed = Vec::new();
    for harness in in_scope {
        for session in harness.sessions.values().flatten() {
            if session.id.as_str() == target {
                exact.push((harness.harness, session));
            } else if !target.is_empty() && session.id.as_str().starts_with(target) {
                prefixed.push((harness.harness, session));
            }
        }
    }
    // An exact id is that session, even when it is also the prefix of others.
    let mut found = if exact.is_empty() { prefixed } else { exact };
    match found.as_slice() {
        [] => Err(LaunchError::NoSessionMatches(target.to_owned())),
        [(harness, session)] => Ok(Target::Open(*harness, (*session).clone())),
        _ => {
            found.sort_by(|a, b| {
                (a.0.number(), std::cmp::Reverse(a.1.started_at), &a.1.id).cmp(&(
                    b.0.number(),
                    std::cmp::Reverse(b.1.started_at),
                    &b.1.id,
                ))
            });
            Ok(Target::Filtered {
                prefix: target.to_owned(),
                matches: found
                    .into_iter()
                    .map(|(harness, session)| (harness, session.id.clone()))
                    .collect(),
            })
        }
    }
}

/// `--latest`: the newest session of the `--project` project, or of the
/// project of the working directory.
fn latest(
    request: &Request,
    working_dir: &Path,
    in_scope: &[&HarnessIndex],
) -> Result<Target, LaunchError> {
    let (projects, what) = match &request.project {
        Some(spec) => (
            find_projects(in_scope, spec, working_dir),
            format!("matches \"{spec}\""),
        ),
        None => (
            projects_of_dir(in_scope, working_dir),
            format!("for {}", working_dir.display()),
        ),
    };
    if projects.is_empty() {
        return Err(LaunchError::NoProject(what));
    }
    let newest = projects
        .iter()
        .filter_map(|&(harness, project)| {
            let index = in_scope.iter().find(|h| h.harness == harness)?;
            let sessions = index.sessions.get(&project.id)?;
            sessions.iter().map(|s| (harness, s)).max_by(|a, b| {
                a.1.started_at
                    .cmp(&b.1.started_at)
                    .then_with(|| b.0.number().cmp(&a.0.number()))
            })
        })
        .max_by(|a, b| {
            a.1.started_at
                .cmp(&b.1.started_at)
                .then_with(|| b.0.number().cmp(&a.0.number()))
        });
    match newest {
        Some((harness, session)) => Ok(Target::Open(harness, session.clone())),
        None => Err(LaunchError::NoSessions(projects[0].1.name.clone())),
    }
}

/// The project whose `cwd` is `dir` or its deepest ancestor (product §3.1): the
/// deepest one wins, the lowest harness number among equals, and the first
/// of a harness's list (the most recently active) among those.
///
/// `candidates` come in that order of preference.
pub fn deepest_ancestor<'a, T>(
    candidates: impl IntoIterator<Item = (T, &'a Project)>,
    dir: &Path,
) -> Option<(T, &'a Project)> {
    let mut best: Option<(usize, T, &Project)> = None;
    for (tag, project) in candidates {
        let Some(cwd) = project.cwd.as_ref().filter(|cwd| cwd.is_absolute()) else {
            continue;
        };
        // `starts_with` compares whole path components.
        if !dir.starts_with(cwd) {
            continue;
        }
        let depth = cwd.components().count();
        if best.as_ref().is_none_or(|(deepest, ..)| depth > *deepest) {
            best = Some((depth, tag, project));
        }
    }
    best.map(|(_, tag, project)| (tag, project))
}

/// The projects at the deepest ancestor of `dir` (one per harness that has
/// one at that depth).
fn projects_of_dir<'a>(in_scope: &[&'a HarnessIndex], dir: &Path) -> Vec<(HarnessId, &'a Project)> {
    let ordered = by_number(in_scope);
    let candidates = ordered
        .iter()
        .flat_map(|h| h.projects.iter().map(|p| (h.harness, p)));
    let Some((_, best)) = deepest_ancestor(candidates, dir) else {
        return Vec::new();
    };
    let depth = best.cwd.as_ref().map(|cwd| cwd.components().count());
    ordered
        .iter()
        .flat_map(|h| h.projects.iter().map(|p| (h.harness, p)))
        .filter(|(_, p)| {
            p.cwd.as_ref().is_some_and(|cwd| {
                cwd.is_absolute() && dir.starts_with(cwd) && Some(cwd.components().count()) == depth
            })
        })
        .collect()
}

fn by_number<'a>(in_scope: &[&'a HarnessIndex]) -> Vec<&'a HarnessIndex> {
    let mut ordered = in_scope.to_vec();
    ordered.sort_by_key(|h| h.harness.number());
    ordered
}

/// The projects `spec` names, best first: at most one per harness, the
/// lowest harness number first. `spec` is a path (absolute, or relative to
/// `working_dir`; compared by components) or, when no project has that
/// working directory, a display name (case-insensitive).
fn find_projects<'a>(
    in_scope: &[&'a HarnessIndex],
    spec: &str,
    working_dir: &Path,
) -> Vec<(HarnessId, &'a Project)> {
    let wanted = normalize(&working_dir.join(spec));
    let ordered = by_number(in_scope);
    let first_of = |matches: &dyn Fn(&Project) -> bool| -> Vec<(HarnessId, &'a Project)> {
        ordered
            .iter()
            .filter_map(|h| {
                let project = h.projects.iter().find(|p| matches(p))?;
                Some((h.harness, project))
            })
            .collect()
    };
    let by_path =
        first_of(&|p: &Project| p.cwd.as_ref().is_some_and(|cwd| normalize(cwd) == wanted));
    if !by_path.is_empty() {
        return by_path;
    }
    let name = spec.to_lowercase();
    first_of(&|p: &Project| p.name.to_lowercase() == name)
}

/// `path` without `.` components and with `..` applied lexically.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push(component);
                }
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests;
