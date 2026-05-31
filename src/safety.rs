use std::io::IsTerminal;

use anyhow::{bail, Result};
use inquire::Confirm;

/// Sorted for binary_search.
const WRITE_VERBS: &[&str] = &[
    "add",
    "create",
    "delete",
    "remove",
    "reorder",
    "set",
    "set-default",
    "update",
];

pub fn is_write_verb(name: &str) -> bool {
    WRITE_VERBS.binary_search(&name).is_ok()
}

/// Block writes when `--read-only` / `NESTR_READ_ONLY` is set.
pub fn enforce_read_only(verb: &str) -> Result<()> {
    if is_write_verb(verb) {
        bail!("Write operation '{verb}' is blocked in read-only mode.");
    }
    Ok(())
}

const AGENT_ENV_VARS: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_CODE",
    "CODEX",
    "CURSOR_AGENT",
    "NESTR_AGENT_MODE",
];

pub fn is_agent_mode() -> bool {
    AGENT_ENV_VARS.iter().any(|v| std::env::var(v).is_ok())
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
    fn classifies_write_verbs() {
        assert!(is_write_verb("delete"));
        assert!(is_write_verb("update"));
        assert!(!is_write_verb("list"));
        assert!(!is_write_verb("get"));
    }

    #[test]
    fn read_only_blocks_writes_only() {
        assert!(enforce_read_only("delete").is_err());
        assert!(enforce_read_only("list").is_ok());
    }
}
