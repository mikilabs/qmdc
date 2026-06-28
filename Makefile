.PHONY: init help build test test-fast lint format clean install check speedtest validate-compare
.PHONY: py-build py-test py-lint py-format py-install py-bump qmdc-py
.PHONY: ts-build ts-test ts-lint ts-format ts-install ts-bump qmdc-ts
.PHONY: rs-build rs-test rs-lint rs-format rs-bump qmdc-rs
.PHONY: ext-build ext-install ext-bump ext-test ext-package ext-release mermaid-sync guide-sync
.PHONY: site site-build site-serve site-regenerate site-build-strict site-deploy mkdocs-test
.PHONY: mkdocs-bump mkdocs-bump-major mkdocs-bump-minor mkdocs-bump-patch mkdocs-release
.PHONY: semantic-index semantic-audit semantic-test semantic-hints semantic-refresh
.PHONY: bump bump-major bump-minor bump-patch
.PHONY: binary-bump binary-bump-major binary-bump-minor binary-bump-patch
.PHONY: semantic-bump semantic-bump-major semantic-bump-minor semantic-bump-patch semantic-release publish publish-check dist
.PHONY: md-lint test-report reports-clean

# ============================================================================
# INIT — reproducible environment from scratch
# ============================================================================

init:
	@echo ""
	@echo "═══════════════════════════════════════════════════════════════"
	@echo "  Initializing development environment"
	@echo "═══════════════════════════════════════════════════════════════"
	@echo ""
	@echo "--- Checking prerequisites ---"
	@command -v uv >/dev/null 2>&1 || { echo "❌ uv not found. Install: https://docs.astral.sh/uv/getting-started/installation/"; exit 1; }
	@command -v node >/dev/null 2>&1 || { echo "❌ node not found. Install Node.js 18+"; exit 1; }
	@command -v npm >/dev/null 2>&1 || { echo "❌ npm not found. Install Node.js 18+"; exit 1; }
	@command -v cargo >/dev/null 2>&1 || { echo "❌ cargo not found. Install: https://rustup.rs/"; exit 1; }
	@echo "✅ All prerequisites found"
	@echo ""
	@echo "--- Python (uv) ---"
	cd qmdc-py && uv sync --extra dev
	@echo "✅ Python ready"
	@echo ""
	@echo "--- TypeScript (npm) ---"
	cd qmdc-ts && npm ci
	@echo "✅ TypeScript ready"
	@echo ""
	@echo "--- Rust (cargo) ---"
	@rustup component add clippy rustfmt 2>/dev/null || true
	@cargo install cargo-nextest 2>/dev/null || true
	cd qmdc-rs && cargo build
	@echo "✅ Rust ready"
	@echo ""
	@echo "--- VS Code Extension ---"
	cd qmdc-vscode && npm ci
	@echo "✅ Extension deps ready"
	@echo ""
	@echo "--- Running tests ---"
	$(MAKE) test
	@echo ""
	@echo "═══════════════════════════════════════════════════════════════"
	@echo "  ✅ Environment initialized and all tests passed!"
	@echo "═══════════════════════════════════════════════════════════════"
	@echo ""

