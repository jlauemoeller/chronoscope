//! Saving a section and exporting the session to files (SPEC §6.4, §9.5).
//!
//! 1. `w` / `W` in the transcript answer [`Msg::SaveSection`] with the text,
//!    `e` / `E` answer [`Msg::Export`] with the markdown document.
//! 2. The `App` opens the [`Prompt`] in the help line, labelled `Save to: `
//!    or `Export to: `, with the default path filled in and the cursor at its
//!    end. The text waits in the browser's [`PendingWrite`].
//! 3. `Enter` ([`Msg::PromptSubmit`]) names the path. A path inside the data
//!    of a harness is refused. A path that exists is asked about with the
//!    `Overwrite <path>? (y/n)` prompt; `y` writes and any other key cancels.
//!    A path that does not exist is written at once.
//! 4. `Esc` ([`Msg::PromptCancel`]) cancels. A toast reports the result.
//!
//! Paths are relative to the directory the application was started in, and
//! `~` is the home directory. No directory is created.

use std::path::{Component, Path, PathBuf};

use super::layout::Screen;
use super::{App, Id};
use crate::components::{Prompt, PromptMode};
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

    fn done(self, path: &str) -> String {
        match self {
            WriteKind::Save => format!("Saved to {path}"),
            WriteKind::Export => format!("Exported to {path}"),
        }
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
    pub fn set_save_dir(&mut self, dir: PathBuf) {
        self.save_dir = dir;
    }

    /// Sets the home directory `~` stands for (`None`: `~` is not expanded).
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
        let id = self
            .browser
            .open
            .as_ref()
            .map_or("", |open| open.summary.id.as_str());
        sanitize_for_file_name(&id.chars().take(8).collect::<String>())
    }

    /// Opens the path prompt with `default` filled in.
    fn ask_path(&mut self, kind: WriteKind, text: String, default: String) {
        if self.screen != Screen::Browser
            || !self.application.mounted(&Id::Transcript)
            || self.application.mounted(&Id::Prompt)
        {
            return;
        }
        let prompt = Prompt::new(kind.label(), default, PromptMode::Text, self.theme.clone())
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

    /// `Enter` in the path prompt (`answer` is the path), or `y` in the
    /// overwrite prompt.
    pub(super) fn write_prompt_submit(&mut self, answer: &str) {
        let Some(pending) = self.browser.write.take() else {
            return;
        };
        self.close_overlay(Id::Prompt);
        match pending.overwrite {
            Some(overwrite) => {
                if answer == "y" {
                    self.write_file(
                        pending.kind,
                        &pending.text,
                        &overwrite.path,
                        &overwrite.shown,
                    );
                }
            }
            None => self.path_chosen(pending.kind, pending.text, answer),
        }
    }

    /// `Esc` in the path prompt, or a no in the overwrite prompt: nothing is
    /// written.
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
        let prompt = Prompt::confirm("Overwrite ", &shown, "? (y/n)", self.theme.clone())
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
    /// (SPEC §9.5: chronoscope never writes there).
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

/// `path` as an absolute path without `..` and symlinks, as far as it can be
/// told: the longest part of it that exists is canonicalised and the rest is
/// appended, with `.` and `..` resolved in the text.
fn resolved(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let components: Vec<Component<'_>> = absolute.components().collect();
    for split in (1..=components.len()).rev() {
        let prefix: PathBuf = components[..split].iter().collect();
        let Ok(mut real) = prefix.canonicalize() else {
            continue;
        };
        for component in &components[split..] {
            match component {
                Component::ParentDir => {
                    real.pop();
                }
                Component::CurDir => {}
                other => real.push(other),
            }
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
