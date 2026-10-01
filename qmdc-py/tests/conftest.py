"""Which `qmdc` the harnesses test.

The harnesses import only the package root (`from qmdc import ...`), so a name
missing from the public surface fails here, not only after publishing.

QMDC_TEST_TARGET=source (default): whatever `qmdc` the venv resolves, which is
  the editable in-tree install that `make py-test` syncs.
QMDC_TEST_TARGET=installed: the built wheel installed into a clean venv (see
  scripts/package-e2e.sh). Python silently prefers a `qmdc/` directory that is
  earlier on sys.path, so a stray in-tree path would turn the whole run green on
  the sources. Installed mode therefore proves the module came from
  QMDC_TEST_INSTALL_DIR before any test runs, and aborts otherwise.
"""

import os
from pathlib import Path

import pytest


def pytest_configure(config):
    target = os.environ.get("QMDC_TEST_TARGET", "source")
    if target == "source":
        return
    if target != "installed":
        raise pytest.UsageError(f"QMDC_TEST_TARGET must be 'source' or 'installed', got '{target}'")
    install_dir = os.environ.get("QMDC_TEST_INSTALL_DIR")
    if not install_dir:
        raise pytest.UsageError("QMDC_TEST_TARGET=installed needs QMDC_TEST_INSTALL_DIR")

    import qmdc

    loaded = Path(qmdc.__file__).resolve()
    expected = Path(install_dir).resolve()
    if expected not in loaded.parents or "site-packages" not in loaded.parts:
        raise pytest.UsageError(f"qmdc loaded from {loaded}, not the install under {expected}")
    print(f"[qmdc under test] installed package: {loaded}")
