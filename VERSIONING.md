# Versioning Policy

zSTASH Desktop follows [Semantic Versioning 2.0.0](https://semver.org/).

## Version Format

```
MAJOR.MINOR.PATCH
```

- **MAJOR**: Incompatible API changes, breaking wallet format changes
- **MINOR**: New features, backward-compatible functionality
- **PATCH**: Bug fixes, security patches, backward-compatible changes

## Tag Format

Git tags use the `vX.Y.Z` format:

```
v0.1.0
v1.0.0
v1.2.3
```

## Version Sources

The canonical version is defined in the workspace `Cargo.toml`:

```toml
[workspace.package]
version = "0.1.0"
```

All crates inherit this version via `version.workspace = true`.

The following files reference the version and must stay in sync:

| File | Field |
|------|-------|
| `Cargo.toml` | `[workspace.package].version` |
| `apps/zstash-app-tauri/src-tauri/tauri.conf.json` | `version` |
| `apps/zstash-app-tauri/package.json` | `version` |

## Versioned Components

### Rust Crates
- `zstash-core`
- `zstash-engine`
- `zstash-network`
- `zstash-keystone`
- `zstash-tor`
- `zstash-app-tauri`
- `zstash-cli`

### Frontend
- `apps/zstash-app-tauri/package.json`

### Tauri App
- `apps/zstash-app-tauri/src-tauri/tauri.conf.json`

## Release Process

1. Update version in `Cargo.toml` (workspace)
2. Update version in `tauri.conf.json`
3. Update version in `package.json`
4. Commit: `chore: bump version to X.Y.Z`
5. Tag: `git tag vX.Y.Z`
6. Push with tags: `git push && git push --tags`

## Version in App

The version is surfaced in:

- **UI**: Settings > About section shows version and git commit
- **Logs**: Version and git commit logged at startup

Example log entry:
```
INFO zSTASH Desktop starting version=0.1.0 git_commit=abc1234
```

## Git Commit Hash

The short git commit hash is captured at build time and included in:

- Log output at startup
- UI (Settings > About)
- `VersionInfo` IPC response

This helps identify exact builds during debugging and support.

## CI/CD Integration

The CI workflow uses tags as the source of truth for releases:

- Tags trigger release builds
- Tag name determines release version
- Release artifacts are named with the version

## IPC Version Contract

The IPC schema version (`SCHEMA_VERSION = 1`) is separate from the app version.
IPC version changes follow their own policy documented in the IPC types.
