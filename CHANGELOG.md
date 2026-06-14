# Changelog

## Unreleased

### Fixed

- **`sm repo list` columns now align regardless of repository name length.** Column widths are computed from the actual content (and padding ignores ANSI color codes), so long repository ids/paths no longer push the later columns out of alignment. Repositories are also listed in a stable, sorted order.

### Added

- **Automatic once-a-day update notice.** After a command (on an interactive terminal, at most once per 24h), `sm` checks whether a newer release is published and prints a one-line "✨ A new version of sm is available" notice pointing at `sm self upgrade`. Best-effort and silent on failure; disable with `SM_NO_UPDATE_CHECK=1`.
- **Per-repository auto-upgrade.** Flag a repository with `sm repo auto-upgrade on <url>` (or `sm repo add --auto-upgrade <url>`) and it is pulled to its latest commit automatically by the same once-a-day maintenance pass — third-party repos stay manual unless you opt them in. State is shown in `sm repo list` (new `AUTO-UPGRADE` column). Auto-upgrade and pinning are mutually exclusive (pinning turns it off); local repositories are excluded since they are always live. Disable the background pass with `SM_NO_AUTO_UPGRADE=1`.

## Version 0.0.9 (2026-06-14)

### Added

- **Type-to-search in interactive menus.** The interactive selection menus (`sm`, `sm add -i`, and `sm configure`) now filter as you type, using case-insensitive substring matching on the item name and repository. Matching keeps the original alphabetical order; an empty query shows everything.

## Version 0.0.8 (2026-06-14)

### Fixed

