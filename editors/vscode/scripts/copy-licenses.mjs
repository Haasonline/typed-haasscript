// Copies the repository's license files beside package.json, where
// `vsce package` looks for them. The copies are not committed.

import { copyFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const LICENSE_FILES = ["LICENSE-MIT", "LICENSE-APACHE"];

for (const name of LICENSE_FILES) {
  copyFileSync(
    fileURLToPath(new URL(`../../../${name}`, import.meta.url)),
    fileURLToPath(new URL(`../${name}`, import.meta.url)),
  );
}
