// Finding a script's ths.toml and deciding whether to offer to create one.
// No editor API here, so the tests run it directly.

import * as path from "node:path";

/** The project settings file; the server checks only files below one. */
export const SETTINGS_FILE = "ths.toml";

/** The `ths.toml` in `fileDirectory` or the nearest directory above it,
 * as the server finds it. */
export async function findSettingsFile(
  fileDirectory: string,
  exists: (file: string) => Promise<boolean>,
): Promise<string | undefined> {
  let directory = fileDirectory;
  for (;;) {
    const candidate = path.join(directory, SETTINGS_FILE);
    if (await exists(candidate)) {
      return candidate;
    }
    const parent = path.dirname(directory);
    if (parent === directory) {
      return undefined;
    }
    directory = parent;
  }
}

export interface OfferCircumstances {
  hasSettingsFile: boolean;
  /** A lone file outside every workspace folder has no obvious place for one. */
  inWorkspaceFolder: boolean;
  /** "Don't ask again" was chosen for the folder. */
  declined: boolean;
  /** Already offered for the folder this session. */
  alreadyOffered: boolean;
}

export function shouldOfferSettingsFile(circumstances: OfferCircumstances): boolean {
  return (
    !circumstances.hasSettingsFile &&
    circumstances.inWorkspaceFolder &&
    !circumstances.declined &&
    !circumstances.alreadyOffered
  );
}
