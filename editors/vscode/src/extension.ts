import * as path from 'node:path';
import { spawn } from 'node:child_process';
import * as vscode from 'vscode';
import { LanguageClient, LanguageClientOptions, ServerOptions } from 'vscode-languageclient/node';

let client: LanguageClient | undefined;
let outputChannel: vscode.OutputChannel | undefined;

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
  outputChannel = vscode.window.createOutputChannel('PLC VS Code');
  context.subscriptions.push(outputChannel);

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



  context.subscriptions.push(
    vscode.commands.registerCommand('plc-vscode.runCurrentFile', async () => {
      await runCurrentStructuredTextFile(context);
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


async function runCurrentStructuredTextFile(context: vscode.ExtensionContext): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  if (!editor) {
    await vscode.window.showWarningMessage('Open a Structured Text file before running PLC VS Code.');
    return;
  }

  if (editor.document.isDirty) {
    await editor.document.save();
  }

  const config = vscode.workspace.getConfiguration('plcVscode');
  const repositoryRoot = config.get<string>('repositoryRoot', '') || workspaceRoot(context);
  const command = config.get<string>('cliCommand', 'cargo');
  const args = [...config.get<string[]>('cliArgs', ['run', '--quiet', '--package', 'plc_cli', '--', 'run']), editor.document.uri.fsPath];

  outputChannel?.clear();
  outputChannel?.appendLine(`$ ${command} ${args.join(' ')}`);
  outputChannel?.show(true);

  await new Promise<void>((resolve) => {
    const child = spawn(command, args, { cwd: repositoryRoot });
    child.stdout.on('data', (chunk: Buffer) => outputChannel?.append(chunk.toString()));
    child.stderr.on('data', (chunk: Buffer) => outputChannel?.append(chunk.toString()));
    child.on('error', async (error: Error) => {
      outputChannel?.appendLine(`Failed to run Structured Text: ${error.message}`);
      await vscode.window.showErrorMessage(`PLC VS Code run failed: ${error.message}`);
      resolve();
    });
    child.on('close', async (code: number | null) => {
      if (code === 0) {
        await vscode.window.showInformationMessage('PLC VS Code run completed.');
      } else {
        await vscode.window.showErrorMessage(`PLC VS Code run failed with exit code ${code}.`);
      }
      resolve();
    });
  });
}
