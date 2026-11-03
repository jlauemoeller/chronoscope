//! Support for the large-session benchmarks and the budget test (SPEC §9.2,
//! §9.6): a deterministic generator of synthetic sessions, helpers that load a
//! session the way the application does and time it, a frame-rendering
//! scenario, and a counting allocator to measure the heap.
//!
//! Library code, so that `benches/` and `tests/` share it. Not part of the
//! public API.

mod alloc;
pub mod generate;
#[cfg(test)]
mod tests;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::{Rect, Size};
use tuirealm::command::Cmd;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};
use tuirealm::props::AttrValue;

pub use alloc::{CountingAlloc, HeapStats};
pub use generate::{
    LARGE_MAX, LARGE_MIN, LARGE_PERCENT, Rng, SESSION_ID, SyntheticSession, generate_session,
    generate_session_with,
};

use crate::app::Msg;
use crate::app::layout::browser_areas;
use crate::components::TurnSidebar;
use crate::components::transcript::{GO_TO_TURN_ATTR, Transcript, WHEEL_DOWN};
use crate::components::turn_block::{SectionId, SectionKey};
use crate::harness::Harness;
use crate::harness::claude::ClaudeHarness;
use crate::model::{Item, ModelUpdate, SessionId, Turn};
use crate::theme::builtin;
use crate::timestamp::{DEFAULT_TIMESTAMP_FORMAT, DisplayTz, TimestampFormatter};

/// The seed of the benchmark fixture.
pub const SEED: u64 = 0x5EED_0036;

/// Bump when the generator's output changes, so cached fixtures are rebuilt.
const GENERATOR_VERSION: u32 = 2;

/// The directory generated fixtures are cached in: `target/bench-fixtures`
/// (under `CARGO_TARGET_DIR` when it is set), which is never committed.
pub fn fixture_cache_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).join("target"),
            PathBuf::from,
        )
        .join("bench-fixtures")
}

/// The size of the benchmark fixture: 100 MiB.
pub const FIXTURE_BYTES: u64 = 100 * 1024 * 1024;

/// A generated session inside a Claude data directory.
#[derive(Debug, Clone)]
pub struct Fixture {
    /// The Claude data directory (`projects`) that holds the session.
    pub data_dir: PathBuf,
    /// What was generated.
    pub session: SyntheticSession,
}

impl Fixture {
    /// The harness that reads the fixture.
    pub fn harness(&self) -> ClaudeHarness {
        ClaudeHarness::with_data_dir(self.data_dir.clone(), Default::default())
    }

    /// The session's id.
    pub fn id(&self) -> SessionId {
        SessionId::from(self.session.id.as_str())
    }
}

/// Generates a session of about `bytes` bytes under `root` (rebuilding it
/// even when it exists).
///
/// # Errors
/// Any error from writing the file.
pub fn generate_fixture(
    root: &Path,
    bytes: u64,
    seed: u64,
    large_percent: usize,
) -> io::Result<Fixture> {
    let data_dir = root.join("projects");
    let path = data_dir
        .join("-home-user-p")
        .join(format!("{SESSION_ID}.jsonl"));
    let session = generate_session_with(&path, bytes, seed, large_percent)?;
    Ok(Fixture { data_dir, session })
}

/// Like [`generate_fixture`], but reuses the file left by an earlier run
/// under `cache_dir` (the generator is deterministic). Meant for `target/`.
///
/// # Errors
/// Any error from writing or reading the file.
pub fn cached_fixture(
    cache_dir: &Path,
    bytes: u64,
    seed: u64,
    large_percent: usize,
) -> io::Result<Fixture> {
    let root = cache_dir.join(format!(
        "v{GENERATOR_VERSION}-{bytes}-{seed:x}-l{large_percent}"
    ));
    let data_dir = root.join("projects");
    let path = data_dir
        .join("-home-user-p")
        .join(format!("{SESSION_ID}.jsonl"));
    let marker = root.join("complete");
    if marker.exists()
        && let Ok(meta) = std::fs::metadata(&path)
        && meta.len() >= bytes
    {
        let session = scan_session(&path)?;
        return Ok(Fixture { data_dir, session });
    }
    let _ = std::fs::remove_dir_all(&root);
    let fixture = generate_fixture(&root, bytes, seed, large_percent)?;
    std::fs::write(&marker, b"")?;
    Ok(fixture)
}

