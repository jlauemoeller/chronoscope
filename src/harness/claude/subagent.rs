//! Session ids to files, and the linkage from an `Agent`/`Task` tool call to
//! its subagent transcript (product §2.1, §4.4).
//!
//! A subagent transcript lives at
//! `<project dir>/<session-uuid>/subagents/agent-<agentId>.jsonl`, next to
//! `agent-<agentId>.meta.json`. Its [`SessionId`] is
//! `<session-uuid>/subagents/agent-<agentId>`, which makes the file name
//! `<id>.jsonl` below the project directory, like that of a main session.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use serde::Deserialize;
use serde_json::Value;

pub use crate::harness::shared::SubagentInfo;
use crate::model::SessionId;

/// The fields of a meta file the linkage needs.
#[derive(Deserialize)]
struct Meta {
    #[serde(rename = "toolUseId")]
    tool_use_id: Option<String>,
}

/// The shape of a session id.
enum IdKind<'a> {
    /// A main session: `<uuid>`.
    Main,
    /// A subagent: `<uuid>/subagents/agent-<agentId>`, whose transcript is
    /// `<root>/subagents/<agent>.jsonl` below its project directory.
    Subagent { root: &'a str, agent: &'a str },
}

/// Classifies `id` and rejects anything that is not a plain session or
/// subagent id (empty or `.`/`..` components, separators, drive prefixes), so
/// an id can never name a file outside the data directory.
fn classify(id: &str) -> Option<IdKind<'_>> {
    let safe = |part: &str| {
        !part.is_empty() && part != "." && part != ".." && !part.contains(['\\', ':', '\0'])
    };
    let parts: Vec<&str> = id.split('/').collect();
    if !parts.iter().all(|p| safe(p)) {
        return None;
    }
    match parts[..] {
        [_] => Some(IdKind::Main),
        [root, "subagents", agent] if agent.starts_with("agent-") => {
            Some(IdKind::Subagent { root, agent })
        }
        _ => None,
    }
}

/// The transcript files the user named on the command line, which need not
/// be under the data directory: file stem to file. Shared with the loader
/// threads, so a clone sees what the others register.
#[derive(Debug, Clone, Default)]
pub(super) struct Opened(Arc<Mutex<HashMap<String, PathBuf>>>);

impl Opened {
    /// Remembers that session `stem` is the file `path`.
    pub(super) fn insert(&self, stem: String, path: PathBuf) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(stem, path);
    }

    fn get(&self, stem: &str) -> Option<PathBuf> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(stem)
            .cloned()
    }
}

/// The file of an opened session, or of a subagent of one: its transcripts
/// are in `<stem>/subagents/` next to the file.
fn resolve_opened(opened: &Opened, id: &SessionId) -> Option<PathBuf> {
    if let Some(path) = opened.get(id.as_str()) {
        return path.exists().then_some(path);
    }
    let Some(IdKind::Subagent { root, agent }) = classify(id.as_str()) else {
        return None;
    };
    let parent = opened.get(root)?;
    let candidate = parent
        .with_extension("")
        .join("subagents")
        .join(format!("{agent}.jsonl"));
    candidate.exists().then_some(candidate)
}

