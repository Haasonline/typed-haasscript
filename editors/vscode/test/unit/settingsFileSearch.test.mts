import assert from "node:assert/strict";
import * as path from "node:path";
import { test } from "node:test";

import { type OfferCircumstances, SETTINGS_FILE, findSettingsFile, shouldOfferSettingsFile } from "../../src/settingsFileSearch.ts";

const PROJECT = path.join(path.sep, "work", "bots");
const SCRIPT_DIRECTORY = path.join(PROJECT, "trend", "signals");

function existing(...files: string[]): (file: string) => Promise<boolean> {
  return async (file) => files.includes(file);
}

test("a settings file in an ancestor directory is found", async () => {
  const settingsFile = path.join(PROJECT, SETTINGS_FILE);
  assert.equal(await findSettingsFile(SCRIPT_DIRECTORY, existing(settingsFile)), settingsFile);
});

test("the nearest settings file wins", async () => {
  const outer = path.join(PROJECT, SETTINGS_FILE);
  const inner = path.join(SCRIPT_DIRECTORY, SETTINGS_FILE);
  assert.equal(await findSettingsFile(SCRIPT_DIRECTORY, existing(outer, inner)), inner);
});

test("the search stops at the filesystem root", async () => {
  assert.equal(await findSettingsFile(SCRIPT_DIRECTORY, existing()), undefined);
});

const OFFERED: OfferCircumstances = {
  hasSettingsFile: false,
  inWorkspaceFolder: true,
  declined: false,
  alreadyOffered: false,
};

test("a script in a folder without a settings file is offered one", () => {
  assert.equal(shouldOfferSettingsFile(OFFERED), true);
});

test("nothing is offered where a settings file exists", () => {
  assert.equal(shouldOfferSettingsFile({ ...OFFERED, hasSettingsFile: true }), false);
});

test("nothing is offered for a file outside every workspace folder", () => {
  assert.equal(shouldOfferSettingsFile({ ...OFFERED, inWorkspaceFolder: false }), false);
});

test("nothing is offered after don't ask again", () => {
  assert.equal(shouldOfferSettingsFile({ ...OFFERED, declined: true }), false);
});

test("a folder is offered once a session", () => {
  assert.equal(shouldOfferSettingsFile({ ...OFFERED, alreadyOffered: true }), false);
});
