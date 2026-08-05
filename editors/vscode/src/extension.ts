import { execFile } from "node:child_process";
import { promisify } from "node:util";

import * as vscode from "vscode";
import {
  LanguageClient,
  type LanguageClientOptions,
  type ServerOptions,
} from "vscode-languageclient/node";

const exec = promisify(execFile);
const output = vscode.window.createOutputChannel("Zoning");
let client: LanguageClient | undefined;

function executable(): string {
  return vscode.workspace
    .getConfiguration("zoning")
    .get<string>("executablePath", "zoning");
}

async function run(action: "run" | "status"): Promise<void> {
  const command = executable();
  try {
    const { stdout, stderr } = await exec(command, ["setup", action], {
      cwd: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
    });
    output.clear();
    output.append(stdout);
    output.append(stderr);
    output.show(true);
  } catch (error: unknown) {
    const detail = error instanceof Error ? error.message : String(error);
    void vscode.window.showErrorMessage(
      `Zoning could not run \`${command} setup ${action}\`: ${detail}`,
    );
  }
}

export async function activate(
  context: vscode.ExtensionContext,
): Promise<void> {
  context.subscriptions.push(
    output,
    vscode.commands.registerCommand("zoning.setup", () => run("run")),
    vscode.commands.registerCommand("zoning.status", () => run("status")),
  );

  const cwd = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  const serverOptions: ServerOptions = {
    command: executable(),
    args: ["lsp", "--stdio"],
    ...(cwd ? { options: { cwd } } : {}),
  };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { language: "zoning", scheme: "file" },
      { language: "zoning", scheme: "untitled" },
    ],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher("**/*.zone"),
    },
    outputChannel: output,
  };

  client = new LanguageClient(
    "zoning",
    "Zoning Language Server",
    serverOptions,
    clientOptions,
  );
  await client.start();
}

export async function deactivate(): Promise<void> {
  const running = client;
  client = undefined;
  await running?.stop();
}
