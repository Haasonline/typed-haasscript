// Runs `vsce package`, as a pre-release when this version is a preview.
// Any arguments, such as `--target linux-x64`, are passed on to vsce.
//   node scripts/vsce-package.mjs [vsce arguments…]

import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import { isPreview } from "./release-channel.mjs";

const PRE_RELEASE_FLAG = "--pre-release";

const extensionRoot = fileURLToPath(new URL("..", import.meta.url));
const vsceArguments = ["exec", "vsce", "package", "--no-dependencies", ...(isPreview() ? [PRE_RELEASE_FLAG] : [])];
const result = spawnSync("pnpm", [...vsceArguments, ...process.argv.slice(2)], {
  cwd: extensionRoot,
  stdio: "inherit",
  shell: process.platform === "win32",
});
process.exit(result.status ?? 1);
