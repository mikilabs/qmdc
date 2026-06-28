#!/usr/bin/env node
// Launcher for the `qmdc` CLI: resolves the per-platform optionalDependency
// package (installed by npm via os/cpu/libc filtering) and execs the native
// Rust binary it contains. See the @qmdc/cli-<platform> packages.
"use strict";

const { spawnSync } = require("node:child_process");
const path = require("node:path");

function isMusl() {
  // glibc systems report a glibc version; musl (Alpine) does not.
  try {
    const report = process.report?.getReport();
    const header = report && report.header;
    if (header && header.glibcVersionRuntime) return false;
    return true;
  } catch {
    return false;
  }
}

function platformPackage() {
  const { platform, arch } = process;
  if (platform === "linux") {
    const libc = isMusl() ? "musl" : "gnu";
    // We currently ship musl only for x64; arm64 falls back to gnu.
    if (arch === "x64" && libc === "musl") return "@qmdc/cli-linux-x64-musl";
    if (arch === "x64") return "@qmdc/cli-linux-x64";
    if (arch === "arm64") return "@qmdc/cli-linux-arm64";
  } else if (platform === "darwin") {
    if (arch === "arm64") return "@qmdc/cli-darwin-arm64";
    if (arch === "x64") return "@qmdc/cli-darwin-x64";
  } else if (platform === "win32") {
    if (arch === "x64") return "@qmdc/cli-win32-x64";
    if (arch === "arm64") return "@qmdc/cli-win32-arm64";
  }
  return null;
}

function resolveBinary() {
  const pkg = platformPackage();
  if (!pkg) {
    throw new Error(`qmdc: unsupported platform ${process.platform}/${process.arch}. ` +
      `Install from source with: cargo install qmdc`);
  }
  const binName = process.platform === "win32" ? "qmdc.exe" : "qmdc";
  try {
    const pkgJson = require.resolve(`${pkg}/package.json`);
    return path.join(path.dirname(pkgJson), binName);
  } catch {
    throw new Error(
      `qmdc: the platform package "${pkg}" is not installed.\n` +
      `If you use a lockfile, ensure npm >= 11.3 / Node >= 24, or reinstall without --no-optional.\n` +
      `Fallbacks: "npx --package=@qmdc/cli-... qmdc" or "cargo install qmdc".`
    );
  }
}

const result = spawnSync(resolveBinary(), process.argv.slice(2), { stdio: "inherit" });
process.exit(result.status === null ? 1 : result.status);
