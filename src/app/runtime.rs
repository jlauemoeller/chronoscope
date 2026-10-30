//! The main loop (SPEC §1.1) and the startup sequence.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use tuirealm::terminal::TerminalAdapter;

use super::redraw::RedrawLimiter;
use super::startup::AppInit;
use super::{App, Msg, production_listener};
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
    let mut app = App::new(init, production_listener())?;
    let result = run_loop(&mut app, guard.adapter_mut(), &shutdown);
    // The event listener stops before the terminal is restored.
    drop(app);
    drop(guard);
    result
}