help:
	@echo "QMDC Parser — Makefile commands:"
	@echo ""
	@echo "  Common commands:"
	@echo "    make init        - Install everything from scratch (uv, npm, cargo)"
	@echo "    make build       - Build all parsers"
	@echo "    make test        - Format + lint + tests"
	@echo "    make test-fast   - Same, but in parallel (~2x faster)"
	@echo "    make lint        - Check the code"
	@echo "    make format      - Format the code"
	@echo "    make bump        - Bump the version of all parsers (patch)"
	@echo "    make binary-bump - Binary cascade: rs+py+ts+vscode (patch)"
	@echo "    make speedtest   - Benchmark the parsers"
	@echo "    make clean       - Clean build artifacts"
	@echo ""
	@echo "  Python (py-):"
	@echo "    make py-build    - Install the Python parser"
	@echo "    make py-test     - Python tests"
	@echo "    make py-lint     - Python linting"
	@echo "    make py-format   - Python formatting"
	@echo "    make py-bump     - Bump version (patch)"
	@echo "    make qmdc-py      - Run the Python CLI (qmdc)"
	@echo ""
	@echo "  TypeScript (ts-):"
	@echo "    make ts-build    - Build the TypeScript parser"
	@echo "    make ts-test     - TypeScript tests"
	@echo "    make ts-lint     - TypeScript linting"
	@echo "    make ts-format   - TypeScript formatting"
	@echo "    make ts-bump     - Bump version (patch)"
	@echo "    make qmdc-ts      - Run the TypeScript CLI (qmdc)"
	@echo ""
	@  echo "  Rust (rs-):"
	@echo "    make rs-build    - Build the Rust parser"
	@echo "    make rs-test     - Rust tests"
	@echo "    make rs-lint     - Rust linting (clippy)"
	@echo "    make rs-bump     - Bump version (patch)"
	@echo "    make qmdc-rs      - Run the Rust CLI (qmdc)"
	@echo ""
	@echo "  VS Code Extension:"
	@echo "    make ext-bump    - Bump version (patch)"
	@echo "    make ext-build   - Build .vsix (current platform)"
	@echo "    make ext-install - Build and install into VS Code"
	@echo "    make mermaid-sync - Sync Mermaid core into the VS Code extension"
	@echo ""
	@echo "  Static Site:"
	@echo "    make site WS=./docs          - Build the static site (= site-build)"
	@echo "    make site-build WS=./docs    - Build the static site (MkDocs)"
	@echo "    make site-serve WS=./docs    - Build and serve the site (MkDocs)"
	@echo "    make mkdocs-test              - qmdc-mkdocs tests"
	@echo "    make mkdocs-bump              - Bump the qmdc-mkdocs version (patch)"
	@echo ""
	@echo "  Semantic:"
	@echo "    make semantic-index WS=./docs  - Update the semantic index"
	@echo "    make semantic-audit WS=./docs  - Audit edges vs semantics"

# ============================================================================
# COMMON COMMANDS
# ============================================================================

build: py-build ts-build rs-build
	@echo ""
	@echo "✅ All parsers built!"

test: test-report validate-docs validate-compare md-lint
	@echo ""
	@echo "✅ All tests passed!"

# Aggregate every JUnit report (pytest + nextest + TS) into one unified table.
# Fails on any failure, any vacuous (0-test) suite, or a suite below its baseline
# floor in scripts/test-baseline.json. Depends on all five suites so it runs last.
test-report: py-test ts-test rs-test mkdocs-test semantic-test vscode-test
	@uv run --no-project python scripts/test-report.py

# Purge stale JUnit reports before a run so a renamed/removed test cannot leave a
# phantom count. A prerequisite of every suite, so it runs exactly once, before
# any suite writes (even under `make -j`).
reports-clean:
	@rm -rf test-reports

# Fast parallel test: `make test-fast` runs all test suites in parallel (~2x faster).
# Prefers gmake (Homebrew GNU Make 4+) for clean grouped output via --output-sync.
GMAKE := $(shell command -v gmake 2>/dev/null)
ifdef GMAKE
test-fast:
	@echo "Using $(GMAKE) (with --output-sync)"
	@$(GMAKE) -j --output-sync=target test
else
test-fast:
	@echo "Using make (output may interleave — install gmake via brew for cleaner output)"
	@$(MAKE) -j test
endif

# Parallel-friendly test: run with `make -j test` to parallelize independent tasks.
# Dependencies encode the real graph:
#   py-test depends on py-format + py-lint (but NOT on ts/rs anything)
#   ts-test depends on ts-format + ts-lint
#   rs-test depends on rs-format + rs-lint
#   validate-docs depends on py-build (uses Python CLI)
#   validate-compare depends on py-build + ts-build + rs-build-debug

validate-docs: py-build
	@echo "=== Validating docs workspace ==="
	@ERRORS=$$(./bin/qmdc-py workspace validate docs 2>/dev/null); \
	if [ "$$ERRORS" != "[]" ]; then \
		echo "$$ERRORS"; \
		echo ""; \
		echo "❌ Docs workspace has validation errors!"; \
		exit 1; \
	else \
		echo "✅ Docs workspace is valid"; \
	fi

# Run markdownlint over docs/ — config in .markdownlint.json at repo root.
# Catches structural markdown issues that survive QMDC validation
# (e.g. unlabelled fenced code blocks, broken heading hierarchy, dead emphasis-as-headings).
md-lint:
	@echo "=== markdownlint docs/ ==="
