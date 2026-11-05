// Names `package.json` declares; each must match its entry there.

/** The language identifiers of `contributes.languages`. */
export enum Language {
  Typed = "typed-haasscript",
  Plain = "haasscript",
}

/** The section of `contributes.configuration`. */
export const SETTINGS_SECTION = "typedHaasScript";

/** The settings under `SETTINGS_SECTION`. */
export enum Setting {
  ServerPath = "server.path",
}

/** The identifiers of `contributes.commands`. */
export enum CommandId {
  RestartServer = "typedHaasScript.restartServer",
  CreateSettingsFile = "typedHaasScript.createSettingsFile",
}
