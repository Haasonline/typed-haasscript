// Packages the extension for one platform with its own `ths` inside:
//   pnpm run package:platform <platform>
// where <platform> is a `vsce package --target` name, such as linux-x64.
// Builds `ths` for the matching Rust target, so that target must be
// installed (`rustup target add …`) and able to link on this machine.

import { spawnSync } from "node:child_process";
import { chmodSync, copyFileSync, mkdirSync, rmSync } from "node:fs";
import { fileURLToPath } from "node:url";

/** The Rust target each platform's `ths` is built for. Linux is built
 * against musl, statically linked, so one binary runs on every
 * distribution, Alpine included. */
const RUST_TARGETS = {
  "win32-x64": "x86_64-pc-windows-msvc",
  "win32-arm64": "aarch64-pc-windows-msvc",
  "darwin-x64": "x86_64-apple-darwin",
  "darwin-arm64": "aarch64-apple-darwin",
  "linux-x64": "x86_64-unknown-linux-musl",
  "linux-arm64": "aarch64-unknown-linux-musl",
  "alpine-x64": "x86_64-unknown-linux-musl",
  "alpine-arm64": "aarch64-unknown-linux-musl",
};
const WINDOWS_PLATFORM_PREFIX = "win32-";
const BINARY_NAME = "ths";
const WINDOWS_EXECUTABLE_EXTENSION = ".exe";
// Must match BUNDLED_SERVER_DIRECTORY in src/serverCommand.ts.
const SERVER_DIRECTORY = "server";
const EXECUTABLE_MODE = 0o755;

const platform = process.argv[2];
const rustTarget = RUST_TARGETS[platform];
if (rustTarget === undefined) {
  console.error(`Usage: pnpm run package:platform <platform>, one of: ${Object.keys(RUST_TARGETS).join(", ")}`);
  process.exit(2);
}

const extensionRoot = fileURLToPath(new URL("..", import.meta.url));
const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url));
const binaryName = platform.startsWith(WINDOWS_PLATFORM_PREFIX) ? BINARY_NAME + WINDOWS_EXECUTABLE_EXTENSION : BINARY_NAME;

run("cargo", ["build", "--release", "--locked", "--package", BINARY_NAME, "--target", rustTarget], repositoryRoot, {
  // Symbols only make the download bigger.
  CARGO_PROFILE_RELEASE_STRIP: "symbols",
});

const serverDirectory = `${extensionRoot}${SERVER_DIRECTORY}`;
rmSync(serverDirectory, { recursive: true, force: true });
mkdirSync(serverDirectory);
const bundled = `${serverDirectory}/${binaryName}`;
copyFileSync(`${repositoryRoot}target/${rustTarget}/release/${binaryName}`, bundled);
chmodSync(bundled, EXECUTABLE_MODE);

run("node", ["scripts/copy-licenses.mjs"], extensionRoot);
run("node", ["scripts/vsce-package.mjs", "--target", platform], extensionRoot);

function run(command, commandArguments, directory, environment = {}) {
  const result = spawnSync(command, commandArguments, {
    cwd: directory,
    stdio: "inherit",
    env: { ...process.env, ...environment },
    shell: process.platform === "win32",
  });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}
