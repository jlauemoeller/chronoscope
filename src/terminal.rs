//! Terminal setup and restore (product §8.5).
//!
//! The terminal is put into raw mode on the alternate screen (with mouse
//! capture only when the config asks for it) by a [`TerminalGuard`]. It is
//! restored when the guard is dropped, when a panic unwinds (a panic hook,
//! see [`install_panic_hook`]) and, for signals, through the normal exit path
//! of the main loop (see [`install_signal_handler`]).

use std::io::Write;
use std::panic::PanicHookInfo;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::cursor::Show;
use crossterm::event::DisableMouseCapture;
use crossterm::execute;
use crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};
use tuirealm::terminal::{CrosstermTerminalAdapter, TerminalAdapter, TerminalError};

/// What to switch on when the terminal is set up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSetup {
    /// Whether mouse capture is enabled.
    pub mouse_capture: bool,
}

impl TerminalSetup {
    /// The setup for the `mouse` config value: mouse capture is enabled only
    /// when `mouse = true`.
    pub fn from_mouse_config(mouse: bool) -> Self {
        Self {
            mouse_capture: mouse,
        }
    }
}

/// Whether the terminal modes have been entered and not yet restored.
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// Leaves everything [`TerminalGuard`] entered: mouse capture, the alternate
/// screen and raw mode, and shows the cursor.
///
/// It does nothing unless a guard is live, and only works once per guard, so
/// the guard's `Drop` and the panic hook can both call it.
pub fn restore_terminal() {
    if !ACTIVE.swap(false, Ordering::SeqCst) {
        return;
    }
    let mut stdout = std::io::stdout();
    // Restoring is best effort; there is nowhere to report a failure.
    let _ = execute!(stdout, DisableMouseCapture, LeaveAlternateScreen, Show);
    let _ = disable_raw_mode();
    let _ = stdout.flush();
}

/// Owns the terminal for the lifetime of the application and restores it on
/// `Drop`.
pub struct TerminalGuard {
    adapter: CrosstermTerminalAdapter,
}

impl TerminalGuard {
    /// Enters raw mode and the alternate screen, and mouse capture if `setup`
    /// asks for it.
    ///
    /// On failure the modes entered so far are left again before the error is
    /// returned.
    pub fn enter(setup: TerminalSetup) -> Result<Self, TerminalError> {
        let adapter = CrosstermTerminalAdapter::new()?;
        ACTIVE.store(true, Ordering::SeqCst);
        // From here on `Drop` restores whatever was entered.
        let mut guard = Self { adapter };
        guard.adapter.enable_raw_mode()?;
        guard.adapter.enter_alternate_screen()?;
        if setup.mouse_capture {
            guard.adapter.enable_mouse_capture()?;
        }
        Ok(guard)
    }

    /// The terminal to draw to.
    pub fn adapter_mut(&mut self) -> &mut CrosstermTerminalAdapter {
        &mut self.adapter
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = self.adapter.restore();
        restore_terminal();
    }
}

/// The hook that was installed before ours.
pub type PreviousHook = Box<dyn Fn(&PanicHookInfo<'_>) + Send + Sync + 'static>;

/// Installs a panic hook that calls `restore` and then `previous`, so the
/// panic message is printed on the restored terminal.
pub fn install_panic_hook_with<R>(restore: R, previous: PreviousHook)
where
    R: Fn() + Send + Sync + 'static,
{
    std::panic::set_hook(Box::new(move |info| {
        restore();
        previous(info);
    }));
}

/// Installs the panic hook that restores the terminal ([`restore_terminal`])
/// and then runs the hook that was installed before.
pub fn install_panic_hook() {
    install_panic_hook_with(restore_terminal, std::panic::take_hook());
}

/// Arranges for SIGINT, SIGTERM and SIGHUP (Ctrl-C and Ctrl-Break on Windows)
/// to set the returned flag. The main loop checks it every iteration and
/// leaves through its normal path, which restores the terminal.
///
/// In raw mode `Ctrl-c` is a key press, not a signal; it is handled by
/// `GlobalEvents`. Can only be called once per process.
pub fn install_signal_handler() -> Result<Arc<AtomicBool>, ctrlc::Error> {
    let flag = Arc::new(AtomicBool::new(false));
    let handler_flag = Arc::clone(&flag);
    ctrlc::set_handler(move || handler_flag.store(true, Ordering::SeqCst))?;
    Ok(flag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_capture_follows_the_mouse_config() {
        assert!(TerminalSetup::from_mouse_config(true).mouse_capture);
        assert!(!TerminalSetup::from_mouse_config(false).mouse_capture);
    }
}