/// The statistics of an existing generated file, found by reading it.
fn scan_session(path: &Path) -> io::Result<SyntheticSession> {
    use std::io::BufRead;
    let file = std::fs::File::open(path)?;
    let mut session = SyntheticSession {
        path: path.to_path_buf(),
        id: SESSION_ID.to_owned(),
        bytes: file.metadata()?.len(),
        lines: 0,
        prompts: 0,
        tool_calls: 0,
        large_outputs: 0,
    };
    let mut reader = io::BufReader::with_capacity(1 << 20, file);
    let mut line = Vec::new();
    while reader.read_until(b'\n', &mut line)? > 0 {
        session.lines += 1;
        let has = |needle: &[u8]| line.windows(needle.len()).any(|w| w == needle);
        if has(br#""origin":{"kind":"human"}"#) {
            session.prompts += 1;
        }
        if has(br#""type":"tool_use""#) {
            session.tool_calls += 1;
        }
        if has(br#""type":"tool_result""#) && line.len() > crate::model::INLINE_OUTPUT_LIMIT * 4 {
            session.large_outputs += 1;
        }
        line.clear();
    }
    Ok(session)
}

/// When the pieces of a load arrived, measured from the call to `load()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadTimings {
    /// The first batch of turns (`Appended`).
    pub first_content: Duration,
    /// `Loaded`.
    pub loaded: Duration,
    /// How many turns the session has.
    pub turns: usize,
    /// How many entries could not be parsed.
    pub warnings: usize,
}

/// Loads session `id` with `harness` on its worker thread, passes every
/// update to `sink` as the UI thread would receive it, and returns when
/// `Loaded` arrived. The worker is stopped and awaited, so that the memory it
/// held is released before this returns.
///
/// # Panics
/// When the worker stops sending before `Loaded`, or does not end.
pub fn run_load(
    harness: &dyn Harness,
    id: &SessionId,
    mut sink: impl FnMut(&ModelUpdate),
) -> LoadTimings {
    let (tx, rx) = mpsc::channel();
    let start = Instant::now();
    let handle = harness.load(id, tx).expect("the loader starts");
    let mut first_content = None;
    let mut warnings = 0;
    let mut turns = 0;
    let loaded = loop {
        let update = rx.recv().expect("the worker sends Loaded before it ends");
        let now = start.elapsed();
        match &update {
            ModelUpdate::Appended(batch) => {
                first_content.get_or_insert(now);
                turns += batch.len();
            }
            ModelUpdate::ParseWarnings(n) => warnings = *n,
            _ => {}
        }
        let done = update == ModelUpdate::Loaded;
        sink(&update);
        if done {
            break now;
        }
    };
    stop_and_wait(&handle);
    drop(rx);
    LoadTimings {
        first_content: first_content.unwrap_or(loaded),
        loaded,
        turns,
        warnings,
    }
}

/// How long it takes from the call to `load()` until the first batch of turns
/// arrives. The worker is stopped and awaited afterwards, outside the
/// measured time.
///
/// # Panics
/// When the worker ends without sending turns, or does not end.
pub fn first_content(harness: &dyn Harness, id: &SessionId) -> Duration {
    let (tx, rx) = mpsc::channel();
    let start = Instant::now();
    let handle = harness.load(id, tx).expect("the loader starts");
    let took = loop {
        match rx.recv().expect("the worker sends turns") {
            ModelUpdate::Appended(_) => break start.elapsed(),
            ModelUpdate::Loaded => panic!("the session has no turns"),
            _ => {}
        }
    };
    stop_and_wait(&handle);
    took
}

