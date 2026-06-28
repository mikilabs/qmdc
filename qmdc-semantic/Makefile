.PHONY: lint test check format format-check all

format:
	cd .. && uv run ruff format qmdc-semantic/
	cd .. && uv run ruff check --select I --fix qmdc-semantic/

lint:
	cd .. && uv run ruff check qmdc-semantic/

format-check: format lint

test: format-check
	cd .. && uv run pytest qmdc-semantic/tests/ -v

all: test
