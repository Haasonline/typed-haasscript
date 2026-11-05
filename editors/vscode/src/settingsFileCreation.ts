// Creates a ths.toml by running `ths init`, which owns what the file holds.

import { execFile } from "node:child_process";

const INIT_SUBCOMMAND = "init";
/** The error code of a command that cannot be found. */
const COMMAND_NOT_FOUND = "ENOENT";

export type CreationResult = { created: true } | { created: false; reason: string };

export function createSettingsFile(thsCommand: string, directory: string): Promise<CreationResult> {
  return new Promise((resolve) => {
    execFile(thsCommand, [INIT_SUBCOMMAND, directory], (error, _output, errorOutput) => {
      if (error === null) {
        resolve({ created: true });
      } else if (error.code === COMMAND_NOT_FOUND) {
        resolve({ created: false, reason: `the '${thsCommand}' command was not found` });
      } else {
        resolve({ created: false, reason: errorOutput.trim() || error.message });
      }
    });
  });
}
