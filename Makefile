# Makefile for ghop
# Usage: make <target>
# Many targets accept variables to pass extra arguments, e.g.:
#   make run ARGS="echo hello"
#   make tui CMDS="echo one"   # multiple commands can be quoted with spaces

SHELL := /bin/sh

# Binary/package name
BIN := ghop

# Docker params
DOCKER ?= docker
IMAGE ?= ghop
TAG ?= latest
FILE ?= ghop.yml
SET ?= dev

# Default target
.DEFAULT_GOAL := help

.PHONY: help build release run tui test check fmt clippy doc install uninstall clean ci docker-build docker-run-help docker-run docker-dev

help: ## Show this help
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z0-9_.-]+:.*##/ { printf "\033[36m%-18s\033[0m %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

build: ## Build debug binary
	cargo build

release: ## Build optimized release binary
	cargo build --release && strip target/release/$(BIN) && ls -lh target/release/$(BIN)

run: ## Run the app (pass args with ARGS="...")
	cargo run -- $(ARGS)

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

# --- Docker targets ---

docker-build: ## Build Docker image (override IMAGE and TAG as needed)
	$(DOCKER) build --progress plain -t $(IMAGE):$(TAG) .

docker-run-help: ## Run container to show ghop --help
	$(DOCKER) run --rm -it $(IMAGE):$(TAG) --help

docker-run: ## Run container and pass ARGS to ghop (e.g., ARGS="--version")
	$(DOCKER) run --rm -it $(IMAGE):$(TAG) $(ARGS)

# Convenience target: mount current dir and run a set from FILE and SET
# Example: make docker-dev SET=dev FILE=ghop.yml

docker-dev: ## Run with current dir mounted: -f $(FILE) $(SET)
	$(DOCKER) run --rm -it -v "$$PWD":/work $(IMAGE):$(TAG) -f $(FILE) $(SET)
