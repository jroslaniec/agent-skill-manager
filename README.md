# Agent Skill Manager

A CLI tool for managing [Agent Skills](https://agentskills.io).

**Current Limitations**:

- Only GitHub repositories are supported
- Only Claude Code is supported
- Windows is not supported (uses Unix symlinks)

## How it works

The tool clones repositories containing skills to a local cache directory. When you enable a skill, it creates a symlink from `~/.claude/skills/<skill-name>` to the cached repository location. Disabling a skill removes the symlink while keeping the repository cached for fast re-enabling.

## Installation

### Installer Script (macOS and Linux)

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/jroslaniec/agent-skill-manager/releases/latest/download/agent-skill-manager-installer.sh | sh
```

### From Source

```bash
cargo install --path .
```

## Usage

### Interactive Mode

The easiest way to manage skills is through the interactive UI:

```bash
# Manage all skills from all repositories
sm

# Add repository and interactively select skills
sm add -i github.com/anthropics/skills/skills
```

### Add Skills

Add skills to your global Claude Code configuration:

```bash
# Add a skill
sm add github.com/anthropics/skills/skills/pdf

# Add multiple skills
sm add \
  github.com/anthropics/skills/skills/pdf \
  github.com/anthropics/skills/skills/docx

# Interactive mode - select skills from a repository
sm add -i github.com/anthropics/skills/skills
```

### Add a Repository

If you want to add a repository without enabling skills, use:

```bash
sm repo add github.com/anthropics/skills/skills
```

Repositories should contain skill directories, each with a `SKILL.md` file:
```txt
repo-root/
├── skill-name-1/
│   └── SKILL.md
├── skill-name-2/
│   └── SKILL.md
```

You can also add a repository from a nested path:
```bash
sm repo add github.com/owner/repo/path/to/skills
```

Example with nested structure:
```txt
repo-root/
├── tools/
│   └── agent-skills/
│       ├── skill-a/
│       │   └── SKILL.md
│       └── skill-b/
│           └── SKILL.md
```
For this structure, use: `sm repo add github.com/owner/repo/tools/agent-skills`

### Repository Version Management

Pin repositories to specific commits to prevent accidental updates:

```bash
# Add repository pinned to specific commit
sm repo add github.com/owner/repo@abc12345

# Pin existing repository to current commit
sm repo pin github.com/owner/repo

# Unpin to allow upgrades
sm repo unpin github.com/owner/repo

# Upgrade to latest commit
sm repo upgrade github.com/owner/repo

# Upgrade to specific commit (auto-pins)
sm repo upgrade github.com/owner/repo@abc12345

# Upgrade all unpinned repositories
sm upgrade
sm upgrade --force  # Skip confirmation
```

### Enable/Disable Skills

```bash
# Interactive mode (easiest)
sm

# Enable using full reference
sm skill enable github.com/anthropics/skills/skills/pdf
sm enable github.com/anthropics/skills/skills/pdf

# Or just the skill name if already registered
sm skill enable pdf
sm enable pdf

# Disable (works with both name and full reference)
sm skill disable pdf
sm disable pdf
```

### Auxiliary Commands

```bash
# List skills
sm list
sm list --all

# List repositories
sm repo list

# Remove a repository and all its skills
sm repo delete github.com/anthropics/skills/skills

# Show cache directory location
sm cache dir

# Purge everything (remove all repositories, skills, and cache)
sm purge
```

## Command Aliases

The tool supports multiple aliases for convenience:

- `repositories`, `repository`, `repos`, `repo` - Repository commands
- `skills`, `skill`, `sk` - Skill commands
- `list`, `ls` - Shortcut for `skills list`
- `enable` - Shortcut for `skills enable`
- `disable` - Shortcut for `skills disable`

## TODO

- [ ] Add support for other coding agents (OpenCode, Codex, etc.)
- [ ] Add Windows support
- [ ] Add support for other repository platforms (GitLab, Bitbucket, etc.)
