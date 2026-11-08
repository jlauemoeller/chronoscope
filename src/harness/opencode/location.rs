//! Where OpenCode's database lives (product §8.1).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The database's path below a data directory.
fn below(data_dir: &Path) -> PathBuf {
    data_dir.join("opencode").join("opencode.db")
}

/// The places the database may be, most likely first.
///
/// - `$XDG_DATA_HOME/opencode/opencode.db` when `XDG_DATA_HOME` is set and
///   non-empty, on every platform.
/// - On Windows, `%LOCALAPPDATA%\opencode\opencode.db`. **Unverified**: the
///   location on Windows must be confirmed before the tier-3 release (product
///   §10).
/// - `<home>/.local/share/opencode/opencode.db`: the default on Linux and,
///   by the same XDG-style convention, on macOS. **Unverified** on macOS
///   (product §10).
pub fn candidate_db_paths(
    home: &Path,
    xdg_data_home: Option<OsString>,
    local_app_data: Option<OsString>,
) -> Vec<PathBuf> {
    let set = |var: Option<OsString>| var.filter(|v| !v.is_empty()).map(PathBuf::from);
    let mut out = Vec::new();
    if let Some(xdg) = set(xdg_data_home) {
        out.push(below(&xdg));
    }
    if cfg!(windows)
        && let Some(local) = set(local_app_data)
    {
        out.push(below(&local));
    }
    out.push(below(&home.join(".local").join("share")));
    out.dedup();
    out
}

/// The first candidate that exists, else the first candidate (so that an
/// absent database still has a path to report and to watch).
pub fn default_db_path(
    home: &Path,
    xdg_data_home: Option<OsString>,
    local_app_data: Option<OsString>,
) -> PathBuf {
    let candidates = candidate_db_paths(home, xdg_data_home, local_app_data);
    candidates
        .iter()
        .find(|path| path.is_file())
        .or(candidates.first())
        .cloned()
        .unwrap_or_else(|| below(&home.join(".local").join("share")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_environment_the_default_is_under_home() {
        let home = Path::new("/home/u");
        assert_eq!(
            candidate_db_paths(home, None, None),
            [below(&Path::new("/home/u").join(".local").join("share"))]
        );
    }

    #[test]
    fn xdg_data_home_comes_first() {
        let home = Path::new("/home/u");
        let got = candidate_db_paths(home, Some("/data".into()), None);
        assert_eq!(
            got,
            [
                below(Path::new("/data")),
                below(&home.join(".local").join("share")),
            ]
        );
    }

    #[test]
    fn an_empty_variable_counts_as_unset() {
        let home = Path::new("/home/u");
        assert_eq!(
            candidate_db_paths(home, Some("".into()), Some("".into())).len(),
            1
        );
    }

    #[test]
    fn local_app_data_is_a_candidate_on_windows_only() {
        let home = Path::new("/home/u");
        let got = candidate_db_paths(home, None, Some("/local".into()));
        assert_eq!(got.len(), if cfg!(windows) { 2 } else { 1 });
        if cfg!(windows) {
            assert_eq!(got[0], below(Path::new("/local")));
        }
    }

    #[test]
    fn the_first_existing_candidate_wins() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let xdg = tmp.path().join("xdg");
        let in_home = below(&home.join(".local").join("share"));
        std::fs::create_dir_all(in_home.parent().unwrap()).unwrap();
        std::fs::write(&in_home, b"").unwrap();
        // XDG names a path that does not exist: the home one is used.
        assert_eq!(
            default_db_path(&home, Some(xdg.clone().into_os_string()), None),
            in_home
        );
        // Both exist: XDG comes first.
        let in_xdg = below(&xdg);
        std::fs::create_dir_all(in_xdg.parent().unwrap()).unwrap();
        std::fs::write(&in_xdg, b"").unwrap();
        assert_eq!(
            default_db_path(&home, Some(xdg.into_os_string()), None),
            in_xdg
        );
    }

    #[test]
    fn with_none_existing_the_first_candidate_is_reported() {
        let tmp = tempfile::tempdir().unwrap();
        let xdg = tmp.path().join("xdg");
        assert_eq!(
            default_db_path(tmp.path(), Some(xdg.clone().into_os_string()), None),
            below(&xdg)
        );
    }
}
