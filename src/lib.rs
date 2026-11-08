//! chronoscope: a terminal viewer for AI coding-agent session histories.
//!
//! All application code lives in this library; the binary only calls [`run`].

pub mod app;
#[doc(hidden)]
pub mod bench_support;
pub mod cli;
pub mod clipboard;
pub mod components;
pub mod config;
pub mod cost;
pub mod duration;
pub mod harness;
pub mod index;
pub mod keymap;
pub mod launch;
pub mod logging;
pub mod markdown;
pub mod model;
pub mod paths;
pub mod section_details;
pub mod terminal;
#[doc(hidden)]
pub mod test_support;
pub mod theme;
pub mod timestamp;
pub mod tokens;
pub mod transcript_search;

/// Library entry point called by the binary.
///
/// Parses the process arguments. `--help` and `--version` print and exit with
/// code 0; invalid arguments exit with code 2 (both handled by clap).
/// Otherwise it starts the application and returns when the user quits. An
/// error that stops the start (no session matches, ...) is returned before the
/// terminal is touched.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    match cli::parse_args(std::env::args_os()) {
        Ok(cli) => Ok(app::runtime::run_app(&cli)?),
        Err(err) => err.exit(),
    }
}
