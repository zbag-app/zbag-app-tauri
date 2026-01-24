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

## Testing CI Locally

Use [act](https://github.com/nektos/act) to run GitHub Actions workflows locally before pushing.

### Installation

```bash
brew install act    # macOS
# See https://github.com/nektos/act#installation for other platforms
```

### Running Workflows

The CI uses `self-hosted` runners, so specify an image:

```bash
# Run the bun-tests job (fastest for frontend changes)
act -j bun-tests -P self-hosted=catthehacker/ubuntu:act-22.04

# Run the rust job (full Rust CI: audit, clippy, tests, e2e)
act -j rust -P self-hosted=catthehacker/ubuntu:rust-22.04

# Run with environment variables
act -j rust -P self-hosted=catthehacker/ubuntu:rust-22.04 --env ZSTASH_GRPC_URL=https://lwd.testnet.zec.pro

# Dry run (validate workflow syntax without executing)
act -n
```

**Image notes:**
- `rust-22.04`: Has Rust 1.x, clippy, rustfmt pre-installed (~2GB)
- `act-22.04`: Base Ubuntu image (~1GB) - Bun 1.3.5 installed via workflow action
- First run downloads container images; subsequent runs use cached images

### Requirements

- Docker must be running
- First run downloads container images (~2GB)

### Limitations

- Some GitHub-specific features (caching, artifacts) may not work identically
- E2E tests require Playwright browsers installed in the container
- For quick syntax validation, use `actionlint` instead: `brew install actionlint && actionlint`

See [AGENTS.md](./AGENTS.md) for detailed guidelines.