/// Finds the transcript of session `id`: a file the user opened by its path,
/// else `<data_dir>/<project dir>/<id>.jsonl`. Lists the project directories
/// (sorted by name, so a clash is resolved the same way every time) and stats
/// one candidate in each; reads no file.
pub(super) fn resolve(data_dir: &Path, opened: &Opened, id: &SessionId) -> Option<PathBuf> {
    if let Some(path) = resolve_opened(opened, id) {
        return Some(path);
    }
    let file = file_below_project(id, classify(id.as_str())?);
    let mut dirs: Vec<PathBuf> = fs::read_dir(data_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    dirs.sort();
    dirs.into_iter()
        .map(|dir| dir.join(&file))
        .find(|candidate| candidate.exists())
}

/// The transcript of session `id`, of shape `kind`, relative to its project
/// directory, joined one component at a time: `<id>.jsonl` for a main
/// session, `<root>/subagents/<agent>.jsonl` for a subagent.
fn file_below_project(id: &SessionId, kind: IdKind<'_>) -> PathBuf {
    match kind {
        IdKind::Main => PathBuf::from(format!("{id}.jsonl")),
        IdKind::Subagent { root, agent } => Path::new(root)
            .join("subagents")
            .join(format!("{agent}.jsonl")),
    }
}

/// The directory that holds the subagent transcripts of `session`, whose
/// transcript is `path`. For a subagent this is the directory it is in.
pub(super) fn subagents_dir(path: &Path, session: &SessionId) -> Option<PathBuf> {
    match classify(session.as_str())? {
        IdKind::Main => Some(path.with_extension("").join("subagents")),
        IdKind::Subagent { .. } => path.parent().map(Path::to_path_buf),
    }
}

/// A subagent transcript found below a session's `subagents` directory, with
/// the stat that identifies its content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SubagentFile {
    pub path: PathBuf,
    pub size: u64,
    pub mtime: SystemTime,
}

/// Lists the subagent transcripts of the session whose transcript is
/// `session_path`: every `agent-*.jsonl` found recursively below
/// `<session file without extension>/subagents/`, in path order. Workflow
/// runs nest theirs under `subagents/workflows/<run>/`. Other files there
/// (`agent-*.meta.json`, `journal.jsonl`) are not transcripts and do not
/// match. A symlink to a directory is not followed. A missing directory means
/// no subagents and logs nothing; a directory that cannot be listed is
/// logged. A symlink to a file that cannot be stat'ed (a dangling one) is
/// listed with size 0, so that reading it logs the failure. Anything that
/// is demonstrably not a regular file (a FIFO, a socket, a device), which
/// opening could block on, is left out. The `subagents` directory itself is
/// not followed when it is a symlink.
pub(super) fn list_transcripts(session_path: &Path) -> Vec<SubagentFile> {
    list(session_path, log::Level::Warn)
}

/// Like [`list_transcripts`], for the stats key, which runs on every rebuild:
/// a directory that cannot be listed is logged at `debug` only, so that it
/// does not log again for every session at every rebuild. [`list_transcripts`]
/// (from `stats`) still warns.
pub(super) fn list_transcripts_quietly(session_path: &Path) -> Vec<SubagentFile> {
    list(session_path, log::Level::Debug)
}

fn list(session_path: &Path, level: log::Level) -> Vec<SubagentFile> {
    let mut found = Vec::new();
    let root = session_path.with_extension("").join("subagents");
    if fs::symlink_metadata(&root).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return found;
    }
    list_below(&root, &mut found, level);
    found.sort_by(|a, b| a.path.cmp(&b.path));
    found
}

fn list_below(dir: &Path, found: &mut Vec<SubagentFile>, level: log::Level) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
        Err(err) => {
            log::log!(level, "could not list {}: {err}", dir.display());
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                log::log!(level, "could not list {}: {err}", dir.display());
                continue;
            }
        };
        let path = entry.path();
        let kind = match entry.file_type() {
            Ok(kind) => kind,
            Err(err) => {
                log::log!(level, "could not stat {}: {err}", path.display());
                continue;
            }
        };
        if kind.is_dir() {
            list_below(&path, found, level);
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !(name.starts_with("agent-") && name.ends_with(".jsonl")) {
            continue;
        }
        let meta = if kind.is_symlink() {
            fs::metadata(&path).ok()
        } else {
            entry.metadata().ok()
        };
        // A directory behind a link is not followed; a FIFO or a device is
        // not a transcript, and opening it could block.
        if meta.as_ref().is_some_and(|meta| !meta.is_file()) {
            continue;
        }
        found.push(SubagentFile {
            path,
            size: meta.as_ref().map_or(0, fs::Metadata::len),
            mtime: meta
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH),
        });
    }
}

