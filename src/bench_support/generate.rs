//! A deterministic generator of synthetic Claude sessions of a requested
//! size, for the §9.2 benchmarks and their tests.
//!
//! The mix is meant to look like a long real session: human prompts (some of
//! several lines), assistant messages split over several lines the way Claude
//! Code writes them (a partial text line, then the completed one), readable
//! and redacted thinking, `Bash`/`Read`/`Edit`/`Grep` calls with results,
//! about one result in twenty a large one (100 KB to 2 MB, so stored lazily),
//! `turn_duration` entries and some injected attachments.
//!
//! The same seed and size always produce the same bytes.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, SecondsFormat, Utc};

/// The id of every generated session (the file stem).
pub const SESSION_ID: &str = "3f1b7c2e-0000-4000-8000-000000000036";

/// The share of tool results that are large, in percent.
pub const LARGE_PERCENT: usize = 5;

/// The smallest large output, in bytes.
pub const LARGE_MIN: usize = 100 * 1024;
/// The biggest large output, in bytes.
pub const LARGE_MAX: usize = 2 * 1024 * 1024;

/// What was generated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticSession {
    /// The transcript file.
    pub path: PathBuf,
    /// The session id (the file stem).
    pub id: String,
    /// The size of the file in bytes.
    pub bytes: u64,
    /// The number of JSONL lines.
    pub lines: usize,
    /// The number of human prompts.
    pub prompts: usize,
    /// The number of tool calls.
    pub tool_calls: usize,
    /// How many of the tool results are large (stored lazily).
    pub large_outputs: usize,
}

/// A small deterministic random number generator (SplitMix64).
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    /// A generator that always yields the same sequence for `seed`.
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `0..n` (`n` must not be 0).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    /// A number in `lo..=hi`.
    pub fn between(&mut self, lo: usize, hi: usize) -> usize {
        lo + self.below(hi - lo + 1)
    }

    /// True in `percent` percent of the calls.
    pub fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    /// One of `items`.
    pub fn pick<'a, T: ?Sized>(&mut self, items: &[&'a T]) -> &'a T {
        items[self.below(items.len())]
    }
}

pub(crate) const WORDS: &[&str] = &[
    "parser",
    "layout",
    "viewport",
    "cache",
    "block",
    "section",
    "render",
    "stream",
    "buffer",
    "offset",
    "session",
    "transcript",
    "harness",
    "theme",
    "fold",
    "width",
    "height",
    "markdown",
    "token",
    "update",
    "channel",
    "worker",
    "index",
    "stats",
    "query",
    "result",
    "error",
    "config",
    "keymap",
    "scroll",
];

const FILES: &[&str] = &[
    "src/lib.rs",
    "src/main.rs",
    "src/app/mod.rs",
    "src/model/mod.rs",
    "src/components/transcript/mod.rs",
    "src/harness/claude/assemble.rs",
    "src/markdown/prose.rs",
    "src/theme/resolve.rs",
    "tests/claude_loading.rs",
    "Cargo.toml",
];

const COMMANDS: &[&str] = &[
    "cargo test --lib",
    "cargo build --release",
    "git status --short",
    "git log --oneline -n 20",
    "rg -n 'TODO' src",
    "ls -la src",
    "cargo clippy --all-targets",
    "cat Cargo.toml",
];

const SENTENCES: &[&str] = &[
    "I'll start by looking at how the module is organised.",
    "The failing case comes from the width calculation, so I'll read that next.",
    "That explains the `offset` drift: it is clamped before the layout is known.",
    "Here is the plan:\n\n1. read the caller\n2. fix the clamp\n3. add a regression test",
    "The change is small, but it touches a hot path, so I'll run the whole suite.",
    "```rust\nfn clamp(offset: usize, max: usize) -> usize {\n    offset.min(max)\n}\n```",
    "Done. The tests pass and the behaviour matches the description.",
    "Note that the result contains `\"quotes\"`, tabs\tand unicode: héllo → wörld.",
];

/// Writes a Claude-format session of about `target_bytes` to `path` and
/// returns what it holds. The file ends at the first exchange that reaches
/// `target_bytes`, so it is never smaller than the target and exceeds it by
/// less than one exchange (large outputs are only written when they fit).
///
/// # Errors
/// Any error from creating or writing the file.
pub fn generate_session(path: &Path, target_bytes: u64, seed: u64) -> io::Result<SyntheticSession> {
    generate_session_with(path, target_bytes, seed, LARGE_PERCENT)
}

