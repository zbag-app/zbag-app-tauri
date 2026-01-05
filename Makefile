# Makefile for Zkore Desktop
# Tauri v2 + Rust workspace

.DEFAULT_GOAL := help

TAURI_DIR := apps/zkore-app-tauri

.PHONY: help install build build-release build-frontend build-tui \
        test test-engine test-core test-network test-keystone test-tor test-migrations \
        fmt fmt-check clippy clippy-strict lint \
        pre-commit check audit check-telemetry \
        dev tauri-build tui tui-release \
        cli cli-dev cli-run cli-wallet-list cli-wallet-create cli-sync cli-balance cli-address \
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
	@cargo build --workspace --exclude zkore-app-tauri

build-release: ## Production release build (libs)
	@cargo build --release --locked --workspace --exclude zkore-app-tauri

build-frontend: ## Build frontend dist (for Tauri)
	@cd $(TAURI_DIR) && bun run build

# ============================================================================
# Test (Rust)
# ============================================================================

test: ## Run all Rust library tests
	@cargo test --workspace --exclude zkore-app-tauri

test-engine: ## Test zkore-engine crate
	@cargo test -p zkore-engine

test-core: ## Test zkore-core crate
	@cargo test -p zkore-core

test-network: ## Test zkore-network crate
	@cargo test -p zkore-network

test-keystone: ## Test zkore-keystone crate
	@cargo test -p zkore-keystone

test-tor: ## Test zkore-tor crate
	@cargo test -p zkore-tor

test-migrations: ## Run migration tests
	@cargo test -p zkore-engine --test app_db_migrations --test wallet_db_encryption_and_migrations

# ============================================================================
# Lint/Format (Rust)
# ============================================================================

fmt: ## Format Rust code
	@cargo fmt --all

fmt-check: ## Check Rust formatting (CI)
	@cargo fmt --all -- --check

clippy: ## Run clippy lints
	@cargo clippy --workspace --all-targets --exclude zkore-app-tauri

clippy-strict: ## Clippy with warnings as errors
	@cargo clippy --workspace --all-targets --exclude zkore-app-tauri -- -D warnings

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

tauri-build: ## Tauri production build
	@cd $(TAURI_DIR) && bun run tauri build

# ============================================================================
# TUI
# ============================================================================

build-tui: ## Build zkore-tui binary
	@cargo build -p zkore-tui

tui: ## Run zkore-tui
	@cargo run -p zkore-tui

tui-release: ## Run zkore-tui (release build)
	@cargo run --release -p zkore-tui

# ============================================================================
# CLI
# ============================================================================

CLI_RELEASE := ./target/release/zkore
CLI_DEBUG := ./target/debug/zkore

cli: ## Build zkore-cli binary (release)
	@cargo build --release -p zkore-cli

cli-dev: ## Build zkore-cli binary (debug)
	@cargo build -p zkore-cli

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
	@echo "Zkore Desktop - Makefile targets"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-18s %s\n", $$1, $$2}'
