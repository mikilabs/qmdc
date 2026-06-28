.PHONY: lint test check build clean bump bump-major bump-minor bump-patch

# Note: warnings=errors is set globally in .cargo/config.toml

build:
	cargo build --release

lint:
	cargo clippy -- -D warnings

test:
	@echo "Running all tests (unit, cli, microtests, workspace)..."
	cargo test

test-parser:
	cargo test --test parser

test-cli:
	cargo test --test cli_conformance --test cli_unit

test-workspace:
	cargo test --test workspace_conformance --test workspace_unit

check: lint test

clean:
	cargo clean

all: check

# ============ Version management ============

bump: bump-patch

bump-major:
	bash scripts/bump_version.sh major

bump-minor:
	bash scripts/bump_version.sh minor

bump-patch:
	bash scripts/bump_version.sh patch
