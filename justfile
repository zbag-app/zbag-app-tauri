tauri_dir := "apps/zbag-app-tauri"

default:
    just --list

# Install frontend dependencies.
install:
    make install

# Build Rust library crates.
build:
    make build

# Run Rust library tests.
test:
    make test

# Format Rust code.
fmt:
    make fmt

# Check Rust formatting.
fmt-check:
    make fmt-check

# Run clippy lints.
clippy:
    make clippy

# Run clippy with warnings as errors.
clippy-strict:
    make clippy-strict

# Pre-commit checks: format, lint, and local guardrails.
pre-commit:
    make pre-commit

# Compatibility alias for older muscle memory.
precommit: pre-commit

# CI-like checks without building the final Tauri bundle.
check:
    make check

# Build frontend assets.
frontend-build:
    make build-frontend

# Run frontend unit tests.
frontend-test:
    cd {{tauri_dir}} && bun run test

# Run the Tauri desktop app in development mode.
app-dev:
    make dev

# Build the Tauri desktop app bundle.
app-build:
    make tauri-build

# Run CEF network smoke parser fixtures.
smoketest-selftest:
    make cef-smoketest-selftest

# Run CEF network smoke against an existing packaged app.
smoketest:
    make cef-smoketest

# Report CEF and bundle sizes.
cef-audit:
    make cef-audit

# Run cargo audit.
audit:
    make audit

# Regenerate CHANGELOG.md from git history.
changelog:
    make changelog

# Preview unreleased changelog entries.
changelog-unreleased:
    make changelog-unreleased

# Scan for secrets when gitleaks is installed.
gitleaks:
    @if command -v gitleaks >/dev/null 2>&1; then \
      gitleaks detect --source .; \
    else \
      echo "gitleaks not installed; skipping"; \
    fi

# CI-like local verification, including app bundle build.
verify:
    make check
    cd {{tauri_dir}} && bun run test
    make tauri-build

# Remove build artifacts.
clean:
    make clean-all
