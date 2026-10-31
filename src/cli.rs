//! The command line (SPEC §6.8).

use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use crate::launch::Request;
use crate::model::HarnessId;

/// A harness `--harness` can name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum HarnessArg {
    Claude,
    Opencode,
}

impl From<HarnessArg> for HarnessId {
    fn from(arg: HarnessArg) -> Self {
        match arg {
            HarnessArg::Claude => HarnessId::Claude,
            HarnessArg::Opencode => HarnessId::OpenCode,
        }
    }
}

/// Command-line interface.
#[derive(Debug, Clone, Default, Parser)]
#[command(name = "chronoscope", version, about)]
pub struct Cli {
    /// A session id (or the start of one) to open directly, or the path of a
    /// Claude `.jsonl` file. Without it the session selector opens.
    #[arg(value_name = "SESSION_ID | PATH")]
    pub target: Option<String>,

    /// Open the newest session of the current directory's project (or of
    /// `--project`)
    #[arg(long, conflicts_with = "target")]
    pub latest: bool,

    /// Restrict the lookup to one harness, and start on its tab
    #[arg(long, value_enum, ignore_case = true, value_name = "HARNESS")]
    pub harness: Option<HarnessArg>,

    /// Start on this project: a path or a name
    #[arg(long, value_name = "PATH|NAME")]
    pub project: Option<String>,

    /// Use this theme for this run (it is not saved)
    #[arg(long, value_name = "NAME")]
    pub theme: Option<String>,

    /// Read this config file instead of the default one
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,
}

impl Cli {
    /// What the arguments ask the session lookup for.
    pub fn request(&self) -> Request {
        Request {
            target: self.target.clone(),
            latest: self.latest,
            harness: self.harness.map(HarnessId::from),
            project: self.project.clone(),
        }
    }
}

/// Parses `args` (the first item is the program name) into the CLI.
///
/// Returns the clap error for invalid input, `--help` and `--version`.
pub fn parse_args<I, T>(args: I) -> Result<Cli, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    Cli::try_parse_from(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use clap::error::ErrorKind;

    #[test]
    fn no_arguments_parse() {
        let cli = parse_args(["chronoscope"]).unwrap();
        assert_eq!(cli.request(), Request::default());
        assert!(cli.theme.is_none() && cli.config.is_none());
    }

    #[test]
    fn unknown_argument_is_rejected() {
        let err = parse_args(["chronoscope", "--bogus"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn version_and_help_are_reported_by_clap() {
        let v = parse_args(["chronoscope", "--version"]).unwrap_err();
        assert_eq!(v.kind(), ErrorKind::DisplayVersion);
        let h = parse_args(["chronoscope", "--help"]).unwrap_err();
        assert_eq!(h.kind(), ErrorKind::DisplayHelp);
    }

    #[test]
    fn help_lists_the_positional_argument_and_every_option() {
        let help = Cli::command().render_help().to_string();
        for needle in [
            "[SESSION_ID | PATH]",
            "--latest",
            "--harness <HARNESS>",
            "--project <PATH|NAME>",
            "--theme <NAME>",
            "--config <PATH>",
        ] {
            assert!(help.contains(needle), "{needle} is missing from:\n{help}");
        }
    }

    #[test]
    fn latest_conflicts_with_the_positional_argument() {
        let err = parse_args(["chronoscope", "--latest", "85a8"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
        let err = parse_args(["chronoscope", "85a8", "--latest"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
    }

    #[test]
    fn every_option_is_read() {
        let cli = parse_args([
            "chronoscope",
            "--latest",
            "--harness",
            "opencode",
            "--project",
            "brio",
            "--theme",
            "gruvbox",
            "--config",
            "/tmp/c.toml",
        ])
        .unwrap();
        assert_eq!(
            cli.request(),
            Request {
                target: None,
                latest: true,
                harness: Some(HarnessId::OpenCode),
                project: Some("brio".to_owned()),
            }
        );
        assert_eq!(cli.theme.as_deref(), Some("gruvbox"));
        assert_eq!(cli.config, Some(PathBuf::from("/tmp/c.toml")));
    }

    #[test]
    fn the_harness_is_case_insensitive_and_limited_to_the_known_ones() {
        let cli = parse_args(["chronoscope", "--harness", "OpenCode"]).unwrap();
        assert_eq!(cli.request().harness, Some(HarnessId::OpenCode));
        let cli = parse_args(["chronoscope", "--harness", "CLAUDE"]).unwrap();
        assert_eq!(cli.request().harness, Some(HarnessId::Claude));
        let err = parse_args(["chronoscope", "--harness", "codex"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidValue);
    }

    #[test]
    fn the_positional_argument_is_kept_as_typed() {
        let cli = parse_args(["chronoscope", "85a8"]).unwrap();
        assert_eq!(cli.target.as_deref(), Some("85a8"));
    }
}
