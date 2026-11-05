// Fails unless the release tag given is `v` followed by the version in
// package.json, so a release never publishes packages of another version.
//   node scripts/check-release-tag.mjs v0.1.0

import { readFileSync } from "node:fs";

const TAG_PREFIX = "v";

const tag = process.argv[2] ?? "";
const manifest = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
const expected = TAG_PREFIX + manifest.version;
if (tag !== expected) {
  console.error(`The tag '${tag}' does not match the extension's version: expected '${expected}'.`);
  process.exit(1);
}
