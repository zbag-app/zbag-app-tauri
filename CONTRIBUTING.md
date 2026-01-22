# Contributing to zSTASH

## Development Setup

See [CLAUDE.md](./CLAUDE.md) for build instructions and architecture overview.

## Version Control

This project uses standard **git** for version control.

### Common Workflows

**View status and changes:**
```bash
git status     # Working copy status
git log        # Commit history
git diff       # Uncommitted changes
```

**Create commits:**
```bash
git add <files>             # Stage changes
git commit -m "message"     # Create commit
```

**Work with branches:**
```bash
git checkout -b my-branch   # Create and switch to branch
git push -u origin my-branch # Push branch to remote
```

**Update from remote:**
```bash
git pull --rebase           # Update and rebase local changes
git fetch                   # Fetch without merging
```

**Undo mistakes:**
```bash
git stash                   # Temporarily store changes
git stash pop               # Restore stashed changes
git reset HEAD~1            # Undo last commit (keep changes)
```

## Commit Messages

Follow existing patterns:
- `US<N>: ...` - User story work
- `docs: ...` - Documentation
- `chore: ...` - Maintenance
- `fix: ...` - Bug fixes

## Pull Requests

1. Create a branch: `git checkout -b fix/description`
2. Make changes and commit: `git commit -m "fix: description"`
3. Push: `git push -u origin fix/description`
4. Open PR on GitHub linking relevant issue

## Code Quality

Before submitting:
```bash
make pre-commit   # Format + lint
make test         # Run tests
make tauri-build  # Full build verification
```

See [AGENTS.md](./AGENTS.md) for detailed guidelines.