# @npx markdownlint 'docs/**/*.md'
	@npx --yes markdownlint-cli './**/*.md'
	@echo "✅ markdownlint passed"

lint: py-lint ts-lint rs-lint
	@echo ""
	@echo "✅ Linting complete!"

format: py-format ts-format rs-format
	@echo ""
	@echo "✅ Formatting complete!"

install: py-install ts-install
	@echo ""
	@echo "✅ All dependencies installed!"

check: test

validate-compare: py-build ts-build rs-build-debug
	@echo "=== Comparing validation errors across parsers ==="
	./scripts/compare_validate_errors.sh docs

# ============================================================================
# VERSION BUMPING (ALL)
# ============================================================================

bump: bump-patch

bump-major:
	@echo ""
	@echo "═══════════════════════════════════════════════════════════════"
	@echo "  Bumping MAJOR version for all parsers"
	@echo "═══════════════════════════════════════════════════════════════"
	@echo ""
	@$(MAKE) rs-bump-major
	@echo ""
	@$(MAKE) py-bump-major
	@echo ""
	@$(MAKE) ts-bump-major
	@echo ""
	@$(MAKE) ext-bump
	@echo ""
	@$(MAKE) mkdocs-bump-major
	@echo ""
	@echo "✅ All versions bumped (MAJOR)!"
	@echo ""

bump-minor:
	@echo ""
	@echo "═══════════════════════════════════════════════════════════════"
	@echo "  Bumping MINOR version for all parsers"
	@echo "═══════════════════════════════════════════════════════════════"
	@echo ""
	@$(MAKE) rs-bump-minor
	@echo ""
	@$(MAKE) py-bump-minor
	@echo ""
	@$(MAKE) ts-bump-minor
	@echo ""
	@$(MAKE) ext-bump
	@echo ""
	@$(MAKE) mkdocs-bump-minor
	@echo ""
	@echo "✅ All versions bumped (MINOR)!"
	@echo ""

bump-patch:
	@echo ""
	@echo "═══════════════════════════════════════════════════════════════"
	@echo "  Bumping PATCH version for all parsers"
	@echo "═══════════════════════════════════════════════════════════════"
	@echo ""
	@$(MAKE) rs-bump-patch
	@echo ""
	@$(MAKE) py-bump-patch
	@echo ""
	@$(MAKE) ts-bump-patch
	@echo ""
	@$(MAKE) ext-bump
	@echo ""
	@$(MAKE) mkdocs-bump-patch
	@echo ""
	@echo "✅ All versions bumped (PATCH)!"
	@echo ""

# ============================================================================
# BINARY CASCADE BUMP
# ----------------------------------------------------------------------------
# The native Rust binary is embedded in crates `qmdc`, PyPI `qmdc`, npm `qmdc`,
# and `qmdc-vscode`. When the binary changes, ALL FOUR must be rebuilt + bumped.
# `qmdc-semantic` / `qmdc-mkdocs` are NOT touched — they consume `qmdc` at
# runtime and pick up the new release on a fresh install.
# All four move at the same level so the bundled-binary version is unambiguous.
# ============================================================================

binary-bump: binary-bump-patch

binary-bump-major:
	@echo ""
	@echo "═══════════════════════════════════════════════════════════════"
	@echo "  Binary cascade: bumping MAJOR for crates+PyPI+npm qmdc + vscode"
	@echo "═══════════════════════════════════════════════════════════════"
	@echo ""
	@$(MAKE) rs-bump-major
	@echo ""
	@$(MAKE) py-bump-major
	@echo ""
	@$(MAKE) ts-bump-major
	@echo ""
	@$(MAKE) ext-bump
	@echo ""
	@echo "✅ Binary cascade bumped (MAJOR): rs, py, ts, vscode. semantic/mkdocs untouched."
	@echo ""

binary-bump-minor:
	@echo ""
	@echo "═══════════════════════════════════════════════════════════════"
	@echo "  Binary cascade: bumping MINOR for crates+PyPI+npm qmdc + vscode"
	@echo "═══════════════════════════════════════════════════════════════"
	@echo ""
	@$(MAKE) rs-bump-minor
	@echo ""
	@$(MAKE) py-bump-minor
	@echo ""
	@$(MAKE) ts-bump-minor
	@echo ""
	@$(MAKE) ext-bump
	@echo ""
	@echo "✅ Binary cascade bumped (MINOR): rs, py, ts, vscode. semantic/mkdocs untouched."
	@echo ""

