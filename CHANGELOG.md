# Changelog

## Unreleased

### Added

- **Universal git support** - Repository references now support any git source
  - HTTPS URLs from any domain (GitLab, Bitbucket, self-hosted servers)
  - SSH URLs (`git@host:owner/repo.git` format)
  - Local filesystem paths (absolute, relative, ~-prefixed)
  - Tag and commit references with `@tag` or `@sha` suffix

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
