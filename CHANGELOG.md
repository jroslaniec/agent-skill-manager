# Changelog

All notable changes to this project will be documented in this file.

# Version 0.0.1 (2026-01-08)

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
