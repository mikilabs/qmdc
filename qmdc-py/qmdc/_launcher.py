"""Console-script entry point for the `qmdc` command.

Two-channel model:
- `import qmdc`            -> the pure-Python parser (this package's modules)
- `qmdc` on the CLI        -> the fast native Rust binary, when bundled

At publish time the per-platform Rust binary is copied into ``qmdc/bin/<platform>/qmdc``
(see ``scripts/publish.sh``). When that binary is present we exec it; otherwise we fall
back to the pure-Python CLI so the command always works from a source checkout.
"""

import os
import platform
import sys
from pathlib import Path


def _bundled_binary() -> Path | None:
    """Return the bundled native binary for this platform, or None."""
    system = platform.system().lower()
    machine = platform.machine().lower()

    if system == "darwin":
        plat = "macos-arm64" if ("arm" in machine or "aarch64" in machine) else "macos-x64"
    elif system == "linux":
        plat = "linux-arm64" if ("arm" in machine or "aarch64" in machine) else "linux-x64"
    elif system == "windows":
        plat = "windows-arm64" if ("arm" in machine or "aarch64" in machine) else "windows-x64"
    else:
        return None

    binary_name = "qmdc.exe" if system == "windows" else "qmdc"
    candidate = Path(__file__).parent / "bin" / plat / binary_name
    return candidate if candidate.exists() else None


def main() -> None:
    """Exec the bundled native binary if available, else the pure-Python CLI."""
    binary = _bundled_binary()
    if binary is not None:
        if os.name == "nt":
            import subprocess

            sys.exit(subprocess.run([str(binary), *sys.argv[1:]], check=False).returncode)
        os.execv(str(binary), [str(binary), *sys.argv[1:]])

    # Fallback: pure-Python CLI (used from a source checkout / when no binary bundled).
    from qmdc.cli import cli

    cli()


if __name__ == "__main__":
    main()
