//! The resume command of a Claude session (SPEC §6.7).

use std::process::Command;

use crate::model::SessionSummary;

/// The default command template: `{id}` stands for the session id.
pub const DEFAULT_RESUME_COMMAND: &str = "claude --resume {id}";

/// Builds the command from `template`: split into words with shell quoting
/// rules (no shell is involved), `{id}` replaced in every argument, run in the
/// session's cwd when it is known. An empty or unparsable template gives
/// `None` and a logged warning.
pub(super) fn build(template: &str, session: &SessionSummary) -> Option<Command> {
    let words = match shell_words::split(template) {
        Ok(words) => words,
        Err(err) => {
            log::warn!("invalid resume command {template:?}: {err}");
            return None;
        }
    };
    let Some((program, args)) = words.split_first() else {
        log::warn!("the resume command is empty");
        return None;
    };
    let mut command = Command::new(program);
    command.args(args.iter().map(|a| a.replace("{id}", session.id.as_str())));
    if let Some(cwd) = &session.cwd {
        command.current_dir(cwd);
    }
    Some(command)
}