/// Finds the subagent started by `call` in `parent`: through the meta file
/// whose `toolUseId` is `call`, else through the `agentId` of the call's
/// `tool_result` entry in the parent transcript.
pub(super) fn find(
    data_dir: &Path,
    opened: &Opened,
    parent: &SessionId,
    call: &str,
) -> Option<SessionId> {
    let path = resolve(data_dir, opened, parent)?;
    let dir = subagents_dir(&path, parent)?;
    let root = match classify(parent.as_str())? {
        IdKind::Main => parent.as_str(),
        IdKind::Subagent { root, .. } => root,
    };
    let agent = find_by_meta(&dir, call).or_else(|| find_by_result(&path, &dir, call))?;
    Some(SessionId::from(format!("{root}/subagents/agent-{agent}")))
}

/// Reads the meta file of subagent session `id`.
pub(super) fn info(data_dir: &Path, opened: &Opened, id: &SessionId) -> Option<SubagentInfo> {
    let IdKind::Subagent { .. } = classify(id.as_str())? else {
        return None;
    };
    let path = resolve(data_dir, opened, id)?;
    let meta = path.with_extension("meta.json");
    let text = match fs::read_to_string(&meta) {
        Ok(text) => text,
        Err(err) => {
            log::warn!("could not read {}: {err}", meta.display());
            return None;
        }
    };
    match serde_json::from_str(&text) {
        Ok(info) => Some(info),
        Err(err) => {
            log::warn!("could not parse {}: {err}", meta.display());
            None
        }
    }
}

/// Whether `agent` can be part of a file name.
fn is_agent_id(agent: &str) -> bool {
    !agent.is_empty()
        && agent
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn transcript_exists(dir: &Path, agent: &str) -> bool {
    dir.join(format!("agent-{agent}.jsonl")).is_file()
}

/// The agent id of the meta file naming `call`, whose transcript exists.
fn find_by_meta(dir: &Path, call: &str) -> Option<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names.into_iter().find_map(|name| {
        let agent = name.strip_prefix("agent-")?.strip_suffix(".meta.json")?;
        if !is_agent_id(agent) || !transcript_exists(dir, agent) {
            return None;
        }
        let text = fs::read_to_string(dir.join(&name)).ok()?;
        let meta: Meta = serde_json::from_str(&text).ok()?;
        (meta.tool_use_id.as_deref() == Some(call)).then(|| agent.to_owned())
    })
}

#[derive(Deserialize)]
struct ResultLine {
    message: Option<ResultMessage>,
    #[serde(rename = "toolUseResult")]
    tool_use_result: Option<Value>,
}

#[derive(Deserialize)]
struct ResultMessage {
    content: Option<Value>,
}

