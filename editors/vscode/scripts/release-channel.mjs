// Whether this version is a preview, from `preview` in package.json: the
// one switch for an alpha or beta. A preview is packaged and published as a
// pre-release, so the marketplaces show it as one and only users who opt into
// pre-releases are updated to it, and its GitHub release is a pre-release.
//   node scripts/release-channel.mjs    prints true or false

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

export function isPreview() {
  const manifest = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
  return manifest.preview === true;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  console.log(String(isPreview()));
}
