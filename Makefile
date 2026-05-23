# Makefile for bagZ Desktop
# Tauri v2 + Rust workspace

.DEFAULT_GOAL := help

TAURI_DIR := apps/bagz-app-tauri
UNAME_S := $(shell uname -s)
TAURI_FEATURES ?= cef-runtime
TAURI_BUNDLES ?=
CEF_STAGE_ROOT ?= $(CURDIR)/target/cef-stage
CEF_SAFE_LOCALES ?= en.lproj,en_GB.lproj

# Force explicit macOS bundle outputs so build failures surface during DMG packaging.
ifeq ($(UNAME_S),Darwin)
TAURI_BUNDLES := app,dmg
endif

.PHONY: help install build build-release build-frontend \
        test test-engine test-core test-network test-keystone test-tor test-migrations \
        test-e2e test-bridge test-bridge-build \
        fmt fmt-check clippy clippy-strict lint \
        pre-commit check audit check-telemetry check-cef-network-hardening check-cef-args cef-smoketest-selftest cef-smoketest \
        dev tauri-build cef-audit cef-stage-safe cef-stage-aggressive tauri-build-slim-safe tauri-build-slim-aggressive \
        cli cli-dev cli-run cli-wallet-list cli-wallet-create cli-sync cli-balance cli-address \
        changelog changelog-unreleased \
        clean clean-frontend clean-all

# ============================================================================
# Setup
# ============================================================================

install: ## Install frontend dependencies
	@cd $(TAURI_DIR) && bun install

# ============================================================================
# Build (Rust)
# ============================================================================

build: ## Build Rust library crates
	@cargo build --workspace --exclude bagz-app-tauri --exclude bagz-xtask

build-release: ## Production release build (libs)
	@cargo build --release --locked --workspace --exclude bagz-app-tauri --exclude bagz-xtask

build-frontend: ## Build frontend dist (for Tauri)
	@cd $(TAURI_DIR) && bun run build

# ============================================================================
# Test (Rust)
# ============================================================================

test: ## Run all Rust library tests
	# `bagz-core` includes async-gated tests (`--features async`). Running it separately keeps the
	# rest of the workspace tests feature-default while still exercising the async code path.
	@cargo test --workspace --exclude bagz-app-tauri --exclude bagz-xtask --exclude bagz-core
	@cargo test -p bagz-core --features async

test-engine: ## Test bagz-engine crate
	@cargo test -p bagz-engine

test-core: ## Test bagz-core crate
	@cargo test -p bagz-core --features async

test-network: ## Test bagz-network crate
	@cargo test -p bagz-network

test-keystone: ## Test bagz-keystone crate
	@cargo test -p bagz-keystone

test-tor: ## Test bagz-tor crate
	@cargo test -p bagz-tor

test-migrations: ## Run migration tests
	@cargo test -p bagz-engine --test app_db_migrations --test wallet_db_encryption_and_migrations

# ============================================================================
# E2E Testing
# ============================================================================

test-e2e: install ## Run Playwright E2E tests (starts test bridge automatically)
	@./scripts/e2e-test.sh

test-bridge-build: ## Build the test bridge server
	@cargo build -p bagz-app-tauri --features test-bridge

test-bridge: test-bridge-build ## Run the test bridge server
	@cargo run -p bagz-app-tauri --features test-bridge

# ============================================================================
# Lint/Format (Rust)
# ============================================================================

fmt: ## Format Rust code
	@cargo fmt --all

fmt-check: ## Check Rust formatting (CI)
	@cargo fmt --all -- --check

clippy: ## Run clippy lints
	@cargo clippy --workspace --all-targets --exclude bagz-app-tauri --exclude bagz-xtask

clippy-strict: ## Clippy with warnings as errors
	@cargo clippy --workspace --all-targets --exclude bagz-app-tauri --exclude bagz-xtask -- -D warnings

lint: fmt-check clippy ## Run all lints

# ============================================================================
# Pre-commit/CI
# ============================================================================

pre-commit: fmt clippy check-telemetry check-cef-network-hardening check-cef-args cef-smoketest-selftest ## Pre-commit checks (formats, lints, and local guardrails)

check: fmt-check clippy-strict check-telemetry check-cef-network-hardening check-cef-args cef-smoketest-selftest test ## CI-like checks (no mutations)

audit: ## Security audit
	@cargo audit

check-telemetry: ## Check for telemetry code
	@./scripts/check-no-telemetry.sh

check-cef-network-hardening: ## Check CEF browser network hardening guardrails
	@./scripts/check-cef-network-hardening.sh

check-cef-args: ## Test parsed CEF runtime arguments
	@cargo test -p bagz-app-tauri --features cef-runtime --test cef_runtime_args

# ============================================================================
# Tauri
# ============================================================================

dev: ## Full Tauri development
	@cd $(TAURI_DIR) && bun run tauri dev --features $(TAURI_FEATURES)

# Default CI=true avoids macOS DMG bundling detach/unmount flakiness (create-dmg can fail with EBUSY); override with CI=false if needed.
tauri-build: ## Tauri production build
	@CI=$${CI:-true} TAURI_FEATURES="$(TAURI_FEATURES)" TAURI_BUNDLES="$(TAURI_BUNDLES)" ./scripts/tauri-cef-build.sh