/// Like [`generate_session`], with `large_percent` percent of the tool
/// results large (0 makes a session of many small entries only).
///
/// # Errors
/// Any error from creating or writing the file.
pub fn generate_session_with(
    path: &Path,
    target_bytes: u64,
    seed: u64,
    large_percent: usize,
) -> io::Result<SyntheticSession> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let file = File::create(path)?;
    let mut gen_ = Generator::new(
        BufWriter::with_capacity(1 << 20, file),
        target_bytes,
        seed,
        large_percent,
    );
    gen_.run()?;
    gen_.out.flush()?;
    let id = path.file_stem().map_or_else(
        || SESSION_ID.to_owned(),
        |s| s.to_string_lossy().into_owned(),
    );
    Ok(SyntheticSession {
        path: path.to_path_buf(),
        id,
        bytes: gen_.bytes,
        lines: gen_.lines,
        prompts: gen_.prompts,
        tool_calls: gen_.tool_calls,
        large_outputs: gen_.large_outputs,
    })
}

struct Generator<W: Write> {
    out: W,
    rng: Rng,
    target: u64,
    large_percent: usize,
    bytes: u64,
    lines: usize,
    prompts: usize,
    tool_calls: usize,
    large_outputs: usize,
    /// Counter behind uuids, message ids and timestamps.
    seq: u64,
    base: DateTime<Utc>,
}

impl<W: Write> Generator<W> {
    fn new(out: W, target: u64, seed: u64, large_percent: usize) -> Self {
        Self {
            out,
            rng: Rng::new(seed),
            target,
            large_percent,
            bytes: 0,
            lines: 0,
            prompts: 0,
            tool_calls: 0,
            large_outputs: 0,
            seq: 0,
            base: DateTime::parse_from_rfc3339("2026-10-21T08:00:00Z")
                .expect("a constant date")
                .with_timezone(&Utc),
        }
    }

    fn run(&mut self) -> io::Result<()> {
        self.raw(&format!(
            r#"{{"type":"mode","mode":"normal","sessionId":"{SESSION_ID}"}}"#
        ))?;
        self.raw(&format!(
            r#"{{"type":"ai-title","aiTitle":"Synthetic large session","sessionId":"{SESSION_ID}"}}"#
        ))?;
        while self.bytes < self.target {
            self.exchange()?;
        }
        Ok(())
    }

