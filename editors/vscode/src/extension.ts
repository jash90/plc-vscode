import * as path from 'node:path';
import * as vscode from 'vscode';
import { LanguageClient, LanguageClientOptions, ServerOptions } from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

function workspaceRoot(context: vscode.ExtensionContext): string {
  return path.resolve(context.extensionPath, '..', '..');
}

function serverOptions(context: vscode.ExtensionContext): ServerOptions {
  const config = vscode.workspace.getConfiguration('plcVscode');
  const command = config.get<string>('serverCommand', 'cargo');
  const args = config.get<string[]>('serverArgs', [
    'run',
    '--package',
    'plc_lsp_server',
    '--bin',
    'plc-lsp-server',
    '--',
  ]);

  return {
    command,
    args,
    options: {
      cwd: workspaceRoot(context),
    },
  };
}

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
  status.text = 'PLC VS Code';
  status.tooltip = 'PLC VS Code language support is active';
  status.command = 'plc-vscode.showStatus';
  status.show();
  context.subscriptions.push(status);

  context.subscriptions.push(
    vscode.commands.registerCommand('plc-vscode.showStatus', async () => {
      const state = client ? 'running' : 'not started';
      await vscode.window.showInformationMessage(`PLC VS Code extension active; language server is ${state}.`);
    }),
  );

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: 'file', language: 'structured-text' }],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher('**/*.{st,iecst,plcst}'),
    },
  };

  client = new LanguageClient(
    'plc-vscode-lsp',
    'PLC VS Code Language Server',
    serverOptions(context),
    clientOptions,
  );

  context.subscriptions.push(client);
  await client.start();
}

export async function deactivate(): Promise<void> {
  const runningClient = client;
  client = undefined;
  if (runningClient) {
    await runningClient.stop();
  }
}
