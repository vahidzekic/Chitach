#!/usr/bin/env node
// Sets the Chitach version everywhere it is declared:
//   Cargo.toml ([workspace.package] version), src-tauri/tauri.conf.json,
//   ui/package.json and ui/package-lock.json.
//
// Usage: node scripts/set-version.mjs 1.2.3      (a leading "v" is stripped)
//        node scripts/set-version.mjs refs/tags/v1.2.3
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const raw = process.argv[2] ?? "";
const version = raw.replace(/^refs\/tags\//, "").replace(/^v/, "");

if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$/.test(version)) {
  console.error(`set-version: "${raw}" is not a semantic version (expected X.Y.Z or vX.Y.Z)`);
  process.exit(1);
}

// Cargo.toml: only the version inside [workspace.package].
const cargoPath = join(root, "Cargo.toml");
const cargo = readFileSync(cargoPath, "utf8");
const section = /(\[workspace\.package\][^\[]*?\nversion\s*=\s*")[^"]*(")/;
if (!section.test(cargo)) {
  console.error("set-version: no version in [workspace.package] of Cargo.toml");
  process.exit(1);
}
writeFileSync(cargoPath, cargo.replace(section, `$1${version}$2`));

const editJson = (rel, fn) => {
  const p = join(root, rel);
  if (!existsSync(p)) return;
  const json = JSON.parse(readFileSync(p, "utf8"));
  fn(json);
  writeFileSync(p, JSON.stringify(json, null, 2) + "\n");
  console.log(`set-version: ${rel} -> ${version}`);
};

editJson("src-tauri/tauri.conf.json", (j) => (j.version = version));
editJson("ui/package.json", (j) => (j.version = version));
editJson("ui/package-lock.json", (j) => {
  j.version = version;
  if (j.packages && j.packages[""]) j.packages[""].version = version;
});
console.log(`set-version: Cargo.toml -> ${version}`);