/// The `toolUseResult.agentId` of the entry that carries the `tool_result`
/// of `call`, whose transcript exists. Scans the parent transcript.
fn find_by_result(parent: &Path, dir: &Path, call: &str) -> Option<String> {
    let mut reader = BufReader::new(File::open(parent).ok()?);
    let mut line = Vec::new();
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) => return None,
            Ok(_) => {}
            Err(err) => {
                log::warn!("could not read {}: {err}", parent.display());
                return None;
            }
        }
        // Cheap filter before parsing: only the result entry has all three.
        let Ok(text) = std::str::from_utf8(&line) else {
            continue;
        };
        if !text.contains("\"toolUseResult\"") || !text.contains(call) {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<ResultLine>(text) else {
            continue;
        };
        let Some(Value::Array(blocks)) = entry.message.and_then(|m| m.content) else {
            continue;
        };
        let is_result_of_call = blocks.iter().any(|b| {
            b.get("type")
                .and_then(Value::as_str)
                .is_some_and(|k| k.ends_with("tool_result"))
                && b.get("tool_use_id").and_then(Value::as_str) == Some(call)
        });
        if !is_result_of_call {
            continue;
        }
        let agent = entry
            .tool_use_result
            .as_ref()
            .and_then(|r| r.get("agentId")?.as_str())
            .filter(|a| is_agent_id(a) && transcript_exists(dir, a));
        if let Some(agent) = agent {
            return Some(agent.to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_session_and_subagent_ids_are_accepted() {
        for ok in ["abc", "abc/subagents/agent-a1"] {
            assert!(classify(ok).is_some(), "{ok}");
        }
        for bad in [
            "",
            "..",
            "../x",
            "a/b",
            "/abs",
            "a//b",
            "a\\b",
            "c:x",
            "a/subagents/other",
            "a/subagents/agent-x/y",
            "a/../agent-x",
            "a/subagents/..",
        ] {
            assert!(classify(bad).is_none(), "{bad:?}");
        }
    }

    /// A data directory with one project directory, `proj`.
    fn data_dir() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        let project = data.join("proj");
        fs::create_dir_all(&project).unwrap();
        (tmp, data, project)
    }

    fn plant(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "{}\n").unwrap();
    }

    #[test]
    fn a_subagent_is_found_below_its_project_directory_part_by_part() {
        let (_tmp, data, project) = data_dir();
        let main = project.join("ses.jsonl");
        let sub = project.join("ses").join("subagents").join("agent-a1.jsonl");
        plant(&main);
        plant(&sub);
        let opened = Opened::default();
        assert_eq!(resolve(&data, &opened, &SessionId::from("ses")), Some(main));
        let found = resolve(&data, &opened, &SessionId::from("ses/subagents/agent-a1"));
        assert_eq!(found.as_deref(), Some(sub.as_path()));
        assert!(found.unwrap().starts_with(&project));
        assert_eq!(
            resolve(&data, &opened, &SessionId::from("ses/subagents/agent-gone")),
            None
        );
    }

    #[test]
    fn an_id_that_would_leave_its_project_directory_resolves_to_nothing() {
        let (tmp, data, project) = data_dir();
        let outside = tmp.path().join("outside");
        // Every file such an id could name, were it joined as it is.
        let bad = [
            ("a/../agent-x", project.join("agent-x.jsonl")),
            (
                "../subagents/agent-x",
                data.join("subagents").join("agent-x.jsonl"),
            ),
            (
                "a/subagents/agent-x/y",
                project.join("a/subagents/agent-x").join("y.jsonl"),
            ),
            (
                "a\\subagents\\agent-x",
                project.join("a\\subagents\\agent-x.jsonl"),
            ),
            (
                "C:x/subagents/agent-y",
                project.join("C:x/subagents/agent-y.jsonl"),
            ),
        ];
        for (_, file) in &bad {
            plant(file);
        }
        let absolute = format!("{}/subagents/agent-x", outside.display());
        plant(&outside.join("subagents").join("agent-x.jsonl"));
        let opened = Opened::default();
        for id in bad
            .iter()
            .map(|(id, _)| (*id).to_owned())
            .chain([absolute, "/etc/subagents/agent-x".to_owned()])
        {
            assert_eq!(
                resolve(&data, &opened, &SessionId::from(id.as_str())),
                None,
                "{id:?}"
            );
        }
    }

    #[test]
    fn the_subagent_of_an_opened_file_is_found_next_to_it() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("elsewhere").join("ses.jsonl");
        let sub = tmp.path().join("elsewhere/ses/subagents/agent-a1.jsonl");
        plant(&file);
        plant(&sub);
        let opened = Opened::default();
        opened.insert("ses".to_owned(), file.clone());
        let data = tmp.path().join("data");
        assert_eq!(resolve(&data, &opened, &SessionId::from("ses")), Some(file));
        assert_eq!(
            resolve(&data, &opened, &SessionId::from("ses/subagents/agent-a1")),
            Some(sub)
        );
    }

    #[test]
    fn the_listing_finds_transcripts_at_any_depth_with_their_size_and_mtime() {
        let tmp = tempfile::tempdir().unwrap();
        let session = tmp.path().join("ses.jsonl");
        let root = tmp.path().join("ses").join("subagents");
        let nested = root.join("workflows").join("run1");
        fs::create_dir_all(&nested).unwrap();
        fs::write(&session, "").unwrap();
        fs::write(root.join("agent-a1.jsonl"), "12345").unwrap();
        fs::write(root.join("agent-a1.meta.json"), "{}").unwrap();
        fs::write(root.join("journal.jsonl"), "{}").unwrap();
        fs::write(root.join("notes.jsonl"), "{}").unwrap();
        fs::write(nested.join("agent-w1.jsonl"), "123").unwrap();
        fs::write(nested.join("journal.jsonl"), "{}").unwrap();

        let found = list_transcripts(&session);
        let paths: Vec<_> = found.iter().map(|f| f.path.clone()).collect();
        assert_eq!(
            paths,
            [root.join("agent-a1.jsonl"), nested.join("agent-w1.jsonl")]
        );
        assert_eq!(found[0].size, 5);
        assert_eq!(found[1].size, 3);
        let mtime = fs::metadata(&paths[0]).unwrap().modified().unwrap();
        assert_eq!(found[0].mtime, mtime);
    }

    #[test]
    fn a_missing_subagents_directory_lists_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(list_transcripts(&tmp.path().join("ses.jsonl")).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn the_listing_does_not_follow_a_link_to_a_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let session = tmp.path().join("ses.jsonl");
        let root = tmp.path().join("ses").join("subagents");
        let elsewhere = tmp.path().join("elsewhere");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&elsewhere).unwrap();
        fs::write(elsewhere.join("agent-z.jsonl"), "{}").unwrap();
        std::os::unix::fs::symlink(&elsewhere, root.join("linked")).unwrap();
        // A directory that looks like a transcript is not one either.
        std::os::unix::fs::symlink(&elsewhere, root.join("agent-dir.jsonl")).unwrap();
        // A dangling link is listed, so that reading it logs the failure.
        std::os::unix::fs::symlink(tmp.path().join("nowhere"), root.join("agent-gone.jsonl"))
            .unwrap();
        let found = list_transcripts(&session);
        let names: Vec<_> = found
            .iter()
            .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["agent-gone.jsonl"]);
        assert_eq!(found[0].size, 0);
    }

    #[cfg(unix)]
    #[test]
    fn a_subagents_directory_that_is_a_link_is_not_followed() {
        let tmp = tempfile::tempdir().unwrap();
        let elsewhere = tmp.path().join("elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        fs::write(elsewhere.join("agent-z.jsonl"), "{}").unwrap();
        fs::create_dir_all(tmp.path().join("ses")).unwrap();
        std::os::unix::fs::symlink(&elsewhere, tmp.path().join("ses").join("subagents")).unwrap();
        assert!(list_transcripts(&tmp.path().join("ses.jsonl")).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_fifo_named_like_a_transcript_is_not_listed() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("ses").join("subagents");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("agent-ok.jsonl"), "{}").unwrap();
        let made = std::process::Command::new("mkfifo")
            .arg(root.join("agent-pipe.jsonl"))
            .status();
        if !made.is_ok_and(|status| status.success()) {
            eprintln!("mkfifo is not available: nothing to test");
            return;
        }
        // A link to the pipe is not a transcript either.
        std::os::unix::fs::symlink(root.join("agent-pipe.jsonl"), root.join("agent-link.jsonl"))
            .unwrap();
        let found = list_transcripts(&tmp.path().join("ses.jsonl"));
        let names: Vec<_> = found
            .iter()
            .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["agent-ok.jsonl"]);
    }

    #[test]
    fn agent_ids_are_file_name_safe() {
        assert!(is_agent_id("a1b2-C_3"));
        assert!(!is_agent_id(""));
        assert!(!is_agent_id("../x"));
        assert!(!is_agent_id("a b"));
    }
}
