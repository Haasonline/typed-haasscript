// Starts `ths server` and hands it the Typed HaasScript and HaasScript
// documents. All checking happens in the server; this only connects it.
// The server asks for the file events it needs, so no watcher is set up here.

import * as vscode from "vscode";
import {
  LanguageClient,
  type LanguageClientOptions,
  type ServerOptions,
} from "vscode-languageclient/node";

import { Language, Setting, CommandId, SETTINGS_SECTION } from "./names";
import { serverCommand } from "./serverCommand";
import { SettingsFilePrompt } from "./settingsFilePrompt";

const CLIENT_ID = "typedHaasScript";
const CLIENT_NAME = "Typed HaasScript";
// The default of the server path setting in package.json: not set.
const SERVER_PATH_NOT_SET = "";
const SERVER_ARGUMENTS = ["server"];

let client: LanguageClient | undefined;
let extensionRoot = "";

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  extensionRoot = context.extensionPath;
  const settingsFilePrompt = new SettingsFilePrompt(context.workspaceState, resolvedServerCommand);
  context.subscriptions.push(
    vscode.commands.registerCommand(CommandId.RestartServer, restart),
    vscode.commands.registerCommand(CommandId.CreateSettingsFile, () => settingsFilePrompt.createOnRequest()),
    vscode.workspace.onDidOpenTextDocument((document) => void settingsFilePrompt.documentOpened(document)),
    vscode.workspace.onDidChangeConfiguration((change) => {
      if (change.affectsConfiguration(`${SETTINGS_SECTION}.${Setting.ServerPath}`)) {
        void restart();
      }
    }),
  );
  for (const document of vscode.workspace.textDocuments) {
    void settingsFilePrompt.documentOpened(document);
  }
  await start();
}

export async function deactivate(): Promise<void> {
  await stop();
}

async function start(): Promise<void> {
  const command = await resolvedServerCommand();
  const server: ServerOptions = { command, args: SERVER_ARGUMENTS };
  const options: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: Language.Typed },
      { scheme: "file", language: Language.Plain },
    ],
  };
  client = new LanguageClient(CLIENT_ID, CLIENT_NAME, server, options);
  try {
    await client.start();
  } catch (error) {
    client = undefined;
    const reason = error instanceof Error ? error.message : String(error);
    void vscode.window.showErrorMessage(
      `Typed HaasScript: cannot start '${command} server' (${reason}). ` +
        `Install ths, or set ${SETTINGS_SECTION}.${Setting.ServerPath} to where it is.`,
    );
  }
}

async function resolvedServerCommand(): Promise<string> {
  const configured = vscode.workspace
    .getConfiguration(SETTINGS_SECTION)
    .get<string>(Setting.ServerPath, SERVER_PATH_NOT_SET);
  return serverCommand(configured, extensionRoot);
}

async function stop(): Promise<void> {
  const running = client;
  client = undefined;
  if (running?.isRunning()) {
    await running.stop();
  }
}

async function restart(): Promise<void> {
  await stop();
  await start();
}
