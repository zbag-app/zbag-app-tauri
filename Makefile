# Makefile for zSTASH Desktop
# Tauri v2 + Rust workspace

.DEFAULT_GOAL := help

TAURI_DIR := apps/zstash-app-tauri

.PHONY: help install build build-release build-frontend \
        test test-engine test-core test-network test-keystone test-tor test-migrations \
        test-e2e test-bridge test-bridge-build \
        fmt fmt-check clippy clippy-strict lint \
        pre-commit check audit check-telemetry \
        dev tauri-build \
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
	@cargo build --workspace --exclude zstash-app-tauri

build-release: ## Production release build (libs)
	@cargo build --release --locked --workspace --exclude zstash-app-tauri

build-frontend: ## Build frontend dist (for Tauri)
	@cd $(TAURI_DIR) && bun run build

# ============================================================================
# Test (Rust)
# ============================================================================

test: ## Run all Rust library tests
	@cargo test --workspace --exclude zstash-app-tauri --exclude zstash-core
	@cargo test -p zstash-core --features async

test-engine: ## Test zstash-engine crate
	@cargo test -p zstash-engine

test-core: ## Test zstash-core crate
	@cargo test -p zstash-core --features async

test-network: ## Test zstash-network crate
	@cargo test -p zstash-network

test-keystone: ## Test zstash-keystone crate
	@cargo test -p zstash-keystone

test-tor: ## Test zstash-tor crate
	@cargo test -p zstash-tor

test-migrations: ## Run migration tests
	@cargo test -p zstash-engine --test app_db_migrations --test wallet_db_encryption_and_migrations

# ============================================================================
# E2E Testing
# ============================================================================

test-e2e: install ## Run Playwright E2E tests (starts test bridge automatically)
	@./scripts/e2e-test.sh

test-bridge-build: ## Build the test bridge server
	@cargo build -p zstash-app-tauri --features test-bridge

test-bridge: test-bridge-build ## Run the test bridge server
	@cargo run -p zstash-app-tauri --features test-bridge

# ============================================================================
# Lint/Format (Rust)
# ============================================================================

fmt: ## Format Rust code
	@cargo fmt --all

fmt-check: ## Check Rust formatting (CI)
	@cargo fmt --all -- --check

clippy: ## Run clippy lints
	@cargo clippy --workspace --all-targets --exclude zstash-app-tauri

clippy-strict: ## Clippy with warnings as errors
	@cargo clippy --workspace --all-targets --exclude zstash-app-tauri -- -D warnings

lint: fmt-check clippy ## Run all lints

# ============================================================================
# Pre-commit/CI
# ============================================================================

pre-commit: fmt clippy ## Pre-commit checks (formats and lints)

check: fmt-check clippy-strict test ## CI-like checks (no mutations)

audit: ## Security audit
	@cargo audit

check-telemetry: ## Check for telemetry code
	@./scripts/check-no-telemetry.sh

# ============================================================================
# Tauri
# ============================================================================

dev: ## Full Tauri development
	@cd $(TAURI_DIR) && bun run tauri dev

# Default CI=true avoids macOS DMG bundling detach/unmount flakiness (create-dmg can fail with EBUSY); override with CI=false if needed.
tauri-build: ## Tauri production build
	@cd $(TAURI_DIR) && CI=$${CI:-true} bun run tauri build

# ============================================================================
# CLI
# ============================================================================

CLI_RELEASE := ./target/release/zstash
CLI_DEBUG := ./target/debug/zstash

cli: ## Build zstash-cli binary (release)
	@cargo build --release -p zstash-cli

cli-dev: ## Build zstash-cli binary (debug)
	@cargo build -p zstash-cli

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
	@echo "zSTASH Desktop - Makefile targets"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-18s %s\n", $$1, $$2}'