fn stop_and_wait(handle: &crate::harness::WatchHandle) {
    handle.stop();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !handle.is_finished() {
        assert!(Instant::now() < deadline, "the loader did not stop");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// The default look: the built-in theme and the default timestamp format.
pub fn new_transcript() -> Transcript {
    let formatter = TimestampFormatter::new(DEFAULT_TIMESTAMP_FORMAT, true, DisplayTz::Local);
    Transcript::new(builtin::default_theme(), formatter)
}

/// Loads the fixture into a [`Transcript`] of `size`, applying each update
/// as the application forwards it, and returns the transcript with the
/// timings (which then include the time spent applying).
pub fn load_transcript(fixture: &Fixture, size: Size) -> (Transcript, LoadTimings) {
    let mut transcript = new_transcript().loading(true).viewport(size);
    let timings = run_load(&fixture.harness(), &fixture.id(), |update| {
        transcript.apply_update(update);
    });
    (transcript, timings)
}

/// Like [`load_transcript`], with the turns sidebar of the session built
/// alongside: each update goes to both, as the application forwards it, so
/// the timings include building the rows (SPEC §6.6).
pub fn load_with_sidebar(fixture: &Fixture, size: Size) -> (Transcript, TurnSidebar, LoadTimings) {
    let mut transcript = new_transcript().loading(true).viewport(size);
    let mut sidebar = TurnSidebar::new(builtin::default_theme());
    let timings = run_load(&fixture.harness(), &fixture.id(), |update| {
        transcript.apply_update(update);
        sidebar.apply_update(update);
    });
    (transcript, sidebar, timings)
}

/// Draws a loaded session's turns sidebar in a pane of `size`, moving the
/// selection one entry down before each frame, as a key press would.
pub struct SidebarScenario {
    /// The sidebar.
    pub sidebar: TurnSidebar,
    terminal: Terminal<TestBackend>,
    next: usize,
}

impl SidebarScenario {
    /// Prepares the scenario for a pane of `size`, with the selection in the
    /// middle of the list.
    pub fn new(mut sidebar: TurnSidebar, size: Size) -> Self {
        let entries = sidebar.rows().entry_count();
        let next = entries / 2;
        sidebar.attr(
            crate::components::VISIBLE_ROWS_ATTR,
            tuirealm::props::AttrValue::Length(crate::components::sidebar_list_rows(size.height)),
        );
        sidebar.select(Some(next));
        let terminal =
            Terminal::new(TestBackend::new(size.width, size.height)).expect("a test backend");
        Self {
            sidebar,
            terminal,
            next,
        }
    }

    /// Moves the selection to the next entry (to the first one after the
    /// last) and draws the frame; returns how long both took.
    pub fn select_next_and_draw(&mut self) -> Duration {
        let start = Instant::now();
        let entries = self.sidebar.rows().entry_count().max(1);
        self.next = (self.next + 1) % entries;
        self.sidebar.select(Some(self.next));
        self.draw();
        start.elapsed()
    }

    fn draw(&mut self) {
        let sidebar = &mut self.sidebar;
        self.terminal
            .draw(|frame| sidebar.view(frame, frame.area()))
            .expect("drawing to a TestBackend never fails");
    }
}

/// A key press in the turns sidebar beside the transcript, as the
/// application handles it: the pane's `on` selects the next or previous entry
/// and names its turn, the transcript goes to that turn's block (the same
/// scroll `]` / `[` give), and both are drawn (SPEC §6.4, §6.6). The screen is
/// that of [`FRAME_SCREEN`]: the pane and the transcript take the sizes
/// [`browser_areas`] gives them.
pub struct SidebarKeyScenario {
    /// The transcript the pane scrolls.
    pub transcript: Transcript,
    /// The pane with the focus.
    pub sidebar: TurnSidebar,
    transcript_terminal: Terminal<TestBackend>,
    sidebar_terminal: Terminal<TestBackend>,
}

/// The screen [`SidebarKeyScenario`] lays the pane and the transcript out in.
pub const FRAME_SCREEN: Size = Size::new(200, 50);

impl SidebarKeyScenario {
    /// Prepares the scenario for the `transcript` and `sidebar` of one loaded
    /// session, both at the first entry.
    pub fn new(mut transcript: Transcript, mut sidebar: TurnSidebar) -> Self {
        let areas = browser_areas(Rect::new(0, 0, FRAME_SCREEN.width, FRAME_SCREEN.height));
        let pane = areas.sidebar.expect("a screen wide enough").as_size();
        let body = areas.transcript.as_size();
        transcript.set_viewport(body);
        sidebar.attr(
            crate::components::VISIBLE_ROWS_ATTR,
            AttrValue::Length(crate::components::sidebar_list_rows(pane.height)),
        );
        sidebar.attr(tuirealm::props::Attribute::Focus, AttrValue::Flag(true));
        let sidebar_terminal =
            Terminal::new(TestBackend::new(pane.width, pane.height)).expect("a test backend");
        let transcript_terminal =
            Terminal::new(TestBackend::new(body.width, body.height)).expect("a test backend");
        let mut scenario = Self {
            transcript,
            sidebar,
            transcript_terminal,
            sidebar_terminal,
        };
        scenario.select(0);
        scenario
    }

    /// Selects entry `entry` and takes the transcript to its block. Not
    /// timed: the first jump far down lays out every block above it.
    pub fn select(&mut self, entry: usize) {
        self.sidebar.select(Some(entry));
        if let Some((id, _)) = self.sidebar.rows().entry(entry) {
            let id = id.clone();
            self.transcript
                .attr(GO_TO_TURN_ATTR, crate::components::payload_value(id));
        }
    }

    /// Selects the last entry, as `G` followed by the pane's focus would
    /// leave it. Not timed.
    pub fn to_end(&mut self) {
        let last = self.sidebar.rows().entry_count().saturating_sub(1);
        self.select(last);
    }

    /// Presses `j` (`down`) or `k`, hands the entry it selects to the
    /// transcript and draws both; returns how long that took. `None` at the
    /// end of the list, where the key does nothing.
    pub fn press_and_draw(&mut self, down: bool) -> Option<Duration> {
        let code = if down { Key::Char('j') } else { Key::Char('k') };
        let key = Event::Keyboard(KeyEvent::new(code, KeyModifiers::NONE));
        let start = Instant::now();
        let Some(Msg::TurnSelected(id)) = self.sidebar.on(&key) else {
            return None;
        };
        self.transcript
            .attr(GO_TO_TURN_ATTR, crate::components::payload_value(id));
        let (transcript, sidebar) = (&mut self.transcript, &mut self.sidebar);
        self.transcript_terminal
            .draw(|frame| transcript.view(frame, frame.area()))
            .expect("drawing to a TestBackend never fails");
        self.sidebar_terminal
            .draw(|frame| sidebar.view(frame, frame.area()))
            .expect("drawing to a TestBackend never fails");
        Some(start.elapsed())
    }

    /// The text of the transcript on screen, one line per row.
    pub fn transcript_screen(&self) -> String {
        crate::test_support::buffer_to_text(self.transcript_terminal.backend().buffer())
    }
}

/// A transcript at `size` showing the middle of a loaded session, ready to
/// be scrolled and drawn: some sections are unfolded, among them the first
/// large tool output at or after the middle (else the first one).
pub struct FrameScenario {
    /// The transcript.
    pub transcript: Transcript,
    terminal: Terminal<TestBackend>,
    /// The section of the large output that was unfolded.
    pub large_section: Option<SectionId>,
    /// Where the scrolling starts.
    start: usize,
}

impl FrameScenario {
    /// Prepares the scenario on a loaded `transcript` of the given size.
    pub fn new(mut transcript: Transcript, size: Size) -> Self {
        transcript.set_viewport(size);
        let middle = transcript.turn_count() / 2;
        let mut unfolded = 0;
        let mut large_section = None;
        // From the middle on, then from the top, for a session whose large
        // outputs are all before the middle.
        for index in (middle..transcript.turn_count()).chain(0..middle) {
            let Some(turn) = transcript.turn(index).cloned() else {
                break;
            };
            let Turn::Assistant(assistant) = turn.as_ref() else {
                continue;
            };
            for item in &assistant.items {
                let Item::ToolCall(call) = item else {
                    continue;
                };
                let large = call.result.as_ref().is_some_and(|r| r.output.is_lazy());
                if large && large_section.is_some() {
                    continue;
                }
                if !large && unfolded >= 12 {
                    continue;
                }
                let id = SectionId {
                    turn: assistant.id.clone(),
                    key: SectionKey::Item(call.id.clone()),
                };
                transcript.set_folded(&id, false);
                if large {
                    large_section = Some(id);
                } else {
                    unfolded += 1;
                }
            }
            if large_section.is_some() && unfolded >= 12 {
                break;
            }
        }
        if let Some(id) = &large_section {
            transcript.select(id);
            // A few rows above the output, so scrolling goes through it.
            let top = transcript.offset().saturating_sub(10);
            transcript.scroll_to(top);
        }
        let terminal =
            Terminal::new(TestBackend::new(size.width, size.height)).expect("a test backend");
        let start = transcript.offset();
        Self {
            transcript,
            terminal,
            large_section,
            start,
        }
    }

    /// Scrolls back to where the scenario started.
    pub fn rewind(&mut self) {
        self.transcript.scroll_to(self.start);
    }

    /// Scrolls one wheel step down and draws the frame; returns how long
    /// both took.
    pub fn scroll_and_draw(&mut self) -> Duration {
        let start = Instant::now();
        self.transcript.perform(Cmd::Custom(WHEEL_DOWN));
        self.draw();
        start.elapsed()
    }

    /// Draws the transcript once; returns how long it took.
    pub fn draw_once(&mut self) -> Duration {
        let start = Instant::now();
        self.draw();
        start.elapsed()
    }

    fn draw(&mut self) {
        let transcript = &mut self.transcript;
        self.terminal
            .draw(|frame| transcript.view(frame, frame.area()))
            .expect("drawing to a TestBackend never fails");
    }

    /// The text on screen, one line per row.
    pub fn screen(&self) -> String {
        crate::test_support::buffer_to_text(self.terminal.backend().buffer())
    }
}

/// Writes `count` small Claude session files into `data_dir`, spread over
/// ten project directories, for the selector index benchmark.
///
/// # Errors
/// Any error from writing the files.
pub fn write_small_sessions(data_dir: &Path, count: usize, seed: u64) -> io::Result<()> {
    let mut rng = Rng::new(seed);
    for n in 0..count {
        let dir = data_dir.join(format!("-home-user-project{}", n % 10));
        std::fs::create_dir_all(&dir)?;
        let id = format!("3f1b7c2e-0000-4000-8000-{n:012x}");
        let prompt = format!(
            "Please look at the {} of the project",
            rng.pick(generate::WORDS)
        );
        let mut text =
            format!(r#"{{"type":"ai-title","aiTitle":"Session {n}","sessionId":"{id}"}}"#);
        text.push('\n');
        for turn in 0..3 {
            let ts = format!("2026-10-21T08:{:02}:{:02}.000Z", n % 60, turn * 10);
            text.push_str(&format!(
                r#"{{"cwd":"/home/user/project{}","sessionId":"{id}","version":"2.1.306","uuid":"u-{n}-{turn}","timestamp":"{ts}","type":"user","origin":{{"kind":"human"}},"message":{{"role":"user","content":"{prompt}"}}}}"#,
                n % 10
            ));
            text.push('\n');
            text.push_str(&format!(
                r#"{{"cwd":"/home/user/project{}","sessionId":"{id}","version":"2.1.306","uuid":"a-{n}-{turn}","timestamp":"{ts}","type":"assistant","message":{{"model":"claude-opus-5-5","id":"msg_{n}_{turn}","role":"assistant","content":[{{"type":"text","text":"Looking."}}]}}}}"#,
                n % 10
            ));
            text.push('\n');
        }
        std::fs::write(dir.join(format!("{id}.jsonl")), text)?;
    }
    Ok(())
}

/// The turns of a loaded session, for tests that look at the model.
pub fn load_turns(fixture: &Fixture) -> (Vec<Arc<Turn>>, LoadTimings) {
    let mut turns = Vec::new();
    let timings = run_load(&fixture.harness(), &fixture.id(), |update| {
        update.apply(&mut turns);
    });
    (turns, timings)
}
