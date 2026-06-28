.PHONY: lint test check format build bump bump-major bump-minor bump-patch

format:
	npm run format

lint:
	npm run lint

build:
	npm run build

test: build
	npm test

all: test

# Version bumping
bump: bump-patch

bump-major:
	bash scripts/bump_version.sh major

bump-minor:
	bash scripts/bump_version.sh minor

bump-patch:
	bash scripts/bump_version.sh patch

