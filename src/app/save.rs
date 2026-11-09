//! Saving a section and exporting the session to files (product §5.4, §8.5).
//!
//! 1. `w` / `W` in the transcript answer [`OutputMsg::SaveSection`] with
//!    the text, `e` / `E` answer [`OutputMsg::Export`] with the markdown
//!    document.
//! 2. The `App` opens the [`Prompt`] in the help line, labelled `Save to: `
//!    or `Export to: `, with the default path filled in and the cursor at its
//!    end. The text waits in the browser's [`PendingWrite`].
//! 3. `Enter` ([`PromptMsg::Submit`] with [`PromptPurpose::SavePath`]) names
//!    the path. A path inside the data of a harness is refused. A path that
//!    exists is asked about with the `Overwrite <path>? (y/n)` prompt
//!    ([`PromptPurpose::Overwrite`]), whose [`PromptMsg::Confirm`] writes on
//!    `y`, and cancels on any other key. A path that does not exist is
//!    written at once.
//! 4. `Esc` ([`PromptMsg::Cancel`]) cancels. A toast reports the result.
//!
//! It handles the save and export requests of the output concern
//! ([`OutputMsg`]) and the answers of these two prompts.
//!
//! [`OutputMsg`]: super::OutputMsg
//! [`OutputMsg::SaveSection`]: super::OutputMsg::SaveSection
//! [`OutputMsg::Export`]: super::OutputMsg::Export
//! [`PromptMsg::Submit`]: super::PromptMsg::Submit
//! [`PromptMsg::Confirm`]: super::PromptMsg::Confirm
//! [`PromptMsg::Cancel`]: super::PromptMsg::Cancel
//!
//! Paths are relative to the directory the application was started in, and
//! `~` is the home directory. No directory is created.

use std::path::{Component, Path, PathBuf};

use super::layout::Screen;
use super::{App, Id, PromptPurpose};
use unicode_width::UnicodeWidthStr;

use crate::components::text_fit::truncate_start;
use crate::components::{MAX_TOAST_WIDTH, Prompt, TOAST_CHROME};
use crate::paths::expand_tilde;
use crate::theme::Role;

/// The columns the prompt is inset from each side, like the browser's help
/// line text.
const PROMPT_INSET: u16 = 1;

/// The toast shown when the path is inside the data of a harness.
pub const REFUSED: &str = "Refusing to write into harness data";

/// What is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WriteKind {
    /// The text of a section.
    Save,
    /// The session as markdown.
    Export,
}

impl WriteKind {
    /// The label of the path prompt.
    fn label(self) -> &'static str {
        match self {
            WriteKind::Save => "Save to: ",
            WriteKind::Export => "Export to: ",
        }
    }

    /// `Saved to <path>` or `Exported to <path>`; a path too long for the
    /// toast is cut at its start with `…`, so that the file name stays.
    fn done(self, path: &str) -> String {
        let prefix = match self {
            WriteKind::Save => "Saved to ",
            WriteKind::Export => "Exported to ",
        };
        let room = usize::from(MAX_TOAST_WIDTH - TOAST_CHROME).saturating_sub(prefix.width());
        format!("{prefix}{}", truncate_start(path, room))
    }

    fn failed(self, error: &std::io::Error) -> String {
        match self {
            WriteKind::Save => format!("Save failed: {error}"),
            WriteKind::Export => format!("Export failed: {error}"),
        }
    }
}

/// A file the user is being asked about: its text, and, once a path that
/// exists was given, the path the overwrite is confirmed for.
#[derive(Debug)]
pub(super) struct PendingWrite {
    kind: WriteKind,
    text: String,
    overwrite: Option<Overwrite>,
}

/// The path that exists, as the user typed it and resolved.
#[derive(Debug)]
struct Overwrite {
    shown: String,
    path: PathBuf,
}

