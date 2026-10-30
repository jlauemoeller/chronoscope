//! chronoscope: a terminal viewer for AI coding-agent session histories.
//!
//! All application code lives in this library; the binary only calls [`run`].

use clap::Parser;

pub mod app;
pub mod components;
pub mod config;
pub mod harness;
pub mod index;
pub mod keymap;
pub mod logging;
pub mod markdown;
pub mod model;
pub mod paths;
pub mod terminal;
#[doc(hidden)]
pub mod test_support;
pub mod theme;
pub mod timestamp;

/// Command-line interface. Only `--help` and `--version` exist for now.
#[derive(Debug, Parser)]
#[command(name = "chronoscope", version, about)]
struct Cli {}

/// Parses `args` (the first item is the program name) into the CLI.
///
/// Returns the clap error for invalid input, `--help` and `--version`.
fn parse_args<I, T>(args: I) -> Result<Cli, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    Cli::try_parse_from(args)
}

/// Library entry point called by the binary.
///
/// Parses the process arguments. `--help` and `--version` print and exit with
/// code 0; invalid arguments exit with code 2 (both handled by clap).
/// Otherwise it starts the application and returns when the user quits.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    match parse_args(std::env::args_os()) {
        Ok(Cli {}) => Ok(app::runtime::run_app()?),
        Err(err) => err.exit(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_arguments_parse() {
        assert!(parse_args(["chronoscope"]).is_ok());
    }

    #[test]
    fn unknown_argument_is_rejected() {
        let err = parse_args(["chronoscope", "--bogus"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::UnknownArgument);
    }

    #[test]
    fn version_and_help_are_reported_by_clap() {
        let v = parse_args(["chronoscope", "--version"]).unwrap_err();
        assert_eq!(v.kind(), clap::error::ErrorKind::DisplayVersion);
        let h = parse_args(["chronoscope", "--help"]).unwrap_err();
        assert_eq!(h.kind(), clap::error::ErrorKind::DisplayHelp);
    }
}
