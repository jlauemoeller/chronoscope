//! A synthetic session with many subagent transcripts, for the stats
//! benchmark (SPEC cycle 3 §3.5): the largest local session has 320 subagent
//! files of 207 MB in total.

use std::io;
use std::path::{Path, PathBuf};

use super::generate::{LARGE_PERCENT, Rng, SESSION_ID, generate_session_with};
use super::{Fixture, generate_fixture};

/// How many subagent files the benchmark session has.
pub const SUBAGENT_FILES: usize = 320;

/// The total size of the benchmark session's subagent files, in bytes. The
/// files come out slightly larger: each ends at the first exchange that
/// reaches its target.
pub const SUBAGENT_BYTES: u64 = 207_000_000;

/// The size of the benchmark session's own transcript.
pub const SUBAGENT_MAIN_BYTES: u64 = 4 * 1024 * 1024;

/// Bump when the output of [`generate_subagent_fixture`] changes, so cached
/// fixtures are rebuilt.
const VERSION: u32 = 1;

/// A generated session with subagent transcripts.
#[derive(Debug, Clone)]
pub struct SubagentFixture {
    /// The Claude data directory and the main session.
    pub fixture: Fixture,
    /// The subagent transcripts, in path order.
    pub files: Vec<PathBuf>,
    /// Their total size in bytes.
    pub bytes: u64,
}

/// `<session without extension>/subagents/agent-<n>.jsonl`, for each `n`.
fn subagent_paths(session: &Path, files: usize) -> Vec<PathBuf> {
    let dir = session.with_extension("").join("subagents");
    (0..files)
        .map(|n| dir.join(format!("agent-a{n:04}.jsonl")))
        .collect()
}

/// Writes a session of about `main_bytes` under `root` with `files` subagent
/// transcripts of about `subagent_bytes` bytes together. Their sizes vary
/// between half and one and a half times the mean, deterministically for
/// `seed`. Rebuilds what exists.
///
/// # Errors
/// Any error from writing a file.
pub fn generate_subagent_fixture(
    root: &Path,
    main_bytes: u64,
    files: usize,
    subagent_bytes: u64,
    seed: u64,
) -> io::Result<SubagentFixture> {
    let fixture = generate_fixture(root, main_bytes, seed, LARGE_PERCENT)?;
    let paths = subagent_paths(&fixture.session.path, files);
    let mean = subagent_bytes / files.max(1) as u64;
    let mut rng = Rng::new(seed ^ 0x5AB_A6E7);
    let mut bytes = 0;
    for (n, path) in paths.iter().enumerate() {
        let target = mean * (50 + rng.below(101) as u64) / 100;
        bytes += generate_session_with(path, target, seed + 1 + n as u64, LARGE_PERCENT)?.bytes;
    }
    Ok(SubagentFixture {
        fixture,
        files: paths,
        bytes,
    })
}

/// Like [`generate_subagent_fixture`], but reuses the files left by an
/// earlier run under `cache_dir` (the generator is deterministic). Meant for
/// `target/`.
///
/// # Errors
/// Any error from writing or reading a file.
pub fn cached_subagent_fixture(
    cache_dir: &Path,
    main_bytes: u64,
    files: usize,
    subagent_bytes: u64,
    seed: u64,
) -> io::Result<SubagentFixture> {
    let root = cache_dir.join(format!(
        "sub-v{VERSION}-{main_bytes}-{files}-{subagent_bytes}-{seed:x}"
    ));
    let marker = root.join("complete");
    if marker.exists() {
        let data_dir = root.join("projects");
        let main = data_dir
            .join("-home-user-p")
            .join(format!("{SESSION_ID}.jsonl"));
        let paths = subagent_paths(&main, files);
        let sizes: io::Result<Vec<u64>> = paths
            .iter()
            .map(|p| std::fs::metadata(p).map(|m| m.len()))
            .collect();
        if let (Ok(sizes), Ok(session)) = (sizes, super::scan_session(&main)) {
            return Ok(SubagentFixture {
                fixture: Fixture { data_dir, session },
                files: paths,
                bytes: sizes.iter().sum(),
            });
        }
    }
    let _ = std::fs::remove_dir_all(&root);
    let fixture = generate_subagent_fixture(&root, main_bytes, files, subagent_bytes, seed)?;
    std::fs::write(&marker, b"")?;
    Ok(fixture)
}
