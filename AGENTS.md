# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run Commands

```bash
cargo build                    # Debug build
cargo build --release          # Release build
cargo install --path .         # Install binary locally as 'sm'
cargo test                     # Run all tests
cargo test skill_ref           # Run specific test module
```

## Architecture Overview

Agent Skill Manager (`sm`) is a CLI tool for managing Claude Code skills. It clones skill repositories, creates symlinks to `~/.claude/skills/`, and tracks state in a TOML config file.

### Module Structure

- **`cli.rs`** - Clap-based argument parsing with command hierarchy
- **`skill_ref.rs`** - Parses skill/repo references (e.g., `github.com/owner/repo/path/skill[@SHA]`)
- **`commands/skill.rs`** - Skill enable/disable/list, symlink management, interactive mode
- **`commands/repo.rs`** - Repository add/delete/pin/upgrade, skill discovery
- **`config/state.rs`** - Config data structures (Repository, Skill)
- **`config/lock.rs`** - File locking with `fs2`, atomic writes via temp file + rename
- **`git/mod.rs`** - Git CLI wrappers (clone, pull, checkout, get SHA)
- **`paths.rs`** - Centralized path management

### Key Patterns

**Config locking**: All config modifications acquire `ConfigLock` first, ensuring atomic updates:
```rust
let mut lock = ConfigLock::acquire()?;
let mut config = lock.load_config()?;
// modify config
lock.save_config(&config)?;
```

**Skill discovery**: `scan_for_skills()` finds directories containing `SKILL.md` files.

**Symlinks**: Skills are enabled by symlinking `~/.claude/skills/{name}` to the cached repo path.

**Reference parsing**: `SkillRef` and `RepoRef` in `skill_ref.rs` normalize various input formats and extract owner/repo/path/sha components.

### Data Storage

- **Config**: `~/.cache/agent-skill-manager/config.toml`
- **Git cache**: `~/.cache/agent-skill-manager/git/{owner}/{repo}/`
- **Claude skills**: `~/.claude/skills/` (symlinks)

## Limitations

- GitHub only (no GitLab/Bitbucket)
- Unix only (uses symlinks, no Windows support)
- Requires `git` CLI installed
