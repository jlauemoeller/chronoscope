//! The main loop (SPEC §1.1) and the startup sequence.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use tuirealm::terminal::TerminalAdapter;

use super::redraw::RedrawLimiter;
use super::resume::{SystemSteps, hand_off};
use super::startup::AppInit;
use super::{App, Msg, SearchWorker, TranscriptSearchWorker, production_listener, session_channel};
use crate::cli::Cli;
use crate::clipboard::SystemClipboard;
use crate::config::{Config, ConfigWarning};
use std::sync::Arc;

use crate::harness::{Harness, build_harnesses_with_home, registered_harnesses};
use crate::index::{IndexHandle, SessionIndex};
use crate::launch::{self, HarnessIndex, Request, Resolution, Target};
use crate::model::{HarnessId, SessionSummary};
use crate::paths::Paths;
use crate::terminal::{TerminalGuard, TerminalSetup, install_panic_hook, install_signal_handler};
use crate::theme::TerminalEnv;

/// Runs the loop until the application quits or `shutdown` is set.
///
/// Each iteration ticks the `Application`, passes every returned `Msg` to
/// [`App::update`], and draws if the [`RedrawLimiter`] allows it.
///
/// The terminal size is taken from `terminal` at the start, so the first
/// frame is laid out for it.
pub fn run_loop<T: TerminalAdapter>(
    app: &mut App,
    terminal: &mut T,
    shutdown: &AtomicBool,
) -> anyhow::Result<()> {
    let size = terminal
        .raw()
        .size()
        .map_err(|err| anyhow::anyhow!("cannot read the terminal size: {err}"))?;
    app.update(Msg::Resized(size.width, size.height));
    let mut limiter = RedrawLimiter::default();
    while !app.should_quit() && !shutdown.load(Ordering::SeqCst) {
        for msg in app.tick()? {
            app.update(msg);
        }
        limiter.request(app.take_redraw());
        let now = Instant::now();
        if limiter.should_draw(now) {
            terminal.draw(|frame| app.view(frame))?;
            limiter.record_draw(now);
        }
    }
    Ok(())
}

/// Tells the app what the command line resolved to. The harnesses and the
/// session channel are set already.
pub(super) fn start_with(app: &mut App, startup: Startup) {
    let Startup {
        resolution,
        index,
        open,
    } = startup;
    app.set_preselection(resolution.preselect);
    app.seed_index(&index);
    match (open, resolution.target) {
        (Some((harness, summary)), _) => app.open_summary(harness, summary),
        (None, Target::Filtered { prefix, matches }) => {
            if let Some(&(harness, _)) = matches.first() {
                app.show_filtered(&prefix, harness);
            }
        }
        (None, _) => {}
    }
}

/// The enabled harnesses. None without a home directory (the default data
/// directories are unknown).
fn build_harnesses(config: &Config, paths: Option<&Paths>) -> Vec<Arc<dyn Harness>> {
    paths.map_or_else(Vec::new, |paths| {
        build_harnesses_with_home(config, &paths.home_dir)
    })
}

/// Starts the session index over `harnesses`, caching stats in the cache
/// directory. `None` without a home directory (there is nowhere to cache).
fn start_index(harnesses: Vec<Arc<dyn Harness>>, paths: Option<&Paths>) -> Option<IndexHandle> {
    let paths = paths?;
    Some(SessionIndex::start(harnesses, paths.cache_dir.clone()))
}

/// What the command line asks the app to start with, resolved (SPEC §6.8).
pub(super) struct Startup {
    pub(super) resolution: Resolution,
    /// The lists the request was resolved against; empty when it did not
    /// need them.
    pub(super) index: Vec<HarnessIndex>,
    /// The session to open directly, if any.
    pub(super) open: Option<(HarnessId, SessionSummary)>,
}

