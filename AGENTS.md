# AGENTS.md

This file provides guidance to coding agents when working with code in this repository.

## Build & Run Commands

```bash
cargo build                    # Debug build
cargo build --release          # Release build
cargo install --path .         # Install binary locally as 'sm'
cargo test                     # Run all tests
cargo test skill_ref           # Run specific test module
```

## Linting

**Before committing**, run these checks (CI will enforce them):

```bash
cargo fmt --check              # Check formatting
cargo clippy -- -D warnings    # Lint with warnings as errors
```

To auto-fix formatting issues: `cargo fmt`

## Architecture Overview

Agent Skill Manager (`sm`) is a CLI tool for managing Claude Code skills. It clones skill repositories (or links directly to local directories), creates symlinks to `~/.claude/skills/`, and tracks state in a TOML config file.

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

**Config locking**: All config modifications acquire `ConfigLock` first via `lock.update(|config| ...)`.

**Avoiding deadlocks**: `flock()` locks per file descriptor—nested `ConfigLock::acquire()` calls deadlock. For batch operations, create `*_with_lock(&lock)` variants that accept an existing lock reference. Never call a public lock-acquiring function from within another.

**Skill discovery**: `scan_for_skills()` finds directories containing `SKILL.md` files.

**Symlinks**: Skills are enabled by symlinking `~/.claude/skills/{name}` to the cached repo path. For local repositories, symlinks point directly to the source directory (no cache copy).

**Reference parsing**: `SkillRef` and `RepoRef` in `skill_ref.rs` normalize various input formats and extract owner/repo/path/sha components.

### Data Storage

- **Config**: `~/.cache/agent-skill-manager/config.toml`
- **Git cache**: `~/.cache/agent-skill-manager/git/{owner}/{repo}/`
- **Claude skills**: `~/.claude/skills/` (symlinks)

## Changelog & Releases

**Changelog**: User-facing changes must be documented in `CHANGELOG.md` under the `## Unreleased` section. When a release happens, CI automatically renames this section to the version number and adds a fresh `## Unreleased` section.

**Release process**: Add the `release` label to a PR. When merged, CI will:
1. Bump the patch version in `Cargo.toml`
2. Rename `## Unreleased` to `## Version X.Y.Z (date)` in `CHANGELOG.md`
3. Commit, tag, and push → triggers cargo-dist release workflow

**What to document**: Bug fixes, new features, breaking changes, and removed features that affect end users. Internal refactors don't need changelog entries.

## Limitations

- Unix only (uses symlinks, no Windows support)
- Requires `git` CLI installed (not needed for local-only repos)
