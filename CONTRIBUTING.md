# Contributing to zSTASH

## Development Setup

See [CLAUDE.md](./CLAUDE.md) for build instructions and architecture overview.

## Version Control

This project uses **jj (Jujutsu)** colocated on Git for version control.

### Why jj?
- First-class undo/redo for all operations
- Automatic rebasing and conflict resolution
- Working copy as a commit (no staging area)
- Git-compatible (existing remotes/CI work unchanged)

### Getting Started with jj

**Install jj:**
```bash
# macOS
brew install jj

# Linux
cargo install --locked jj-cli

# Other platforms: https://docs.jj-vcs.dev/latest/install-and-setup/
```

**Initialize (already done for this repo):**
```bash
jj git init --colocate
```

### Common Workflows

**View status and changes:**
```bash
jj status      # Working copy status
jj log         # Commit history
jj diff        # Uncommitted changes
```

**Create commits:**
```bash
jj new                      # Start new change
jj describe -m "message"    # Set commit message
```

**Work with bookmarks (branches):**
```bash
jj bookmark create my-branch  # Create bookmark at current change
jj git push                   # Push to remote
```

**Undo mistakes:**
```bash
jj undo                     # Undo last operation
jj op log                   # View operation history
```

**Modify previous changes:**
```bash
jj edit <change>            # Edit a previous change
jj squash                   # Squash current change into parent
```

**Note:** Standard Git commands still work since jj colocates with `.git/`.

## Commit Messages

Follow existing patterns:
- `US<N>: ...` - User story work
- `docs: ...` - Documentation
- `chore: ...` - Maintenance
- `fix: ...` - Bug fixes

## Pull Requests

1. Create a bookmark: `jj bookmark create fix/description`
2. Make changes and describe: `jj describe -m "fix: description"`
3. Push: `jj git push --bookmark fix/description`
4. Open PR on GitHub linking relevant issue

## Code Quality

Before submitting:
```bash
make pre-commit   # Format + lint
make test         # Run tests
make tauri-build  # Full build verification
```

See [AGENTS.md](./AGENTS.md) for detailed guidelines.
