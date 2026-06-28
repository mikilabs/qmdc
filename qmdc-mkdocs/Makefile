.PHONY: lint test check format format-check bump bump-major bump-minor bump-patch

format:
	uv run --extra dev ruff format qmdc_mkdocs/ tests/
	uv run --extra dev ruff check --select I --fix qmdc_mkdocs/ tests/

lint:
	uv run --extra dev ruff check qmdc_mkdocs/ tests/

format-check: format lint

test: format-check
	uv run --extra dev pytest -q

all: test

# Version bumping
bump: bump-patch

bump-major:
	bash scripts/bump_version.sh major

bump-minor:
	bash scripts/bump_version.sh minor

bump-patch:
	bash scripts/bump_version.sh patch