cef-smoketest-selftest: ## Run CEF network smoke parser fixtures
	@cargo run -p bagz-xtask --quiet -- cef-smoketest --selftest

# Requires a prebuilt bundle at target/release/bundle/macos/bagZ.app.
cef-smoketest: ## Run CEF network smoke against an existing packaged app
	@cargo run -p bagz-xtask --quiet -- cef-smoketest

cef-audit: ## Report CEF + bundle sizes and largest payload entries
	@./scripts/cef-size-report.sh

cef-stage-safe: ## Stage pruned CEF payload (safe profile)
	@CEF_SLIM_PROFILE=safe CEF_KEEP_LOCALES="$(CEF_SAFE_LOCALES)" CEF_STAGE_ROOT="$(CEF_STAGE_ROOT)" ./scripts/cef-stage-slim.sh

cef-stage-aggressive: ## Stage pruned CEF payload (aggressive profile)
	@CEF_SLIM_PROFILE=aggressive CEF_KEEP_LOCALES="$(CEF_SAFE_LOCALES)" CEF_STAGE_ROOT="$(CEF_STAGE_ROOT)" ./scripts/cef-stage-slim.sh

tauri-build-slim-safe: ## Tauri production build using staged SAFE slim CEF
	@STAGED_CEF_BASE=$$(CEF_SLIM_PROFILE=safe CEF_KEEP_LOCALES="$(CEF_SAFE_LOCALES)" CEF_STAGE_ROOT="$(CEF_STAGE_ROOT)" ./scripts/cef-stage-slim.sh --print-cef-base --quiet); \
		echo "info: using staged SAFE CEF base: $$STAGED_CEF_BASE"; \
		CI=$${CI:-true} CEF_PATH="$$STAGED_CEF_BASE" TAURI_FEATURES="$(TAURI_FEATURES)" TAURI_BUNDLES="$(TAURI_BUNDLES)" ./scripts/tauri-cef-build.sh

tauri-build-slim-aggressive: ## Tauri production build using staged AGGRESSIVE slim CEF
	@STAGED_CEF_BASE=$$(CEF_SLIM_PROFILE=aggressive CEF_KEEP_LOCALES="$(CEF_SAFE_LOCALES)" CEF_STAGE_ROOT="$(CEF_STAGE_ROOT)" ./scripts/cef-stage-slim.sh --print-cef-base --quiet); \
		echo "info: using staged AGGRESSIVE CEF base: $$STAGED_CEF_BASE"; \
		CI=$${CI:-true} CEF_PATH="$$STAGED_CEF_BASE" TAURI_FEATURES="$(TAURI_FEATURES)" TAURI_BUNDLES="$(TAURI_BUNDLES)" ./scripts/tauri-cef-build.sh

# ============================================================================
# CLI
# ============================================================================

CLI_RELEASE := ./target/release/bagz
CLI_DEBUG := ./target/debug/bagz

cli: ## Build bagz-cli binary (release)
	@cargo build --release -p bagz-cli

cli-dev: ## Build bagz-cli binary (debug)
	@cargo build -p bagz-cli

cli-run: cli ## Run CLI with ARGS (e.g., make cli-run ARGS="wallet list")
	@$(CLI_RELEASE) $(ARGS)

cli-wallet-list: cli ## List all wallets
	@$(CLI_RELEASE) wallet list

cli-wallet-create: cli ## Create a new wallet (interactive)
	@$(CLI_RELEASE) wallet create

cli-sync: cli ## Sync wallet (requires WALLET=<name>)
	@test -n "$(WALLET)" || (echo "Error: WALLET is required (e.g., make cli-sync WALLET=mywallet)" && exit 1)
	@$(CLI_RELEASE) sync --wallet $(WALLET)

cli-balance: cli ## Check balance (requires WALLET=<name>)
	@test -n "$(WALLET)" || (echo "Error: WALLET is required (e.g., make cli-balance WALLET=mywallet)" && exit 1)
	@$(CLI_RELEASE) balance --wallet $(WALLET)

cli-address: cli ## Get receive address (requires WALLET=<name>)
	@test -n "$(WALLET)" || (echo "Error: WALLET is required (e.g., make cli-address WALLET=mywallet)" && exit 1)
	@$(CLI_RELEASE) address --wallet $(WALLET)

# ============================================================================
# Changelog
# ============================================================================

changelog: ## Generate CHANGELOG.md from git history
	@git-cliff --output CHANGELOG.md

changelog-unreleased: ## Preview unreleased changes
	@git-cliff --unreleased

# ============================================================================
# Clean
# ============================================================================

clean: ## Clean Rust build artifacts
	@cargo clean

clean-frontend: ## Clean frontend dist
	@rm -rf $(TAURI_DIR)/dist

clean-all: clean clean-frontend ## Clean everything

# ============================================================================
# Help
# ============================================================================

help: ## Show available make targets
	@echo "bagZ Desktop - Makefile targets"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-18s %s\n", $$1, $$2}'