impl App {
    /// Sets the directory relative paths are resolved against; the directory
    /// the application started in unless a test says another.
    #[doc(hidden)]
    pub fn set_save_dir(&mut self, dir: PathBuf) {
        self.save_dir = dir;
    }

    /// Sets the home directory `~` stands for (`None`: `~` is not expanded).
    #[doc(hidden)]
    pub fn set_home_dir(&mut self, home: Option<PathBuf>) {
        self.home_dir = home;
    }

    /// Whether the path or overwrite prompt of a save or export is open.
    pub fn write_prompt_open(&self) -> bool {
        self.browser.write.is_some()
    }

    /// `w` / `W`: asks where to save `text`, the selected section of `kind`.
    pub(super) fn save_section(&mut self, text: String, kind: &str) {
        let Some(project) = self.default_name_project() else {
            return;
        };
        let default = format!("./{project}-{}-{kind}.txt", self.short_id());
        self.ask_path(WriteKind::Save, text, default);
    }

    /// `e` / `E`: asks where to export `document`.
    pub(super) fn export_session(&mut self, document: String) {
        let Some(open) = self.browser.open.as_ref() else {
            return;
        };
        let started = self
            .timestamps
            .format_with(open.summary.started_at, "%Y%m%d-%H%M");
        let Some(project) = self.default_name_project() else {
            return;
        };
        let default = format!("./{project}-{started}-{}.md", self.short_id());
        self.ask_path(WriteKind::Export, document, default);
    }

    /// The project's name as a part of a file name.
    fn default_name_project(&self) -> Option<String> {
        let name = sanitize_for_file_name(&self.open_project_name()?);
        Some(if name.is_empty() {
            "session".to_owned()
        } else {
            name
        })
    }

    /// The first 8 characters of the open session's id, as a part of a file
    /// name.
    fn short_id(&self) -> String {
        // A subagent's id is `<session>/subagents/agent-<id>`: its own part
        // names the file.
        let id = self
            .browser
            .current_session()
            .map_or("", |id| id.as_str().rsplit('/').next().unwrap_or_default());
        sanitize_for_file_name(&id.chars().take(8).collect::<String>())
    }

    /// Opens the path prompt with `default` filled in.
    fn ask_path(&mut self, kind: WriteKind, text: String, default: String) {
        if self.screen != Screen::Browser
            || !self.application.mounted(&self.top_transcript())
            || self.application.mounted(&Id::Prompt)
        {
            return;
        }
        let prompt = Prompt::new(
            kind.label(),
            default,
            PromptPurpose::SavePath,
            self.theme.clone(),
        )
        .with_fill(Role::Background)
        .inset(PROMPT_INSET);
        match self.open_overlay(Id::Prompt, Box::new(prompt)) {
            Ok(()) => {
                self.browser.write = Some(PendingWrite {
                    kind,
                    text,
                    overwrite: None,
                });
            }
            Err(err) => log::warn!("could not open the path prompt: {err}"),
        }
    }

    /// `Enter` in the path prompt: `typed` is the path. An `Enter` that was
    /// already on its way when the overwrite question replaced the path
    /// prompt (two in one tick) answers that question no: nothing is
    /// written and the prompt closes.
    pub(super) fn path_submitted(&mut self, typed: &str) {
        let Some(pending) = self.browser.write.take() else {
            return;
        };
        self.close_overlay(Id::Prompt);
        if pending.overwrite.is_none() {
            self.path_chosen(pending.kind, pending.text, typed);
        }
    }

    /// The answer to the overwrite question: yes writes the file, no writes
    /// nothing. Ignored while the path is asked: only the question answers.
    pub(super) fn overwrite_answered(&mut self, yes: bool) {
        let Some(pending) = self.browser.write.take_if(|w| w.overwrite.is_some()) else {
            return;
        };
        self.close_overlay(Id::Prompt);
        if yes && let Some(overwrite) = pending.overwrite {
            self.write_file(
                pending.kind,
                &pending.text,
                &overwrite.path,
                &overwrite.shown,
            );
        }
    }