- **Interactive mode (and `sm skills enable` / `sm subagents enable`) can now enable a skill or agent that exists on disk in a registered repository but is missing from config.** A bare name not in config was treated as a remote `OWNER/REPO/SKILL` reference and failed with "Not a valid skill reference" (agents: "not found") — even though interactive mode had just listed it from disk. Such names are now matched against the skills/agents present on disk and registered automatically, which also heals config/disk drift (e.g. after a repository's contents changed).

## Version 0.0.7 (2026-06-14)

### Fixed

- **`sm upgrade` no longer drops skills/agents from repositories that keep them in a subdirectory.** A repo added with a subpath (e.g. `owner/repo/skills`) stores that subdirectory, but upgrading rescanned the repo *root* instead — found nothing, and removed every registered skill/agent as "no longer available". Upgrades now reconstruct the repository from its stored record, so the configured subdirectory is always used. As a safety net, an empty scan while items are still registered is now skipped with a warning rather than wiping them.

## Version 0.0.6 (2026-06-14)

### Added

- **`sm self upgrade`** - Upgrade the `sm` binary itself to the latest published release, in place. Checks the GitHub releases API and, when a newer version exists, re-runs the official installer (with checksum verification). Flags: `--check` reports whether an update is available without installing; `--force` reinstalls the latest release even when already up to date (useful for repairing an install) and skips the confirmation prompt. The upgrade never modifies your shell profile.

## Version 0.0.5 (2026-01-31)

### Added

- **Local repos as direct-link** - Local repositories (`/path/to/dir`, `~/dir`, `./dir`) now skip `git clone` entirely. Symlinks point directly to the source directory, so changes are reflected immediately without needing `sm upgrade`. Pin/unpin are rejected for local repos since they have no SHA tracking.
- **`sm add` supports local skill paths** - `sm add /path/to/skill-dir` and `sm add /path/to/skill-dir/SKILL.md` now work directly. The parent directory is auto-registered as a local repo and the skill is enabled in one step.

## Version 0.0.4 (2026-01-18)

### Added

- **Subagent (Agent) support** - Agents can now be discovered and managed alongside skills
  - `sm subagents enable/disable/list` - Manage agents
  - Agent discovery from `AGENT.md` files (parallel to `SKILL.md` for skills)
  - Interactive mode shows both skills and agents with color-coded labels
  - `sm add -i <url>` now detects and displays agents from repositories
  - `--agents-path` flag for integrations to specify agent installation directory

- **Consistent label colors** - Skill and agent labels use consistent colors across all commands
  - `[skill]` labels appear in cyan
  - `[agent]` labels appear in yellow
  - Colors consistent across `sm list`, `sm` (interactive), and `sm add -i`

- **Multi-repository operations**
  - `sm repo add <url1> <url2> <url3>` - Add multiple repositories in one command
  - `sm repo rm <repo1> <repo2> <repo3>` - Remove multiple repositories with progress summary
  - Partial success reporting when some operations fail

- **Standardized cache location** - Cache location is now consistent across all platforms
  - Uses `~/.cache/agent-skill-manager` on all Unix-based systems
  - No longer uses platform-specific directories like `~/Library/Caches` on macOS

- **Lenient file:// URL parsing**
  - `file://path/to/repo` is auto-corrected to `/path/to/repo`
  - `file:///path/to/repo` (correct format) also works

- **Universal git support** - Repository references now support any git source
  - HTTPS URLs from any domain (GitLab, Bitbucket, self-hosted servers)
  - SSH URLs (`git@host:owner/repo.git` format)
  - Local filesystem paths (absolute, relative, ~-prefixed, file:// URLs)
  - Tag and commit references with `@tag` or `@sha` suffix

### Fixed

- **Orphaned agent cleanup** - Agents are now properly removed when their repository is deleted
  - Previously, disabling a repo only removed skills, leaving orphaned agent entries
  - Now removes both skills and agents from config and filesystem

- **Bare `sm` command now shows skills from local repositories**
  - Interactive mode correctly detects and displays skills from locally-cloned repositories
  - Properly handles repo_id format for local paths

- **Interactive mode with local paths** - `sm add -i /path/to/repo` now works correctly
  - Previously failed with "Not a valid GitHub skill reference" error
  - Now successfully shows interactive selection from local repositories

- **Consistent flag behavior** - `-a` short flag now consistently means `--all` across all commands
  - Changed `sm list -a` from meaning `--agents` to meaning `--all`
  - Agent filtering now uses `-g` short flag where needed

- **Fixed interactive mode not detecting legacy cache paths**
  - Skills and agents from repos cloned before universal git support are now properly detected
  - Symlink creation works correctly regardless of cache path format

- **Improved error messages for `sm add` when given repository URLs instead of skill references**
  - Now suggests `sm add -i <url>` for interactive mode
  - Suggests `sm repo add` then `sm skills enable` as alternative
  - Removed GitHub-specific language from error messages

## Version 0.0.3 (2026-01-16)

- **Multi-integration support** - Skills can now be enabled for multiple coding agents simultaneously
  - `sm integrations add claude-code` - Register an integration with default path
  - `sm integrations add opencode` - Built-in support for OpenCode (~/.config/opencode/skill)
  - `sm integrations add codex` - Built-in support for OpenAI Codex CLI (~/.codex/skills)
  - `sm integrations add gemini-cli` - Built-in support for Gemini CLI (~/.gemini/skills)
  - `sm integrations add <name> --path <dir>` - Add custom integration with any path
  - `sm integrations remove <name>` - Remove an integration
  - `sm integrations list` - List all integrations (always shows built-in presets)
  - `sm configure` / `sm config` - Interactive setup to select integrations
  - `sm int` - Shorthand alias for integrations commands
- Name aliases for integrations (e.g., claude/claudecode → claude-code, gemini → gemini-cli)

## Version 0.0.2 (2026-01-08)

### Fixed

- Fix deadlocks in `sm upgrade` and `sm add` commands when performing batch operations

## Version 0.0.1 (2026-01-08)

Initial release of `sm` (Agent Skill Manager) - a CLI for managing [Claude Code](https://claude.ai/code) skills from GitHub repositories.

## Highlights

- **Interactive mode** - Run `sm` to browse and toggle skills with a visual interface
- **One-command install** - `sm add github.com/anthropics/skills/skills/pdf` does everything
- **Instant switching** - Skills enable/disable via symlinks, no re-downloading
- **Version pinning** - Lock repositories to specific commits for reproducibility

## Added

- Interactive TUI for skill management
- `sm add` - one-step skill installation
- `sm add -i` - interactive skill selection from a repository
- `sm enable/disable` - toggle skills (supports multiple at once)
- `sm list` - view skills with filtering options
- `sm repo add/delete/list` - repository management
- `sm repo pin/unpin/upgrade` - version control
- `sm upgrade` - bulk upgrade unpinned repositories
- `sm purge` - complete reset
- `sm cache dir` - show cache location
- Extensive command aliases (`repo`, `repos`, `skill`, `sk`, `ls`, etc.)

## Limitations

- GitHub only (GitLab, Bitbucket planned)
- Claude Code only (other agents planned)
- Unix/macOS only (Windows planned - requires symlink alternative)