binary-bump-patch:
	@echo ""
	@echo "═══════════════════════════════════════════════════════════════"
	@echo "  Binary cascade: bumping PATCH for crates+PyPI+npm qmdc + vscode"
	@echo "═══════════════════════════════════════════════════════════════"
	@echo ""
	@$(MAKE) rs-bump-patch
	@echo ""
	@$(MAKE) py-bump-patch
	@echo ""
	@$(MAKE) ts-bump-patch
	@echo ""
	@$(MAKE) ext-bump
	@echo ""
	@echo "✅ Binary cascade bumped (PATCH): rs, py, ts, vscode. semantic/mkdocs untouched."
	@echo ""

# ============================================================================
# BENCHMARK
# ============================================================================

BENCH_DIR := docs

speedtest: build
	@echo ""
	@echo "Warming up..."
	@./qmdc-rs/target/release/qmdc workspace parse $(BENCH_DIR) > /dev/null 2>&1
	@uv run --project qmdc-py python qmdc-py/qmdc.py workspace parse $(BENCH_DIR) > /dev/null 2>&1
	@node ./qmdc-ts/dist/qmdc.js workspace parse $(BENCH_DIR) > /dev/null 2>&1
	@echo ""
	@echo "═══════════════════════════════════════════════════════════════"
	@echo "  QMD-6 Workspace Parsing Benchmark"
	@echo "═══════════════════════════════════════════════════════════════"
	@echo ""
	@echo "RUST (release):"
	@RS_START=$$(python3 -c "import time; print(int(time.time()*1000))"); \
	./qmdc-rs/target/release/qmdc workspace parse $(BENCH_DIR) > /tmp/rs_out.json 2>/dev/null; \
	RS_END=$$(python3 -c "import time; print(int(time.time()*1000))"); \
	RS_OBJS=$$(grep -c '"__id"' /tmp/rs_out.json); \
	echo "  Objects: $$RS_OBJS"; \
	echo "  Time:    $$((RS_END - RS_START)) ms"
	@echo ""
	@echo "PYTHON:"
	@PY_START=$$(python3 -c "import time; print(int(time.time()*1000))"); \
	uv run --project qmdc-py python qmdc-py/qmdc.py workspace parse $(BENCH_DIR) > /tmp/py_out.json 2>/dev/null; \
	PY_END=$$(python3 -c "import time; print(int(time.time()*1000))"); \
	PY_OBJS=$$(grep -c '"__id"' /tmp/py_out.json); \
	echo "  Objects: $$PY_OBJS"; \
	echo "  Time:    $$((PY_END - PY_START)) ms"
	@echo ""
	@echo "TYPESCRIPT (node):"
	@TS_START=$$(python3 -c "import time; print(int(time.time()*1000))"); \
	node ./qmdc-ts/dist/qmdc.js workspace parse $(BENCH_DIR) > /tmp/ts_out.json 2>/dev/null; \
	TS_END=$$(python3 -c "import time; print(int(time.time()*1000))"); \
	TS_OBJS=$$(grep -c '"__id"' /tmp/ts_out.json); \
	echo "  Objects: $$TS_OBJS"; \
	echo "  Time:    $$((TS_END - TS_START)) ms"
	@echo ""
	@echo "═══════════════════════════════════════════════════════════════"

# ============================================================================
# PYTHON
# ============================================================================

py-build:
	@echo "=== Python: build ==="
	cd qmdc-py && uv sync >/dev/null 2>&1

py-test: reports-clean py-format py-lint py-build
	@echo "=== Python: test ==="
	@mkdir -p test-reports
	cd qmdc-py && uv run --extra dev pytest tests/ -v --junit-xml=../test-reports/py.xml

py-lint: py-format
	@echo "=== Python: lint ==="
	cd qmdc-py && uv run --extra dev ruff check qmdc/ tests/

py-format:
	@echo "=== Python: format ==="
	cd qmdc-py && uv run --extra dev ruff format qmdc/ tests/
	cd qmdc-py && uv run --extra dev ruff check --select I --fix qmdc/ tests/

py-install:
	@echo "=== Python: install ==="
	cd qmdc-py && uv sync --extra dev

py-bump: py-bump-patch

