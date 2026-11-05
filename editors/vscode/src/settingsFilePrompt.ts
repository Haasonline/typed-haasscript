// Offers to create a ths.toml when a script opens outside any project, and
// the command that creates one on request.

import * as path from "node:path";
import * as vscode from "vscode";

import { fileExists } from "./fileExists";
import { Language } from "./names";
import { createSettingsFile } from "./settingsFileCreation";
import { SETTINGS_FILE, findSettingsFile, shouldOfferSettingsFile } from "./settingsFileSearch";

const FILE_SCHEME = "file";
/** Workspace state: the URIs of the folders "Don't ask again" was chosen for. */
const DECLINED_FOLDERS_KEY = "typedHaasScript.settingsFile.declinedFolders";

const CREATE_CHOICE = "Create ths.toml";
const DECLINE_CHOICE = "Don't Ask Again";
const OPEN_CHOICE = "Open ths.toml";

export class SettingsFilePrompt {
  /** Folders offered to this session, by URI. */
  private readonly offered = new Set<string>();

  constructor(
    private readonly workspaceState: vscode.Memento,
    private readonly thsCommand: () => Promise<string>,
  ) {}

  async documentOpened(document: vscode.TextDocument): Promise<void> {
    if (!isScript(document)) {
      return;
    }
    const folder = vscode.workspace.getWorkspaceFolder(document.uri);
    const folderKey = folder?.uri.toString() ?? "";
    const settingsFile = await findSettingsFile(path.dirname(document.uri.fsPath), fileExists);
    const offer = shouldOfferSettingsFile({
      hasSettingsFile: settingsFile !== undefined,
      inWorkspaceFolder: folder !== undefined,
      declined: this.declinedFolders().includes(folderKey),
      alreadyOffered: this.offered.has(folderKey),
    });
    if (!offer || folder === undefined) {
      return;
    }
    this.offered.add(folderKey);
    const choice = await vscode.window.showInformationMessage(
      `Typed HaasScript checks scripts in folders with a ${SETTINGS_FILE}. Create one in '${folder.name}'?`,
      CREATE_CHOICE,
      DECLINE_CHOICE,
    );
    if (choice === CREATE_CHOICE) {
      await this.create(folder);
    } else if (choice === DECLINE_CHOICE) {
      await this.workspaceState.update(DECLINED_FOLDERS_KEY, [...this.declinedFolders(), folderKey]);
    }
  }

  /** The command: creates one in the folder of the active editor, or a chosen folder. */
  async createOnRequest(): Promise<void> {
    const folder = await chosenFolder();
    if (folder === undefined) {
      return;
    }
    if (await fileExists(path.join(folder.uri.fsPath, SETTINGS_FILE))) {
      void vscode.window.showInformationMessage(`'${folder.name}' already has a ${SETTINGS_FILE}.`);
      return;
    }
    await this.create(folder);
  }

  private async create(folder: vscode.WorkspaceFolder): Promise<void> {
    const result = await createSettingsFile(await this.thsCommand(), folder.uri.fsPath);
    if (!result.created) {
      void vscode.window.showErrorMessage(`Typed HaasScript: cannot create ${SETTINGS_FILE}: ${result.reason}`);
      return;
    }
    // The server watches for it and starts checking on its own.
    const choice = await vscode.window.showInformationMessage(
      `Created ${SETTINGS_FILE} in '${folder.name}'. Its scripts are checked now.`,
      OPEN_CHOICE,
    );
    if (choice === OPEN_CHOICE) {
      await vscode.window.showTextDocument(vscode.Uri.joinPath(folder.uri, SETTINGS_FILE));
    }
  }

  private declinedFolders(): string[] {
    return this.workspaceState.get<string[]>(DECLINED_FOLDERS_KEY, []);
  }
}

function isScript(document: vscode.TextDocument): boolean {
  const languages: string[] = [Language.Typed, Language.Plain];
  return document.uri.scheme === FILE_SCHEME && languages.includes(document.languageId);
}

async function chosenFolder(): Promise<vscode.WorkspaceFolder | undefined> {
  const active = vscode.window.activeTextEditor?.document.uri;
  const activeFolder = active === undefined ? undefined : vscode.workspace.getWorkspaceFolder(active);
  if (activeFolder !== undefined) {
    return activeFolder;
  }
  const folders = vscode.workspace.workspaceFolders ?? [];
  if (folders.length === 0) {
    void vscode.window.showInformationMessage(`Open a folder to create a ${SETTINGS_FILE} in.`);
    return undefined;
  }
  return folders.length === 1 ? folders[0] : vscode.window.showWorkspaceFolderPick();
}
