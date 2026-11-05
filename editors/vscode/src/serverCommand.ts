// Which `ths` runs the language server: the one the setting names, else the
// one bundled into a platform-specific package, else `ths` on `PATH`.

import { constants } from "node:fs";
import { access, chmod } from "node:fs/promises";
import * as path from "node:path";

import { fileExists } from "./fileExists";

/** Where packaging puts the binary, relative to the extension's root. */
export const BUNDLED_SERVER_DIRECTORY = "server";
const BINARY_NAME = "ths";
const WINDOWS_PLATFORM = "win32";
const WINDOWS_EXECUTABLE_EXTENSION = ".exe";
const EXECUTABLE_MODE = 0o755;

export function bundledBinaryName(platform: NodeJS.Platform): string {
  return platform === WINDOWS_PLATFORM ? BINARY_NAME + WINDOWS_EXECUTABLE_EXTENSION : BINARY_NAME;
}

/** `configured` is the setting's value; empty means not set. */
export async function serverCommand(configured: string, extensionRoot: string): Promise<string> {
  if (configured.trim() !== "") {
    return configured;
  }
  const bundled = path.join(extensionRoot, BUNDLED_SERVER_DIRECTORY, bundledBinaryName(process.platform));
  if (!(await fileExists(bundled))) {
    return BINARY_NAME;
  }
  await ensureExecutable(bundled);
  return bundled;
}

/** An install that unpacked the package without file modes leaves the
 * binary unrunnable on macOS and Linux. */
async function ensureExecutable(file: string): Promise<void> {
  if (process.platform === WINDOWS_PLATFORM) {
    return;
  }
  try {
    await access(file, constants.X_OK);
  } catch {
    await chmod(file, EXECUTABLE_MODE);
  }
}