    /// `Esc` in the path prompt, or one that was on its way when the
    /// overwrite question replaced it: nothing is written.
    pub(super) fn write_prompt_cancel(&mut self) {
        if self.browser.write.take().is_some() {
            self.close_overlay(Id::Prompt);
        }
    }

    /// The user gave a path: refuses it, asks about overwriting it, or
    /// writes to it.
    fn path_chosen(&mut self, kind: WriteKind, text: String, typed: &str) {
        let shown = typed.trim().to_owned();
        if shown.is_empty() {
            return;
        }
        let path = self.resolve_path(&shown);
        if self.is_harness_data(&path) {
            log::warn!(
                "refusing to write {}: it is inside harness data",
                path.display()
            );
            self.toast_quiet(REFUSED);
            return;
        }
        // A directory is not asked about: writing to it fails, and says so.
        if path.symlink_metadata().is_err() || path.is_dir() {
            self.write_file(kind, &text, &path, &shown);
            return;
        }
        let prompt = Prompt::confirm(
            PromptPurpose::Overwrite,
            "Overwrite ",
            &shown,
            "? (y/n)",
            self.theme.clone(),
        )
        .with_fill(Role::Background)
        .inset(PROMPT_INSET);
        match self.open_overlay(Id::Prompt, Box::new(prompt)) {
            Ok(()) => {
                self.browser.write = Some(PendingWrite {
                    kind,
                    text,
                    overwrite: Some(Overwrite { shown, path }),
                });
            }
            Err(err) => log::warn!("could not open the overwrite prompt: {err}"),
        }
    }

    /// Writes `text` to `path` (created or truncated) and says how it went.
    fn write_file(&mut self, kind: WriteKind, text: &str, path: &Path, shown: &str) {
        match std::fs::write(path, text) {
            Ok(()) => self.toast_quiet(kind.done(shown)),
            Err(err) => {
                log::warn!("could not write {}: {err}", path.display());
                self.toast_quiet(kind.failed(&err));
            }
        }
    }

    /// `typed` as a path: `~` expanded, a relative path under the directory
    /// the application started in.
    fn resolve_path(&self, typed: &str) -> PathBuf {
        let path = match &self.home_dir {
            Some(home) => expand_tilde(typed, home),
            None => PathBuf::from(typed),
        };
        if path.is_absolute() {
            path
        } else {
            self.save_dir.join(path)
        }
    }

    /// Whether `path` is inside a directory a harness reads its data from
    /// (product §8.5: chronoscope never writes there). The check is made when
    /// the path is given, not again when the file is written, so a link made
    /// in between is not caught.
    fn is_harness_data(&self, path: &Path) -> bool {
        let target = resolved(path);
        self.harnesses
            .values()
            .flat_map(|harness| harness.watch_paths())
            .any(|dir| target.starts_with(resolved(&dir)))
    }
}

/// `name` with every character outside `[A-Za-z0-9._-]` replaced by `-`.
pub fn sanitize_for_file_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// How many symlinks [`resolved`] follows before it gives up.
const MAX_LINK_HOPS: usize = 40;

/// `path` as an absolute path without `..` and symlinks, as far as it can be
/// told: the longest part of it that exists is canonicalised and the rest is
/// appended, with `.` and `..` resolved in the text. A symlink in that rest
/// (one that points to something that does not exist yet) is followed, so
/// that a link to a new file is the file it would create.
///
/// The answer is a snapshot: a link made after it is not seen.
fn resolved(path: &Path) -> PathBuf {
    resolved_in(path, MAX_LINK_HOPS)
}

