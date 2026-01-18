# Patterns

- Use `SkillRef::parse()` for GitHub-specific skill references (github.com/owner/repo/skill format)
- Use `RepoRef::parse()` for universal git URLs (any HTTPS, SSH, or local paths)
- When an operation fails due to wrong input type, detect the correct type and suggest the appropriate command
- Use `resolve_repo_cache_path()` for existing repos (handles legacy paths), `repo_cache_path()` for new clones
- CLI short flags should be consistent across similar commands (e.g., `-a` means `--all` in all list commands)
- Determine git source type from URL string: `git@` = SSH, `/` or `file://` = local, otherwise = HTTPS
- RepoRef::parse() can parse both raw user input and repo_ids stored in config (including `local:` prefixed format)
