.PHONY: lint test check format-check bump bump-major bump-minor bump-patch

format:
	cd .. && uv run ruff format qmdc-py/
	cd .. && uv run ruff check --select I --fix qmdc-py/

lint:
	cd .. && uv run ruff check qmdc-py/

format-check: format lint

test: format-check
	cd .. && uv run pytest qmdc-py/tests/ -v

all: test

# Version bumping
bump: bump-patch

bump-major:
	bash scripts/bump_version.sh major

bump-minor:
	bash scripts/bump_version.sh minor

bump-patch:
	bash scripts/bump_version.sh patch
