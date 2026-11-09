//! The one place that uses an operating system's own API (coding.md,
//! Portability, as amended by the cycle 5 spec, §0 item 4).
//!
//! Each function has a Unix version and a portable fallback for the other
//! platforms. Nothing else in the production code uses `std::os`.

use std::fs::Metadata;
use std::process::Command;

/// What tells two files at one path apart: see [`file_identity`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FileIdentity {
    #[cfg(unix)]
    id: (u64, u64),
    #[cfg(not(unix))]
    created: Option<std::time::SystemTime>,
}

/// The identity of the file `meta` describes: its device and inode on Unix.
///
/// On other platforms the fallback is the file's creation time (`None` when
/// the platform does not record it), so a file replaced by another is told
/// apart only when their creation times differ.
pub(crate) fn file_identity(meta: &Metadata) -> FileIdentity {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        FileIdentity {
            id: (meta.dev(), meta.ino()),
        }
    }
    #[cfg(not(unix))]
    {
        FileIdentity {
            created: meta.created().ok(),
        }
    }
}

/// Runs `command` in place of the process: `exec` on Unix, which only
/// returns when the program could not be started, so the result is then
/// always an error.
///
/// On other platforms the fallback runs the program as a child, waits for it
/// (`status()`) and returns its exit code, or 1 when it has none (it was
/// killed by a signal).
///
/// The program and its arguments go straight to the operating system, with
/// no shell. Buffered output is the caller's to flush first.
pub(crate) fn exec_or_run(mut command: Command) -> std::io::Result<i32> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(command.exec())
    }
    #[cfg(not(unix))]
    {
        let status = command.status()?;
        Ok(status.code().unwrap_or(1))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn two_hard_links_to_one_file_share_its_identity() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("a.jsonl");
        let link = tmp.path().join("b.jsonl");
        fs::write(&file, "{}\n").unwrap();
        fs::hard_link(&file, &link).unwrap();
        assert_eq!(
            file_identity(&fs::metadata(&file).unwrap()),
            file_identity(&fs::metadata(&link).unwrap())
        );
    }

    #[test]
    fn a_file_replaced_at_its_path_has_another_identity() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("a.jsonl");
        fs::write(&file, "{}\n").unwrap();
        let before = file_identity(&fs::metadata(&file).unwrap());
        // The new file exists while the old one does, so the two cannot
        // share an inode; it then takes the old one's place.
        let new = tmp.path().join("a.jsonl.new");
        fs::write(&new, "{}\n").unwrap();
        fs::rename(&new, &file).unwrap();
        let after = file_identity(&fs::metadata(&file).unwrap());
        assert_ne!(before, after);
    }

    #[test]
    fn the_identity_of_an_unchanged_file_stays_the_same() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("a.jsonl");
        fs::write(&file, "{}\n").unwrap();
        let before = file_identity(&fs::metadata(&file).unwrap());
        fs::write(&file, "{}\n{}\n").unwrap();
        assert_eq!(before, file_identity(&fs::metadata(&file).unwrap()));
    }

    #[test]
    fn a_program_that_does_not_exist_is_an_error_and_the_process_goes_on() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("no-such-program");
        let mut command = Command::new(&missing);
        command.arg("--version");
        let err = exec_or_run(command).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound, "{err}");
    }
}