/// Resolves the command line against the index, before the terminal is set
/// up, so that every error is reported on stderr.
pub(super) fn resolve_startup(
    request: &Request,
    working_dir: &std::path::Path,
    config: &Config,
    harnesses: &[Arc<dyn Harness>],
) -> anyhow::Result<Startup> {
    let index: Vec<HarnessIndex> = if request.needs_index() {
        harnesses
            .iter()
            .map(|harness| HarnessIndex::read(harness.as_ref()))
            .collect()
    } else {
        Vec::new()
    };
    let registered = registered_harnesses(config);
    let resolution = launch::resolve(request, working_dir, &registered, &index)?;
    let open = match &resolution.target {
        Target::Open(harness, summary) => Some((*harness, summary.clone())),
        Target::File(path) => {
            let claude = harnesses
                .iter()
                .find(|harness| harness.id() == HarnessId::Claude)
                .ok_or(launch::LaunchError::ClaudeDisabled)?;
            let summary = claude
                .open_file(path)
                .map_err(|err| anyhow::anyhow!("cannot read {}: {err:#}", path.display()))?;
            Some((HarnessId::Claude, summary))
        }
        Target::Selector | Target::Filtered { .. } => None,
    };
    Ok(Startup {
        resolution,
        index,
        open,
    })
}

/// The warnings of the command line as startup warnings, which the app shows
/// as a toast.
pub(super) fn launch_warnings(resolution: &Resolution) -> Vec<ConfigWarning> {
    resolution
        .warnings
        .iter()
        .inspect(|message| log::warn!("command line: {message}"))
        .map(|message| ConfigWarning {
            key: "--args".to_owned(),
            message: message.clone(),
        })
        .collect()
}

/// Starts the application: loads the configuration, resolves the command line,
/// sets up the terminal and runs the loop until the user quits. The terminal
/// is restored on every way out. An error in the command line is returned
/// before the terminal is touched.
pub fn run_app(cli: &Cli) -> anyhow::Result<()> {
    let paths = Paths::detect().map(|paths| match &cli.config {
        Some(config) => paths.with_config_file(config.clone()),
        None => paths,
    });
    if let Some(paths) = &paths {
        // Logging is optional; the application works without it.
        let _ = crate::logging::init_logging(&paths.log_file);
    }
    let mut init = AppInit::load_with_theme(
        paths.as_ref(),
        &TerminalEnv::from_env(),
        cli.theme.as_deref(),
    );
    let harnesses = build_harnesses(&init.config, paths.as_ref());
    let working_dir = std::env::current_dir().unwrap_or_else(|err| {
        log::warn!("could not read the working directory: {err}");
        PathBuf::new()
    });
    let startup = resolve_startup(&cli.request(), &working_dir, &init.config, &harnesses)?;
    init.warnings.extend(launch_warnings(&startup.resolution));
    let setup = TerminalSetup::from_mouse_config(init.config.mouse);

    let shutdown = match install_signal_handler() {
        Ok(flag) => flag,
        Err(err) => {
            log::warn!("could not install the signal handler: {err}");
            Default::default()
        }
    };
    install_panic_hook();
    let mut guard = TerminalGuard::enter(setup)?;
    let mut index = start_index(harnesses.clone(), paths.as_ref());
    let events = index.as_mut().and_then(IndexHandle::take_events);
    let (search_worker, search_results) = SearchWorker::start();
    let (transcript_worker, transcript_results) = TranscriptSearchWorker::start();
    let (session_sink, session_events) = session_channel();
    let listener = production_listener(
        events,
        Some(search_results),
        Some(transcript_results),
        Some(session_events),
    );
    let mut app = App::new(init, listener, working_dir)?;
    app.set_search_worker(search_worker);
    app.set_transcript_search_worker(transcript_worker);
    app.set_clipboard(
        Box::new(SystemClipboard::new()),
        Some(Box::new(std::io::stdout())),
    );
    app.set_harnesses(harnesses);
    app.set_session_sink(session_sink);
    start_with(&mut app, startup);
    if let Some(index) = index {
        app.set_index(index);
    }
    let result = run_loop(&mut app, guard.adapter_mut(), &shutdown);
    // The index saves its cache before the process ends.
    app.shutdown();
    let plan = app.take_resume_plan();
    // The event listener stops before the terminal is restored.
    drop(app);
    match plan {
        // Resume in the harness (SPEC §6.7): the terminal is restored first,
        // then this process is replaced. It does not return.
        Some(plan) if result.is_ok() => {
            let mut guard = Some(guard);
            let mut steps = SystemSteps::new(move || drop(guard.take()));
            std::process::exit(hand_off(&plan, &mut steps));
        }
        _ => {
            drop(guard);
            result
        }
    }
}