py-bump-major:
	@echo "=== Python: bump major version ==="
	cd qmdc-py && make bump-major

py-bump-minor:
	@echo "=== Python: bump minor version ==="
	cd qmdc-py && make bump-minor

py-bump-patch:
	@echo "=== Python: bump patch version ==="
	cd qmdc-py && make bump-patch

qmdc-py: py-build
	@cd qmdc-py && uv run python qmdc.py $(ARGS)

# ============================================================================
# TYPESCRIPT
# ============================================================================

ts-build:
	@echo "=== TypeScript: build ==="
	cd qmdc-ts && npm run build

ts-test: reports-clean ts-format ts-lint ts-build
	@echo "=== TypeScript: test ==="
	cd qmdc-ts && npm test

ts-lint: ts-format
	@echo "=== TypeScript: lint ==="
	cd qmdc-ts && npx tsc --noEmit
	cd qmdc-ts && npm run lint

ts-format:
	@echo "=== TypeScript: format ==="
	cd qmdc-ts && npm run format

ts-install:
	@echo "=== TypeScript: install ==="
	cd qmdc-ts && npm install

ts-bump: ts-bump-patch

ts-bump-major:
	@echo "=== TypeScript: bump major version ==="
	cd qmdc-ts && make bump-major

ts-bump-minor:
	@echo "=== TypeScript: bump minor version ==="
	cd qmdc-ts && make bump-minor

ts-bump-patch:
	@echo "=== TypeScript: bump patch version ==="
	cd qmdc-ts && make bump-patch

qmdc-ts: ts-build
	@cd qmdc-ts && ./qmdc $(ARGS)

# ============================================================================
# RUST
# ============================================================================

rs-build:
	@echo "=== Rust: build ==="
	cd qmdc-rs && cargo build --release

rs-build-debug:
	@echo "=== Rust: build (debug) ==="
	cd qmdc-rs && cargo build

rs-test: reports-clean rs-build-debug rs-format rs-lint
	@echo "=== Rust: test ==="
	@mkdir -p test-reports
	cd qmdc-rs && cargo nextest run
	cp qmdc-rs/target/nextest/default/junit.xml test-reports/rs.xml

rs-format:
	@echo "=== Rust: format ==="
	cd qmdc-rs && cargo fmt

rs-lint: rs-build-debug rs-format
	@echo "=== Rust: lint (clippy) ==="
	cd qmdc-rs && cargo clippy --all-targets -- -D warnings

rs-bump: rs-bump-patch

rs-bump-major:
	@echo "=== Rust: bump major version ==="
	cd qmdc-rs && make bump-major

rs-bump-minor:
	@echo "=== Rust: bump minor version ==="
	cd qmdc-rs && make bump-minor

rs-bump-patch:
	@echo "=== Rust: bump patch version ==="
	cd qmdc-rs && make bump-patch

qmdc-rs: rs-build
	@cd qmdc-rs && ./target/release/qmdc $(ARGS)

# ============================================================================
# PUBLISH — one idempotent path: build the matrix, then upload (skip-existing).
# Full command table in RELEASING.md.
# ============================================================================

# Build the FULL platform matrix (scripts/release-build.sh) and idempotently
# publish every artifact to ALL registries: PyPI, npm, crates.io, VS Code
# Marketplace, Open VSX. Re-runnable — anything already published is skipped.
# Tokens from env or .env.publish (UV_PUBLISH_TOKEN, NODE_AUTH_TOKEN,
# CARGO_REGISTRY_TOKEN, VSCE_PAT, OVSX_TOKEN). This is what the release CI runs.
publish: dist
	bash scripts/release-publish.sh --publish

# Same, DRY: build the matrix and show what WOULD publish (uploads nothing).
publish-check: dist
	bash scripts/release-publish.sh

semantic-bump: semantic-bump-patch

semantic-bump-major:
	@echo "=== qmdc-semantic: bump major version ==="
	cd qmdc-semantic && bash scripts/bump_version.sh major

semantic-bump-minor:
	@echo "=== qmdc-semantic: bump minor version ==="
	cd qmdc-semantic && bash scripts/bump_version.sh minor

semantic-bump-patch:
	@echo "=== qmdc-semantic: bump patch version ==="
	cd qmdc-semantic && bash scripts/bump_version.sh patch

