use std::io::IsTerminal;

use anyhow::{bail, Result};
use inquire::Confirm;

/// Sorted for binary_search.
const WRITE_VERBS: &[&str] = &[
    "add",
    "bulk-reorder",
    "create",
    "delete",
    "move",
    "remove",
    "reorder",
    "set",
    "set-default",
    "update",
    "upload",
];

pub fn is_write_verb(name: &str) -> bool {
    WRITE_VERBS.binary_search(&name).is_ok()
}

/// Block a write when read-only mode is on. Called by write commands, which know
/// they're writes — no fragile name-matching.
pub fn enforce_read_only(read_only: bool, action: &str) -> Result<()> {
    if read_only {
        bail!("Write operation '{action}' is blocked in read-only mode (--read-only / NESTR_READ_ONLY).");
    }
    Ok(())
}

/// The environment variables a coding agent sets, each with the fixed name the
/// API is told when it is the one driving the CLI. Only these names ever leave
/// the machine: never the variable's value, a path, or anything about the user.
const AGENT_ENV_VARS: &[(&str, &str)] = &[
    ("CLAUDECODE", "claude-code"),
    ("CLAUDE_CODE", "claude-code"),
    ("CODEX", "codex"),
    ("CURSOR_AGENT", "cursor"),
    ("NESTR_AGENT_MODE", "agent"),
];

/// Which agent is driving the CLI, given a way to look a variable up. Pure, so
/// it is tested without touching the process environment.
pub fn agent_name_from(is_set: impl Fn(&str) -> bool) -> Option<&'static str> {
    AGENT_ENV_VARS
        .iter()
        .find(|(var, _)| is_set(var))
        .map(|(_, name)| *name)
}

/// Which agent is driving the CLI, if any.
pub fn agent_name() -> Option<&'static str> {
    agent_name_from(|v| std::env::var_os(v).is_some())
}

pub fn is_agent_mode() -> bool {
    agent_name().is_some()
}

/// Confirm a destructive action. `--yes` auto-approves; agent/non-tty error out.
pub fn confirm_destructive(action: &str, yes: bool) -> Result<()> {
    if yes {
        eprintln!("[auto-approved via --yes] {action}");
        return Ok(());
    }
    if is_agent_mode() {
        bail!("'{action}' needs confirmation. You are in agent mode — re-run with --yes once the user confirms.");
    }
    if !std::io::stdin().is_terminal() {
        bail!("'{action}' needs confirmation but stdin is not a terminal. Pass --yes.");
    }
    if !Confirm::new(action).with_default(false).prompt()? {
        bail!("Cancelled.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_verbs_are_sorted_for_binary_search() {
        assert!(
            WRITE_VERBS.is_sorted(),
            "WRITE_VERBS must stay sorted or is_write_verb's binary_search breaks silently"
        );
    }

    #[test]
    fn classifies_write_verbs() {
        assert!(is_write_verb("delete"));
        assert!(is_write_verb("update"));
        assert!(!is_write_verb("list"));
        assert!(!is_write_verb("get"));
    }

    #[test]
    fn names_the_agent_from_its_variable() {
        assert_eq!(agent_name_from(|v| v == "CLAUDECODE"), Some("claude-code"));
        assert_eq!(agent_name_from(|v| v == "CODEX"), Some("codex"));
        assert_eq!(agent_name_from(|v| v == "CURSOR_AGENT"), Some("cursor"));
        assert_eq!(agent_name_from(|v| v == "NESTR_AGENT_MODE"), Some("agent"));
        assert_eq!(agent_name_from(|_| false), None);
    }

    #[test]
    fn agent_names_are_fixed_header_safe_labels() {
        for (_, name) in AGENT_ENV_VARS {
            assert!(
                name.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "{name} must stay a fixed lowercase label"
            );
        }
    }

    #[test]
    fn read_only_blocks_only_when_flag_set() {
        assert!(enforce_read_only(true, "nests delete").is_err());
        assert!(enforce_read_only(false, "nests delete").is_ok());
    }
}
