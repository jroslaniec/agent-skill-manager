//! Once-a-day background maintenance: a binary update notice and auto-upgrade of
//! flagged repositories.
//!
//! Runs after a command completes, gated to roughly once per 24h via a small
//! state file (kept out of the lock-guarded `config.toml`), only on a TTY, and
//! is entirely best-effort — it never errors out or changes the exit code.

use std::io::IsTerminal;

use serde::{Deserialize, Serialize};

use crate::paths;

const CHECK_INTERVAL_HOURS: i64 = 24;

#[derive(Debug, Default, Serialize, Deserialize)]
struct MaintenanceState {
    /// RFC3339 timestamp of the last maintenance run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_run: Option<String>,
    /// Latest version seen during the last update check (informational).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    latest_version: Option<String>,
}

fn state_path() -> Option<std::path::PathBuf> {
    paths::cache_dir().ok().map(|d| d.join("maintenance.toml"))
}

fn read_state() -> MaintenanceState {
    state_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_state(state: &MaintenanceState) {
    if let Some(p) = state_path()
        && let Ok(text) = toml::to_string(state)
    {
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(p, text);
    }
}

/// True if at least `CHECK_INTERVAL_HOURS` have elapsed since the last run (or it
/// has never run / the stored timestamp can't be parsed).
fn is_due(state: &MaintenanceState) -> bool {
    let Some(ts) = &state.last_run else {
        return true;
    };
    match chrono::DateTime::parse_from_rfc3339(ts) {
        Ok(last) => {
            let elapsed =
                chrono::Utc::now().signed_duration_since(last.with_timezone(&chrono::Utc));
            elapsed >= chrono::Duration::hours(CHECK_INTERVAL_HOURS)
        }
        Err(_) => true,
    }
}

/// An env var counts as "set to disable" when present and not `0`/empty.
fn env_disabled(var: &str) -> bool {
    std::env::var_os(var).is_some_and(|v| !v.is_empty() && v != "0")
}

/// Entry point called from `main` after the user's command. Best-effort; any
/// failure is swallowed so it can never affect the real command's result.
pub fn run_after_command() {
    // Never run in non-interactive contexts (pipes, scripts, CI).
    if !std::io::stdout().is_terminal() {
        return;
    }

    let update_check = !env_disabled("SM_NO_UPDATE_CHECK");
    let auto_upgrade = !env_disabled("SM_NO_AUTO_UPGRADE");
    if !update_check && !auto_upgrade {
        return;
    }

    let mut state = read_state();

    // The network work — looking up the latest release and pulling auto-upgrade
    // repos — is gated to once per 24h.
    if is_due(&state) {
        // Mark as run now, regardless of outcome, so a flaky network or a slow
        // upstream doesn't make us retry on every invocation for the rest of the day.
        state.last_run = Some(chrono::Utc::now().to_rfc3339());

        // Only overwrite the known latest version on a *successful* lookup, so a
        // transient network failure doesn't wipe a previously-found update.
        if update_check && let Ok(latest) = crate::commands::selfupdate::fetch_latest_version() {
            state.latest_version = Some(latest);
        }

        if auto_upgrade {
            let _ = crate::commands::repo::auto_upgrade_pass();
        }

        write_state(&state);
    }

    // The "update available" notice is shown on EVERY run from the last known
    // latest version (not just when the 24h check happens to run), so it keeps
    // reminding until you actually upgrade.
    if update_check {
        notify_if_update_available(state.latest_version.as_deref());
    }
}

/// Print the one-line "new version available" notice if `latest` is newer than
/// the running binary. A no-op when the latest version is unknown or not newer.
fn notify_if_update_available(latest: Option<&str>) {
    let current = env!("CARGO_PKG_VERSION");
    let Some(latest) = latest else {
        return;
    };

    if crate::commands::selfupdate::is_newer(latest, current) {
        eprintln!(
            "\n{} A new version of sm is available: {} → {}. Run `{}`.",
            console::style("✨").yellow(),
            console::style(current).dim(),
            console::style(latest).cyan(),
            console::style("sm self upgrade").cyan()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_due_when_never_run() {
        assert!(is_due(&MaintenanceState::default()));
    }

    #[test]
    fn is_due_when_timestamp_unparseable() {
        let state = MaintenanceState {
            last_run: Some("not-a-timestamp".to_string()),
            latest_version: None,
        };
        assert!(is_due(&state));
    }

    #[test]
    fn not_due_right_after_a_run() {
        let state = MaintenanceState {
            last_run: Some(chrono::Utc::now().to_rfc3339()),
            latest_version: None,
        };
        assert!(!is_due(&state));
    }

    #[test]
    fn due_after_interval_elapsed() {
        let long_ago = chrono::Utc::now() - chrono::Duration::hours(CHECK_INTERVAL_HOURS + 1);
        let state = MaintenanceState {
            last_run: Some(long_ago.to_rfc3339()),
            latest_version: None,
        };
        assert!(is_due(&state));
    }

    #[test]
    fn env_disabled_semantics() {
        // Uses a var name unlikely to be set in the environment.
        let var = "SM_TEST_DISABLE_FLAG_XYZ";
        unsafe { std::env::remove_var(var) };
        assert!(!env_disabled(var));
        unsafe { std::env::set_var(var, "1") };
        assert!(env_disabled(var));
        unsafe { std::env::set_var(var, "0") };
        assert!(!env_disabled(var));
        unsafe { std::env::set_var(var, "") };
        assert!(!env_disabled(var));
        unsafe { std::env::remove_var(var) };
    }
}
