# Makefile for ghop
# Usage: make <target>
# Many targets accept variables to pass extra arguments, e.g.:
#   make run ARGS="echo hello"
#   make tui CMDS="echo one"   # multiple commands can be quoted with spaces

SHELL := /bin/sh

# Binary/package name
BIN := ghop

# Default target
.DEFAULT_GOAL := help

.PHONY: help build release run tui test check fmt clippy doc install uninstall clean ci

help: ## Show this help
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z0-9_.-]+:.*##/ { printf "\033[36m%-18s\033[0m %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

build: ## Build debug binary
	cargo build

release: ## Build optimized release binary
	cargo build --release && strip target/release/$(BIN) && ls -lh target/release/$(BIN)

run: ## Run the app (pass args with ARGS="...")
	cargo run -- $(ARGS)

# Run in TUI mode. Provide shell commands separated by ; or as quoted strings via CMDS
# Example: make tui CMDS='"echo one" "sleep 1; echo two"'
# Simpler: make tui CMDS="echo one"
# On Windows, commands are routed through cmd; on Unix, sh -c; handled by the app itself.
# We forward as-is; multiple commands must be provided as separate words.
# e.g.: make tui CMDS='echo one' or CMDS='"echo one" "echo two"'
# Note: make splits on spaces; quote appropriately.

tui: ## Run in TUI mode (pass commands with CMDS="...")
	cargo run -- -t -- $(CMDS)

test: ## Run test suite
	cargo test

check: ## Type-check without building artifacts
	cargo check

fmt: ## Format the code
	cargo fmt --all

clippy: ## Lint with clippy (deny warnings)
	cargo clippy --all-targets --all-features -- -D warnings

doc: ## Build documentation
	cargo doc --no-deps

install: ## Install the binary locally
	cargo install --path . --force

uninstall: ## Uninstall the binary
	cargo uninstall $(BIN) || true

clean: ## Clean build artifacts
	cargo clean

ci: ## CI pipeline: fmt check (diff), clippy strict, tests
	@echo "==> Checking formatting"
	@cargo fmt --all -- --check
	@echo "==> Clippy"
	@cargo clippy --all-targets --all-features -- -D warnings
	@echo "==> Tests"
	@cargo test