# Release ONLY qmdc-semantic (pure-Python, PyPI) — build + idempotent upload.
# Bump first with `make semantic-bump`. (See also ext-release, mkdocs-release
# for the other leaf packages.)
semantic-release:
	@echo "=== qmdc-semantic: build + idempotent publish to PyPI ==="
	cd qmdc-semantic && rm -rf dist && uv build && uv run --with twine twine check dist/*
	@set -a; [ -f .env.publish ] && . ./.env.publish; set +a; \
	: "$${UV_PUBLISH_TOKEN:?Set UV_PUBLISH_TOKEN (env or .env.publish) to publish}"; \
	cd qmdc-semantic && TWINE_USERNAME=__token__ TWINE_PASSWORD="$$UV_PUBLISH_TOKEN" \
	  uv run --with twine twine upload --skip-existing dist/*
	@echo "✅ qmdc-semantic published (or already up — skipped)."

# Selective publish (advanced): after `make dist`, upload one registry, e.g.
#   bash scripts/release-publish.sh --publish pypi|npm|crate|vscode

# Cross-build the FULL platform matrix (7 targets) and assemble every artifact
# (binaries + npm per-platform packages + PyPI platform wheels + pure-Python dists)
# into ./dist-release/. Requires cargo-zigbuild + zig. Does NOT publish.
dist:
	@echo "=== Building full release matrix ==="
	bash scripts/release-build.sh

# ============================================================================
# VS CODE EXTENSION
# ============================================================================

ext-bump:
	@echo "=== VS Code Extension: bump version ==="
	cd qmdc-vscode && npm run bump-version

ext-build:
	@echo "=== VS Code Extension: build ==="
	cd qmdc-vscode && npm run build
	@echo ""
	@echo "✅ Extension built: $$(ls -t qmdc-vscode/*.vsix | head -1)"

ext-install: ext-build
	@echo "=== VS Code Extension: install ==="
	code --install-extension qmdc-vscode/*.vsix --force
	@echo ""
	@echo "✅ Extension installed! Restart VS Code."

# Build ONLY the VS Code extension — all 6 platform .vsix into dist-release/vscode.
# Reuses the cargo target cache (qmdc-rs/target): if the Rust source is unchanged,
# the per-target builds are incremental (seconds), NOT a 30-min cold matrix build.
# Does NOT touch py / ts / pypi-wheels / npm — unlike `make dist` / `make publish`.
ext-package:
	@echo "=== VS Code Extension: package all 6 platforms ==="
	cd qmdc-vscode && npm run package:all
	rm -rf dist-release/vscode && mkdir -p dist-release/vscode
	cp qmdc-vscode/*.vsix dist-release/vscode/
	@echo "✅ vsix → dist-release/vscode: $$(ls dist-release/vscode/*.vsix | wc -l | tr -d ' ')"

# Build the extension (ext-package) and idempotently publish ONLY it: Open VSX
# always, VS Code Marketplace if VSCE_PAT is set. Bump the version first
# (`make ext-bump`) — a registry won't accept a re-published same version.
ext-release: ext-package
	bash scripts/release-publish.sh --publish vscode

ext-test:
	@echo "=== VS Code Extension: Playwright tests ==="
	cd qmdc-vscode && npx playwright test

# Sync the shared Mermaid renderer core into the VS Code extension.
# Single source of truth: qmdc-mkdocs/.../templates/js/qmdc-mermaid-core.js.
# The VS Code preview ships its own committed copy (the .vsix has no access to
# the sibling qmdc-mkdocs package), kept identical by this target. A parity test
# (tests/test_mermaid.py::TestMermaidCoreParity) fails the build if they drift.
MERMAID_CORE_SRC := qmdc-mkdocs/qmdc_mkdocs/templates/js/qmdc-mermaid-core.js
MERMAID_CORE_DST := qmdc-vscode/templates/qmdc-mermaid-core.js

mermaid-sync:
	@echo "=== Syncing Mermaid core → VS Code extension ==="
	cp $(MERMAID_CORE_SRC) $(MERMAID_CORE_DST)
	@echo "✅ $(MERMAID_CORE_DST) is in sync"

# Vendored copy of the agent guide embedded into the qmdc crate (so it's
# self-contained / publishable). Canonical source is docs/guides/qmdc-guide.qmd.md.
GUIDE_SRC := docs/guides/qmdc-guide.qmd.md
GUIDE_DST := qmdc-rs/src/qmdc-guide.qmd.md

guide-sync:
	@echo "=== Syncing agent guide → qmdc crate ==="
	cp $(GUIDE_SRC) $(GUIDE_DST)
	@echo "✅ $(GUIDE_DST) is in sync"

# ============================================================================
# STATIC SITE
# ============================================================================

# Shared workspace/port settings for the MkDocs and semantic targets.
WS ?= ./docs
PORT ?= 8800

# MkDocs-based site (qmdc-mkdocs)
# Usage:
#   make site WS=./docs                → builds into ./docs/_site (= site-build)
#   make site-build WS=./docs          → builds into ./docs/_site
#   make site-serve WS=./docs          → build + live reload on port 8800
#   make site-serve WS=./docs PORT=3000
#   make site-regenerate WS=./docs     → regenerate ContentGenerator pages

# `make site` is a convenience alias for `make site-build`.
site: site-build

site-build:
	@echo "=== Building MkDocs site: $(WS) ==="
	cd qmdc-mkdocs && uv run qmdc-mkdocs -w ../$(WS) build

site-serve:
	@echo "=== Serving MkDocs site: $(WS) on port $(PORT) ==="
	cd qmdc-mkdocs && uv run qmdc-mkdocs -w ../$(WS) serve --port $(PORT)

site-regenerate:
	@echo "=== Regenerating content: $(WS) ==="
	cd qmdc-mkdocs && uv run qmdc-mkdocs -w ../$(WS) regenerate

# Strict build — warnings are FATAL. Fails on any mkdocs `WARNING` or qmdc
# `WARN:` (unresolved refs) so broken links never pass. Use this in CI/deploy.
site-build-strict:
	@echo "=== Building site (warnings fatal): $(WS) ==="
	@out="$$(cd qmdc-mkdocs && uv run qmdc-mkdocs -w ../$(WS) build 2>&1)"; rc=$$?; \
	printf '%s\n' "$$out"; \
	if [ $$rc -ne 0 ]; then echo "❌ site build failed"; exit $$rc; fi; \
	if printf '%s\n' "$$out" | grep -E 'WARNING|WARN:' >/dev/null; then \
	  echo ""; echo "❌ build emitted warnings — fix them before deploying"; exit 1; \
	fi; \
	echo "✅ clean build (no warnings)"

# Deploy the docs to Cloudflare. Refreshes the semantic index/hints first (so the
# deployed site ships fresh "similar objects" popovers + inferred edges), then a
# clean strict build, then deploy. `site-build` itself stays hermetic (CI builds
# from the committed hints.json, no embedding provider needed).
# Requires: an embedding provider for the refresh (local Ollama on :11434 with
# qwen3-embedding, or OPENROUTER_API_KEY), and wrangler auth (CLOUDFLARE_API_TOKEN
# + CLOUDFLARE_ACCOUNT_ID, or `wrangler login`).
site-deploy:
	@echo "=== Refreshing semantic index + hints: $(WS) ==="
	cd qmdc-semantic && uv run qmdc-semantic index ../$(WS) $(SEMANTIC_ARGS)
	cd qmdc-semantic && uv run python3 scripts/compute-hints.py ../$(WS)
	@$(MAKE) --no-print-directory site-build-strict WS=$(WS)
	@echo "=== Deploying docs to Cloudflare ==="
	npx wrangler deploy

mkdocs-test: reports-clean
	@echo "=== qmdc-mkdocs: test ==="
	@mkdir -p test-reports
	cd qmdc-mkdocs && uv run --extra dev pytest -q --junit-xml=../test-reports/mkdocs.xml

# Provider-free semantic unit tests (chunking/config/search/storage) + ruff.
# Excludes e2e/slow tests that need a real embedding provider + indexed DB.
semantic-test: reports-clean
	@echo "=== qmdc-semantic: format ==="
	cd qmdc-semantic && uv run --extra dev ruff format qmdc_semantic/ tests/
	cd qmdc-semantic && uv run --extra dev ruff check --select I --fix qmdc_semantic/ tests/
	@echo "=== qmdc-semantic: lint ==="
	cd qmdc-semantic && uv run --extra dev ruff check qmdc_semantic/ tests/
	@echo "=== qmdc-semantic: test (provider-free units) ==="
	@mkdir -p test-reports
	cd qmdc-semantic && uv run --extra dev pytest -m "not e2e and not slow" -q --junit-xml=../test-reports/semantic.xml

# VS Code extension preview/renderer tests (Playwright). Emits JUnit into the
# shared test-reports/ so the unified gate counts them as the `vscode` component.
# Needs the Chromium browser: `cd qmdc-vscode && npx playwright install chromium`.
vscode-test: reports-clean
	@echo "=== qmdc-vscode: preview tests (Playwright) ==="
	@mkdir -p test-reports
	cd qmdc-vscode && npx playwright test

mkdocs-bump: mkdocs-bump-patch

mkdocs-bump-major:
	@echo "=== qmdc-mkdocs: bump major version ==="
	cd qmdc-mkdocs && make bump-major

mkdocs-bump-minor:
	@echo "=== qmdc-mkdocs: bump minor version ==="
	cd qmdc-mkdocs && make bump-minor

mkdocs-bump-patch:
	@echo "=== qmdc-mkdocs: bump patch version ==="
	cd qmdc-mkdocs && make bump-patch

# Release ONLY qmdc-mkdocs (pure-Python, PyPI). Builds its sdist+wheel and
# idempotently uploads (twine --skip-existing) — re-runnable, fast, and does NOT
# touch the Rust binary, the other packages, or the platform matrix. Bump first
# (`make mkdocs-bump`). Token from the env or .env.publish (UV_PUBLISH_TOKEN).
mkdocs-release:
	@echo "=== qmdc-mkdocs: build + idempotent publish to PyPI ==="
	cd qmdc-mkdocs && rm -rf dist && uv build && uv run --with twine twine check dist/*
	@set -a; [ -f .env.publish ] && . ./.env.publish; set +a; \
	: "$${UV_PUBLISH_TOKEN:?Set UV_PUBLISH_TOKEN (env or .env.publish) to publish}"; \
	cd qmdc-mkdocs && TWINE_USERNAME=__token__ TWINE_PASSWORD="$$UV_PUBLISH_TOKEN" \
	  uv run --with twine twine upload --skip-existing dist/*
	@echo "✅ qmdc-mkdocs published (or already up — skipped)."

# ============================================================================
# SEMANTIC INDEX
# ============================================================================

# Update semantic embeddings for a workspace.
# Usage:
#   make semantic-index WS=./docs
#   make semantic-index WS=./docs SEMANTIC_ARGS="-v --force"
SEMANTIC_ARGS ?= -v

semantic-index:
	@echo "=== Updating semantic index: $(WS) ==="
	cd qmdc-semantic && uv run qmdc-semantic index ../$(WS) $(SEMANTIC_ARGS)

semantic-audit:
	@echo "=== Auditing edges: $(WS) ==="
	cd qmdc-semantic && uv run python3 scripts/audit-edges.py ../$(WS) $(SEMANTIC_ARGS)

semantic-hints:
	@echo "=== Computing semantic hints: $(WS) ==="
	cd qmdc-semantic && uv run python3 scripts/compute-hints.py ../$(WS)

# Regenerate the committed semantic artifacts (index + hints) before a release,
# then commit them (embeddings.db via LFS, hints.json plain). See QMD-61 / RELEASING.md.
semantic-refresh:
	@echo "=== Refreshing semantic artifacts (index + hints): $(WS) ==="
	@$(MAKE) semantic-index
	@$(MAKE) semantic-hints
	@echo "✅ Semantic artifacts refreshed. Commit docs/.qmdc-semantic/{embeddings.db,hints.json}."

# ============================================================================
# CLEANUP
# ============================================================================

clean:
	@echo "=== Cleaning build artifacts ==="
	rm -rf qmdc-py/build/
	rm -rf qmdc-py/dist/
	rm -rf qmdc-py/*.egg-info/
	rm -rf qmdc-py/.pytest_cache/
	rm -rf qmdc-py/.ruff_cache/
	rm -rf qmdc-ts/dist/
	rm -rf qmdc-ts/.tsbuildinfo
	cd qmdc-rs && cargo clean 2>/dev/null || true
	find . -type d -name "__pycache__" -exec rm -rf {} + 2>/dev/null || true
	find . -type f -name "*.pyc" -delete
	@echo "✅ Cleanup complete!"