    /// One user prompt and the assistant turn that answers it.
    fn exchange(&mut self) -> io::Result<()> {
        if self.rng.chance(10) {
            self.attachment()?;
        }
        self.prompt()?;
        let messages = self.rng.between(1, 5);
        let mut count = 1;
        for n in 0..messages {
            count += self.assistant_message(n)?;
            if self.bytes >= self.target {
                break;
            }
        }
        let ms = self.rng.between(2_000, 120_000);
        let entry = self.envelope(
            "system",
            &format!(r#""subtype":"turn_duration","durationMs":{ms},"messageCount":{count},"isMeta":false"#),
        );
        self.raw(&entry)
    }

    fn prompt(&mut self) -> io::Result<()> {
        self.prompts += 1;
        let mut text = self.sentence();
        if self.rng.chance(30) {
            text.push_str("\n\nAlso check ");
            text.push_str(self.rng.pick(FILES));
            text.push_str(" and the ");
            text.push_str(self.rng.pick(WORDS));
            text.push_str(" handling.\nThanks.");
        }
        let body = format!(
            r#""origin":{{"kind":"human"}},"message":{{"role":"user","content":{}}}"#,
            json(&text)
        );
        let entry = self.envelope("user", &body);
        self.raw(&entry)
    }

    /// An attachment: an injected one, or noise.
    fn attachment(&mut self) -> io::Result<()> {
        let body = match self.rng.below(3) {
            0 => format!(
                r#""attachment":{{"type":"opened_file_in_ide","filename":"/home/user/project/{}"}}"#,
                self.rng.pick(FILES)
            ),
            1 => {
                let content = format!("hook output about the {}", self.rng.pick(WORDS));
                format!(
                    r#""attachment":{{"type":"hook_system_message","content":{}}}"#,
                    json(&content)
                )
            }
            _ => r#""attachment":{"type":"environment"}"#.to_owned(),
        };
        let entry = self.envelope("attachment", &body);
        self.raw(&entry)
    }

    /// One assistant message (written over several lines) and the results of
    /// its tool calls. Returns how many entries of the turn it wrote.
    fn assistant_message(&mut self, n: usize) -> io::Result<usize> {
        let msg = format!("msg_{:012x}", self.seq + 1);
        let usage = format!(
            r#""usage":{{"input_tokens":{},"output_tokens":{},"cache_creation_input_tokens":50000,"cache_read_input_tokens":90000}}"#,
            self.rng.between(10, 5_000),
            self.rng.between(10, 3_000)
        );
        let mut written = 0;
        let mut block = 0;
        let mut calls: Vec<(String, usize)> = Vec::new();

        if self.rng.chance(60) {
            let thinking = if self.rng.chance(50) {
                self.sentence()
            } else {
                String::new()
            };
            self.assistant_line(
                &msg,
                &usage,
                block,
                &format!(
                    r#"{{"type":"thinking","thinking":{},"signature":"c2lnbmF0dXJl"}}"#,
                    json(&thinking)
                ),
            )?;
            block += 1;
            written += 1;
        }
        // The text arrives in growing pieces, each on its own line.
        let full = self.sentence();
        let cut = full
            .char_indices()
            .map(|(i, _)| i)
            .nth(full.chars().count() / 2)
            .unwrap_or(0);
        for piece in [&full[..cut], &full[..]] {
            if piece.is_empty() {
                continue;
            }
            self.assistant_line(
                &msg,
                &usage,
                block,
                &format!(r#"{{"type":"text","text":{}}}"#, json(piece)),
            )?;
            written += 1;
        }
        block += 1;

        let tools = if n == 0 {
            self.rng.between(1, 4)
        } else {
            self.rng.between(0, 3)
        };
        for _ in 0..tools {
            let (call_id, kind) = self.tool_use(&msg, &usage, block)?;
            calls.push((call_id, kind));
            block += 1;
            written += 1;
        }
        for (call_id, kind) in calls {
            self.tool_result(&call_id, kind)?;
            written += 1;
            if self.bytes >= self.target {
                break;
            }
        }
        Ok(written)
    }

    fn assistant_line(
        &mut self,
        msg: &str,
        usage: &str,
        block: usize,
        content: &str,
    ) -> io::Result<()> {
        let body = format!(
            r#""message":{{"model":"claude-opus-5-5","id":"{msg}","type":"message","role":"assistant","content":[{content}],"stop_reason":null,{usage}}},"apiBlockIndex":{block},"effort":"high""#
        );
        let entry = self.envelope("assistant", &body);
        self.raw(&entry)
    }

    /// Writes a tool call; returns its id and kind (0 Bash, 1 Read, 2 Edit,
    /// 3 Grep).
    fn tool_use(&mut self, msg: &str, usage: &str, block: usize) -> io::Result<(String, usize)> {
        self.tool_calls += 1;
        let kind = self.rng.below(4);
        let id = format!("toolu_{:012x}", self.seq + 1);
        let file = self.rng.pick(FILES);
        let (name, input) = match kind {
            0 => (
                "Bash",
                format!(
                    r#"{{"command":{},"description":"Run a command"}}"#,
                    json(self.rng.pick(COMMANDS))
                ),
            ),
            1 => (
                "Read",
                format!(r#"{{"file_path":"/home/user/project/{file}"}}"#),
            ),
            2 => {
                let old = format!("let {} = {};", self.rng.pick(WORDS), self.rng.below(100));
                let new = format!(
                    "let {} = {};\nlet {} = {};",
                    self.rng.pick(WORDS),
                    self.rng.below(100),
                    self.rng.pick(WORDS),
                    self.rng.below(100)
                );
                (
                    "Edit",
                    format!(
                        r#"{{"file_path":"/home/user/project/{file}","old_string":{},"new_string":{}}}"#,
                        json(&old),
                        json(&new)
                    ),
                )
            }
            _ => (
                "Grep",
                format!(
                    r#"{{"pattern":{},"path":"src"}}"#,
                    json(self.rng.pick(WORDS))
                ),
            ),
        };
        self.assistant_line(
            msg,
            usage,
            block,
            &format!(
                r#"{{"type":"tool_use","id":"{id}","name":"{name}","input":{input},"caller":{{"type":"direct"}}}}"#
            ),
        )?;
        Ok((id, kind))
    }

    /// Writes the result of a tool call: small, or (about one in twenty)
    /// large when it still fits in the target.
    fn tool_result(&mut self, call_id: &str, kind: usize) -> io::Result<()> {
        let room = self.target.saturating_sub(self.bytes) as usize;
        let large_size = self.rng.between(LARGE_MIN, LARGE_MAX);
        let large = self.rng.chance(self.large_percent) && large_size + 1024 <= room;
        let size = if large {
            self.large_outputs += 1;
            large_size
        } else {
            self.rng.between(40, 3_000)
        };
        let text = self.output_text(kind, size);
        let is_error = !large && self.rng.chance(3);
        // The text goes through the writer escaped, straight from a string:
        // the entry is assembled around it.
        let head = self.envelope_head("user");
        let error = if is_error { r#""is_error":true,"# } else { "" };
        // Claude Code repeats the output in `toolUseResult`: an object for a
        // call that succeeded, the error message as a string for one that
        // failed. The assembler reads only `isAsync` from it, but has to get
        // past all of it.
        let result = if is_error {
            json(&text)
        } else {
            tool_use_result(kind, &text)
        };
        let line = format!(
            r#"{head},"message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"{call_id}",{error}"content":{}}}]}},"toolUseResult":{result}}}"#,
            json(&text)
        );
        self.raw(&line)
    }

    /// Text that looks like the output of the tool, about `size` bytes long.
    fn output_text(&mut self, kind: usize, size: usize) -> String {
        let mut text = String::with_capacity(size + 128);
        let mut n = 1;
        while text.len() < size {
            use std::fmt::Write as _;
            let word = self.rng.pick(WORDS);
            let other = self.rng.pick(WORDS);
            let value = self.rng.below(10_000);
            match kind {
                // Read: numbered source lines.
                1 => writeln!(
                    text,
                    "{n:>5}\tfn {word}_{n}(&mut self, {other}: usize) -> usize {{ self.{word} + {value} }} // \"{other}\" → ok"
                ),
                // Grep: path:line:match.
                3 => writeln!(
                    text,
                    "{}:{}:    let {word} = {other}.{word}({value}); // héllo",
                    self.rng.pick(FILES),
                    self.rng.below(2_000)
                ),
                // Edit: a confirmation, then context.
                2 => writeln!(
                    text,
                    "The file has been updated. Here's the result of running `cat -n`:\n{n:>5}\t    let {word} = {value};"
                ),
                // Bash: build or listing output.
                _ => writeln!(
                    text,
                    "   Compiling {word}-{other} v0.{value}.0 (/home/user/project/crates/{word})"
                ),
            }
            .expect("writing to a String never fails");
            n += 1;
        }
        text.truncate(text.trim_end().len());
        text
    }

    fn sentence(&mut self) -> String {
        let mut text = self.rng.pick(SENTENCES).to_owned();
        if self.rng.chance(40) {
            text.push(' ');
            text.push_str(self.rng.pick(SENTENCES));
        }
        text
    }

    /// The fields every conversation entry starts with, up to the type.
    fn envelope_head(&mut self, kind: &str) -> String {
        self.seq += 1;
        let ts = (self.base + chrono::Duration::milliseconds(self.seq as i64 * 700))
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        format!(
            r#"{{"isSidechain":false,"userType":"external","entrypoint":"cli","cwd":"/home/user/project","sessionId":"{SESSION_ID}","version":"2.1.306","gitBranch":"main","uuid":"{:08x}-0000-4000-8000-{:012x}","timestamp":"{ts}","type":"{kind}""#,
            0x3600_0000_u32, self.seq
        )
    }

    /// A whole entry: the head and `rest` (fields, without a leading comma).
    fn envelope(&mut self, kind: &str, rest: &str) -> String {
        let head = self.envelope_head(kind);
        format!("{head},{rest}}}")
    }

    /// Writes one line.
    fn raw(&mut self, line: &str) -> io::Result<()> {
        self.out.write_all(line.as_bytes())?;
        self.out.write_all(b"\n")?;
        self.bytes += line.len() as u64 + 1;
        self.lines += 1;
        Ok(())
    }
}

/// The `toolUseResult` object of a call that succeeded, in the shape Claude
/// Code writes it for the tool (0 Bash, 1 Read, 2 Edit, 3 Grep): the output
/// again, in the fields the tool uses.
fn tool_use_result(kind: usize, text: &str) -> String {
    let lines = text.lines().count();
    let text = json(text);
    match kind {
        0 => format!(
            r#"{{"stdout":{text},"stderr":"","interrupted":false,"isImage":false,"noOutputExpected":false}}"#
        ),
        1 => format!(
            r#"{{"type":"text","file":{{"filePath":"/home/user/project/src/lib.rs","content":{text},"numLines":{lines},"startLine":1,"totalLines":{lines}}}}}"#
        ),
        2 => format!(
            r#"{{"filePath":"/home/user/project/src/lib.rs","oldString":"","newString":"","originalFile":{text},"structuredPatch":[],"userModified":false,"replaceAll":false}}"#
        ),
        _ => format!(
            r#"{{"mode":"content","numFiles":0,"filenames":[],"content":{text},"numLines":{lines}}}"#
        ),
    }
}

/// `text` as a JSON string literal.
fn json(text: &str) -> String {
    serde_json::to_string(text).expect("a string always serialises")
}
