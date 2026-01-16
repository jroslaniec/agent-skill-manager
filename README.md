# Agent Skill Manager

A CLI tool for managing [Agent Skills](https://agentskills.io).

**Supported Coding Agents**: Claude Code, OpenCode, Codex, Gemini CLI (+ custom integrations)

**Current Limitations**:

- Only GitHub repositories are supported
- Windows is not supported (uses Unix symlinks)

## How it works

The tool clones repositories containing skills to a local cache directory. When you enable a skill, it creates symlinks in all your registered coding agents' skill directories. Disabling a skill removes the symlinks while keeping the repository cached for fast re-enabling.

## Installation

### Installer Script (macOS and Linux)

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/jroslaniec/agent-skill-manager/releases/latest/download/agent-skill-manager-installer.sh | sh
```

### From Source

```bash
cargo install --path .
```

## Quick Start

After installation, configure which coding agents you use:

```bash
# Interactive setup - select your coding agents
sm configure

# Or add individually
sm integrations add claude-code
sm integrations add opencode
```

Then add skills:

```bash
sm add github.com/anthropics/skills/skills/pdf
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

### Manage Integrations

Configure which coding agents receive your skills:

```bash
# Interactive setup
sm configure

# Add built-in integrations
sm integrations add claude-code    # ~/.claude/skills/
sm integrations add opencode       # ~/.config/opencode/skill/
sm integrations add codex          # ~/.codex/skills/
sm integrations add gemini-cli     # ~/.gemini/skills/

# Add custom integration
sm integrations add cursor --path ~/.cursor/skills

# List integrations (shows all presets + custom)
sm integrations list

# Remove an integration
sm integrations remove cursor
```

Aliases are supported: `claude` → `claude-code`, `gemini` → `gemini-cli`

## Command Aliases

The tool supports multiple aliases for convenience:

- `repositories`, `repository`, `repos`, `repo` - Repository commands
- `skills`, `skill`, `sk` - Skill commands
- `integrations`, `integration`, `int` - Integration commands
- `list`, `ls` - Shortcut for `skills list`
- `enable` - Shortcut for `skills enable`
- `disable` - Shortcut for `skills disable`
- `configure`, `config` - Configure integrations

## TODO

- [ ] Add Windows support
- [ ] Add support for other repository platforms (GitLab, Bitbucket, etc.)
