use anyhow::{Context, Result, bail};
use console::style;
use std::io::{self, Write};
use std::process::Command;

/// GitHub repository that publishes `sm` releases.
const REPO: &str = "jroslaniec/agent-skill-manager";

/// The official cargo-dist installer for the latest release.
/// Re-running it downloads, checksum-verifies, and installs the newest build,
/// replacing the current binary in place. This is the same script the README
/// documents for installation.
const INSTALLER_URL: &str = "https://github.com/jroslaniec/agent-skill-manager/releases/latest/download/agent-skill-manager-installer.sh";

/// Upgrade the `sm` binary itself to the latest released version.
///
/// * `check` - report whether an update is available without installing anything.
/// * `force` - reinstall the latest release even if already up to date, and skip
///   the confirmation prompt (useful for repairing a broken install).
pub fn upgrade(check: bool, force: bool) -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");

    let latest = fetch_latest_version()
        .context("Could not determine the latest version from GitHub releases")?;

    let update_available = is_newer(&latest, current);

    if check {
        if update_available {
            println!(
                "{} Update available: {} → {}",
                style("✨").yellow(),
                style(current).dim(),
                style(&latest).cyan()
            );
            println!("Run `sm self upgrade` to install it.");
        } else {
            println!(
                "{} sm is up to date ({})",
                style("✓").green(),
                style(current).cyan()
            );
        }
        return Ok(());
    }

    if !update_available && !force {
        println!(
            "{} sm is already up to date ({})",
            style("✓").green(),
            style(current).cyan()
        );
        return Ok(());
    }

    if update_available {
        println!(
            "{} Update available: {} → {}",
            style("✨").yellow(),
            style(current).dim(),
            style(&latest).cyan()
        );
    } else {
        // Reached only with --force on an up-to-date install.
        println!(
            "{} Reinstalling the latest release ({})",
            style("↻").cyan(),
            style(&latest).cyan()
        );
    }

    if !force && !confirm("Upgrade now?")? {
        println!("Cancelled.");
        return Ok(());
    }

    run_installer()?;

    println!(
        "{} sm is now at {}",
        style("✓").green(),
        style(&latest).cyan()
    );
    println!(
        "{}",
        style("Restart your shell or open a new terminal if the version looks stale.").dim()
    );

    Ok(())
}

/// Fetch the latest release version (without a leading `v`) from the GitHub API.
///
/// Shells out to `curl` to stay consistent with the rest of the tool (which
/// shells out to `git`) and to avoid pulling in an HTTP/async dependency stack.
fn fetch_latest_version() -> Result<String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");

    let output = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "10",
            "-H",
            "Accept: application/vnd.github+json",
            &url,
        ])
        .output()
        .context("Failed to run `curl` (is it installed?)")?;

    if !output.status.success() {
        bail!(
            "curl failed while contacting GitHub:\n{}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let body = String::from_utf8_lossy(&output.stdout);
    parse_tag_name(&body).context("Could not find a release tag in the GitHub response")
}

/// Extract the `tag_name` value from a GitHub release JSON payload and strip a
/// leading `v`. Done by hand to avoid a JSON-parsing dependency.
fn parse_tag_name(json: &str) -> Option<String> {
    let key = "\"tag_name\"";
    let after_key = &json[json.find(key)? + key.len()..];
    let after_colon = &after_key[after_key.find(':')? + 1..];
    let start = after_colon.find('"')? + 1;
    let end = after_colon[start..].find('"')? + start;
    let tag = after_colon[start..end].trim();
    Some(tag.strip_prefix('v').unwrap_or(tag).to_string())
}

/// Return true if `latest` is a strictly newer version than `current`.
fn is_newer(latest: &str, current: &str) -> bool {
    version_tuple(latest) > version_tuple(current)
}

/// Parse a `major.minor.patch` string into a comparable tuple. Non-numeric
/// suffixes (e.g. prerelease tags) on a component are ignored for that field.
fn version_tuple(v: &str) -> (u64, u64, u64) {
    let mut parts = v.split('.').map(|p| {
        p.split(|c: char| !c.is_ascii_digit())
            .next()
            .unwrap_or("")
            .parse::<u64>()
            .unwrap_or(0)
    });
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

/// Prompt for a yes/no confirmation on stdin.
fn confirm(question: &str) -> Result<bool> {
    print!("{question} (yes/no): ");
    io::stdout().flush()?;
    let mut response = String::new();
    io::stdin().read_line(&mut response)?;
    Ok(response.trim().eq_ignore_ascii_case("yes"))
}

/// Download and run the official installer for the latest release.
///
/// The installer installs into `$CARGO_HOME/bin` (where `sm` already lives) and
/// verifies checksums. We set `AGENT_SKILL_MANAGER_NO_MODIFY_PATH=1` because an
/// upgrade should never touch the user's shell profile — PATH is already set up.
/// An `AGENT_SKILL_MANAGER_INSTALL_DIR` already present in the environment is
/// honored by the installer (used for sandboxed testing).
fn run_installer() -> Result<()> {
    println!("Downloading and running the installer...");

    let command = format!("curl --proto '=https' --tlsv1.2 -LsSf {INSTALLER_URL} | sh");

    let status = Command::new("sh")
        .arg("-c")
        .arg(&command)
        .env("AGENT_SKILL_MANAGER_NO_MODIFY_PATH", "1")
        .status()
        .context("Failed to launch the installer")?;

    if !status.success() {
        bail!("Installer exited with a non-zero status");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tag_name_basic() {
        let json = r#"{"url":"…","tag_name":"v0.0.6","name":"Version 0.0.6"}"#;
        assert_eq!(parse_tag_name(json).as_deref(), Some("0.0.6"));
    }

    #[test]
    fn parse_tag_name_without_v_prefix() {
        let json = r#"{"tag_name": "1.2.3"}"#;
        assert_eq!(parse_tag_name(json).as_deref(), Some("1.2.3"));
    }

    #[test]
    fn parse_tag_name_missing() {
        assert_eq!(parse_tag_name(r#"{"message":"Not Found"}"#), None);
    }

    #[test]
    fn version_tuple_parses_components() {
        assert_eq!(version_tuple("0.0.5"), (0, 0, 5));
        assert_eq!(version_tuple("1.20.3"), (1, 20, 3));
        assert_eq!(version_tuple("2.0"), (2, 0, 0));
    }

    #[test]
    fn version_tuple_ignores_prerelease_suffix() {
        assert_eq!(version_tuple("0.1.0-rc.1"), (0, 1, 0));
    }

    #[test]
    fn is_newer_compares_correctly() {
        assert!(is_newer("0.0.6", "0.0.5"));
        assert!(is_newer("0.1.0", "0.0.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.0.5", "0.0.5"));
        assert!(!is_newer("0.0.4", "0.0.5"));
    }
}