fn resolved_in(path: &Path, hops: usize) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let components: Vec<Component<'_>> = absolute.components().collect();
    for split in (1..=components.len()).rev() {
        let prefix: PathBuf = components[..split].iter().collect();
        let Ok(mut real) = prefix.canonicalize() else {
            continue;
        };
        for (at, component) in components[split..].iter().enumerate() {
            match component {
                Component::ParentDir => {
                    real.pop();
                }
                Component::CurDir => {}
                other => real.push(other),
            }
            let Ok(target) = std::fs::read_link(&real) else {
                continue;
            };
            if hops == 0 {
                return real;
            }
            // A link that does not lead anywhere yet: what it names, then
            // what comes after it.
            let mut next = match real.parent() {
                Some(parent) => parent.join(target),
                None => target,
            };
            next.extend(components[split + at + 1..].iter());
            return resolved_in(&next, hops - 1);
        }
        return real;
    }
    absolute
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_sanitised_character_by_character() {
        assert_eq!(sanitize_for_file_name("My Project/x"), "My-Project-x");
        assert_eq!(sanitize_for_file_name("brio"), "brio");
        assert_eq!(sanitize_for_file_name("a.b_c-d9"), "a.b_c-d9");
        // One `-` per character, whatever its width in bytes.
        assert_eq!(sanitize_for_file_name("café ☕"), "caf---");
        assert_eq!(sanitize_for_file_name(""), "");
    }

    #[test]
    fn a_success_toast_keeps_the_end_of_a_long_path() {
        assert_eq!(WriteKind::Save.done("./a.txt"), "Saved to ./a.txt");
        let long = format!(
            "/home/user/{}/brio-85a86657-shell.txt",
            "directory/".repeat(8)
        );
        let toast = WriteKind::Export.done(&long);
        assert!(toast.starts_with("Exported to …"), "{toast}");
        assert!(toast.ends_with("/brio-85a86657-shell.txt"), "{toast}");
        assert_eq!(toast.width(), usize::from(MAX_TOAST_WIDTH - TOAST_CHROME));
    }

    #[test]
    fn a_path_is_resolved_through_its_existing_part() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().canonicalize().unwrap();
        std::fs::create_dir(real.join("a")).unwrap();
        assert_eq!(resolved(&real.join("a/new.txt")), real.join("a/new.txt"));
        assert_eq!(resolved(&real.join("a/../b.txt")), real.join("b.txt"));
        assert_eq!(
            resolved(&real.join("a/x/y/../../z.txt")),
            real.join("a/z.txt")
        );
        assert_eq!(resolved(&real.join("./a")), real.join("a"));
    }

    #[cfg(unix)]
    #[test]
    fn a_dangling_symlink_is_followed_to_where_it_points() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().canonicalize().unwrap();
        std::fs::create_dir(real.join("data")).unwrap();
        // To a file that does not exist, absolute and relative, and a chain.
        std::os::unix::fs::symlink(real.join("data/new.txt"), real.join("abs")).unwrap();
        std::os::unix::fs::symlink("data/new.txt", real.join("rel")).unwrap();
        std::os::unix::fs::symlink("abs", real.join("chain")).unwrap();
        for link in ["abs", "rel", "chain"] {
            assert_eq!(
                resolved(&real.join(link)),
                real.join("data/new.txt"),
                "{link}"
            );
        }
        // Through a link to a directory that does not exist, then on.
        std::os::unix::fs::symlink(real.join("data/later"), real.join("dir")).unwrap();
        assert_eq!(
            resolved(&real.join("dir/x.txt")),
            real.join("data/later/x.txt")
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_loop_of_symlinks_ends() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().canonicalize().unwrap();
        std::os::unix::fs::symlink(real.join("b"), real.join("a")).unwrap();
        std::os::unix::fs::symlink(real.join("a"), real.join("b")).unwrap();
        let _ = resolved(&real.join("a"));
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_is_followed() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().canonicalize().unwrap();
        std::fs::create_dir(real.join("target")).unwrap();
        std::os::unix::fs::symlink(real.join("target"), real.join("link")).unwrap();
        assert_eq!(
            resolved(&real.join("link/f.txt")),
            real.join("target/f.txt")
        );
    }
}
