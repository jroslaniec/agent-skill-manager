# Agent Skill Manager

A CLI tool for managing Agent Skills.

**Current Limitations**:

- Only GitHub repositories are supported
- Only Claude Code is supported
- Windows is not supported (uses Unix symlinks)

## How it works

The tool clones repositories containing skills to a local cache directory. When you enable a skill, it creates a symlink from `~/.claude/skills/<skill-name>` to the cached repository location. Disabling a skill removes the symlink while keeping the repository cached for fast re-enabling.

## Installation

### From Source

```bash
cargo install --path .
```

## Usage

### Add a Repository

```bash
sm repo add github.com/jroslaniec/agent-skills
```

### Enable/Disable Skills

```bash
# Enable using full reference
sm skill enable github.com/jroslaniec/agent-skills/git-commit

# Or just the skill name if already registered
sm skill enable git-commit

# Disable (works with both name and full reference)
sm skill disable git-commit
```

### Auxiliary Commands

```bash
# List skills
sm skills list

# List repositories
sm repo list

# Remove a repository and all its skills
sm repo delete github.com/jroslaniec/agent-skills

# Show cache directory location
sm cache dir
```

## Command Aliases

The tool supports multiple aliases for convenience:

- `repositories`, `repository`, `repos`, `repo` - Repository commands
- `skills`, `skill`, `sk` - Skill commands
