#!/usr/bin/env node
/**
 * Script to bump the project version in package.json.
 * Usage: npm run bump-version [major|minor|patch]
 *        or: npx tsx scripts/bump_version.ts [major|minor|patch]
 */

import * as fs from "fs";
import * as path from "path";

interface PackageJson {
  version: string;
  [key: string]: any;
}

function getCurrentVersion(packageJsonPath: string): string {
  const content = fs.readFileSync(packageJsonPath, "utf-8");
  const packageJson: PackageJson = JSON.parse(content);

  if (!packageJson.version) {
    throw new Error("Could not find version in package.json");
  }

  return packageJson.version;
}

function bumpVersion(
  version: string,
  part: "major" | "minor" | "patch"
): string {
  const parts = version.split(".").map(Number);

  if (parts.length !== 3 || parts.some(isNaN)) {
    throw new Error(`Invalid version format: ${version}`);
  }

  let [major, minor, patch] = parts;

  switch (part) {
    case "major":
      major += 1;
      minor = 0;
      patch = 0;
      break;
    case "minor":
      minor += 1;
      patch = 0;
      break;
    case "patch":
      patch += 1;
      break;
    default:
      throw new Error(`Invalid part: ${part}`);
  }

  return `${major}.${minor}.${patch}`;
}

function updatePackageJson(
  packageJsonPath: string,
  oldVersion: string,
  newVersion: string
): void {
  const content = fs.readFileSync(packageJsonPath, "utf-8");
  const packageJson: PackageJson = JSON.parse(content);

  if (packageJson.version !== oldVersion) {
    console.warn(
      `Warning: Version ${oldVersion} not found in package.json (found ${packageJson.version})`
    );
  }

  packageJson.version = newVersion;

  // Write back with proper formatting (2 spaces indentation)
  const newContent = JSON.stringify(packageJson, null, 2) + "\n";
  fs.writeFileSync(packageJsonPath, newContent, "utf-8");

  console.log(`Updated ${packageJsonPath} from ${oldVersion} to ${newVersion}`);
}

function main(): void {
  const args = process.argv.slice(2);

  if (args.length > 1) {
    console.error("Usage: npm run bump-version [major|minor|patch]");
    process.exit(1);
  }

  const part = (args[0] || "patch") as "major" | "minor" | "patch";

  if (!["major", "minor", "patch"].includes(part)) {
    console.error(
      `Invalid argument: ${part}. Must be one of: major, minor, patch`
    );
    process.exit(1);
  }

  const rootDir = path.resolve(__dirname, "..");
  const packageJsonPath = path.join(rootDir, "package.json");

  if (!fs.existsSync(packageJsonPath)) {
    console.error(`Error: package.json not found at ${packageJsonPath}`);
    process.exit(1);
  }

  try {
    const currentVersion = getCurrentVersion(packageJsonPath);
    const newVersion = bumpVersion(currentVersion, part);

    console.log(`Bumping version: ${currentVersion} -> ${newVersion}`);

    updatePackageJson(packageJsonPath, currentVersion, newVersion);

    console.log("Done!");
  } catch (error) {
    console.error(
      `Error: ${error instanceof Error ? error.message : String(error)}`
    );
    process.exit(1);
  }
}

main();

