//! The main loop (SPEC §1.1) and the startup sequence.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use tuirealm::terminal::TerminalAdapter;

use super::redraw::RedrawLimiter;
use super::startup::AppInit;
use super::{App, Msg, SearchWorker, production_listener};
use crate::config::Config;
use crate::harness::build_harnesses_with_home;
use crate::index::{IndexHandle, SessionIndex};
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

/// Starts the session index over the enabled harnesses, caching stats in the
/// cache directory. `None` without a home directory (there is nowhere to
/// cache, and the default data directories are unknown).
fn start_index(config: &Config, paths: Option<&Paths>) -> Option<IndexHandle> {
    let paths = paths?;
    let harnesses = build_harnesses_with_home(config, &paths.home_dir);
    Some(SessionIndex::start(harnesses, paths.cache_dir.clone()))
}

/// Starts the application: loads the configuration, sets up the terminal and
/// runs the loop until the user quits. The terminal is restored on every way
/// out.
pub fn run_app() -> anyhow::Result<()> {
    let paths = Paths::detect();
    if let Some(paths) = &paths {
        // Logging is optional; the application works without it.
        let _ = crate::logging::init_logging(&paths.log_file);
    }
    let init = AppInit::load(paths.as_ref(), &TerminalEnv::from_env());
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
    let mut index = start_index(&init.config, paths.as_ref());
    let events = index.as_mut().and_then(IndexHandle::take_events);
    let working_dir = std::env::current_dir().unwrap_or_else(|err| {
        log::warn!("could not read the working directory: {err}");
        PathBuf::new()
    });
    let (search_worker, search_results) = SearchWorker::start();
    let listener = production_listener(events, Some(search_results));
    let mut app = App::new(init, listener, working_dir)?;
    app.set_search_worker(search_worker);
    if let Some(index) = index {
        app.set_index(index);
    }
    let result = run_loop(&mut app, guard.adapter_mut(), &shutdown);
    // The index saves its cache before the process ends.
    app.shutdown();
    // The event listener stops before the terminal is restored.
    drop(app);
    drop(guard);
    result
}
