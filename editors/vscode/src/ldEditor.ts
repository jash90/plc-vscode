/**
 * Ladder Diagram (LD) custom editor provider — thin host.
 *
 * The webview (media/ldEditor/main.js, bundled from src/ldWebview/) owns the
 * interactive UI; this host shuttles typed protocol messages, runs the CLI
 * (`plc ld --watch` for power-flow, `plc ld` for execution) through the
 * shared `resolveRunInvocation`, and writes files.
 *
 * The webview HTML enforces a Content-Security-Policy: no remote sources,
 * styles from the extension's cspSource, scripts only with a per-load nonce.
 */

import * as vscode from 'vscode';
import { spawn } from 'node:child_process';
import * as path from 'node:path';
import { parseWebviewMessage } from './ldWebview/protocol';
import { resolveRunInvocation } from './ldCli';

export class LdEditorProvider implements vscode.CustomEditorProvider<vscode.CustomDocument> {
  private readonly _onDidChange =
    new vscode.EventEmitter<vscode.CustomDocumentContentChangeEvent<vscode.CustomDocument>>();
  readonly onDidChangeCustomDocument = this._onDidChange.event;

  constructor(private readonly context: vscode.ExtensionContext) {}

  async openCustomDocument(
    uri: vscode.Uri,
    _openContext: { backupId?: string },
    _token: vscode.CancellationToken,
  ): Promise<vscode.CustomDocument> {
    return {
      uri,
      fileName: path.basename(uri.fsPath),
      dispose: () => {},
    } as vscode.CustomDocument;
  }

  async saveCustomDocument(
    _document: vscode.CustomDocument,
    _cancellation: vscode.CancellationToken,
  ): Promise<void> {
    // Delegated to the standard document save in resolveCustomEditor.
  }

  async revertCustomDocument(
    _document: vscode.CustomDocument,
    _cancellation: vscode.CancellationToken,
  ): Promise<void> {
    // No-op: the webview is the source of truth.
  }

  async backupCustomDocument(
    _document: vscode.CustomDocument,
    _context: { destination: vscode.Uri },
    _cancellation: vscode.CancellationToken,
  ): Promise<{ id: string; delete(): void }> {
    return { id: '', delete: () => {} };
  }

  async saveCustomDocumentAs(
    _document: vscode.CustomDocument,
    _destination: vscode.Uri,
    _cancellation: vscode.CancellationToken,
  ): Promise<void> {
    // No-op: handled via WorkspaceEdit in resolveCustomEditor.
  }

  async resolveCustomEditor(
    document: vscode.CustomDocument,
    webviewPanel: vscode.WebviewPanel,
    _token: vscode.CancellationToken,
  ): Promise<void> {
    webviewPanel.webview.options = { enableScripts: true };
    webviewPanel.webview.html = this.getHtml(webviewPanel.webview);

    const post = (message: unknown): void => {
      void webviewPanel.webview.postMessage(message);
    };

    const content = await vscode.workspace.fs.readFile(document.uri);
    post({ type: 'load', text: Buffer.from(content).toString('utf8') });

    webviewPanel.webview.onDidReceiveMessage(async (data: unknown) => {
      let message;
      try {
        message = parseWebviewMessage(data);
      } catch (error) {
        post({ type: 'error', message: `protocol: ${(error as Error).message}` });
        return;
      }
      switch (message.type) {
        case 'save': {
          const buffer = Buffer.from(message.text, 'utf8');
          await vscode.workspace.fs.writeFile(document.uri, buffer);
          await this.updatePowerFlow(post, document.uri);
          break;
        }
        case 'run':
          await this.runLdFile(document.uri);
          break;
        default:
          break;
      }
    });
  }

  /** Compile LD → ST, run, and send power-flow JSON back to the webview. */
  private async updatePowerFlow(
    post: (message: unknown) => void,
    uri: vscode.Uri,
  ): Promise<void> {
    try {
      const invocation = resolveRunInvocation(this.context, 'ld', [uri.fsPath, '--watch']);
      const result = await capture(invocation);
      post({ type: 'powerFlow', json: result });
    } catch (error) {
      vscode.window.showWarningMessage(
        `LD power-flow evaluation failed: ${(error as Error).message}`,
      );
    }
  }

  /** Run the LD file via the CLI, streaming output to the LD channel. */
  private async runLdFile(uri: vscode.Uri): Promise<void> {
    try {
      const invocation = resolveRunInvocation(this.context, 'ld', [uri.fsPath]);
      const output = vscode.window.createOutputChannel('PLC LD');
      output.show(true);
      output.appendLine(`$ ${invocation.command} ${invocation.args.join(' ')}`);

      const child = spawn(
        invocation.command,
        invocation.args,
        invocation.cwd ? { cwd: invocation.cwd } : undefined,
      );
      child.stdout.on('data', (chunk: Buffer) => output.append(chunk.toString()));
      child.stderr.on('data', (chunk: Buffer) => output.append(chunk.toString()));
      child.on('close', (code: number | null) => {
        output.appendLine(
          code === 0 ? 'LD execution completed.' : `LD execution failed with exit code ${code}.`,
        );
      });
    } catch (error) {
      vscode.window.showErrorMessage(`LD run failed: ${(error as Error).message}`);
    }
  }

  /** The webview shell: CSP-hardened, external bundle + stylesheet. */
  private getHtml(webview: vscode.Webview): string {
    const nonce = getNonce();
    const scriptUri = webview.asWebviewUri(
      vscode.Uri.joinPath(this.context.extensionUri, 'media', 'ldEditor', 'main.js'),
    );
    const styleUri = webview.asWebviewUri(
      vscode.Uri.joinPath(this.context.extensionUri, 'media', 'ldEditor', 'main.css'),
    );
    const csp = [
      "default-src 'none'",
      `style-src ${webview.cspSource}`,
      `script-src 'nonce-${nonce}'`,
      `img-src ${webview.cspSource} data:`,
    ].join('; ');

    return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta http-equiv="Content-Security-Policy" content="${csp}">
<title>Ladder Diagram Editor</title>
<link rel="stylesheet" href="${styleUri}">
</head>
<body>
<div id="toolbar">
  <div id="palette"></div>
  <div style="flex:1"></div>
  <button id="btn-save">Save</button>
  <button id="btn-run">Run</button>
  <button id="btn-toggle-json">JSON</button>
</div>
<div id="canvas-container">
  <div id="ld-canvas"></div>
  <textarea id="ld-textarea" spellcheck="false"></textarea>
</div>
<div id="status-bar">Ready.</div>
<script nonce="${nonce}" src="${scriptUri}"></script>
</body>
</html>`;
  }
}

function getNonce(): string {
  const possible = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
  let text = '';
  for (let i = 0; i < 32; i += 1) {
    text += possible.charAt(Math.floor(Math.random() * possible.length));
  }
  return text;
}

/** Run an invocation and resolve with stdout (reject on non-zero exit). */
function capture(invocation: {
  command: string;
  args: string[];
  cwd?: string;
}): Promise<string> {
  return new Promise<string>((resolve, reject) => {
    const child = spawn(
      invocation.command,
      invocation.args,
      invocation.cwd ? { cwd: invocation.cwd } : undefined,
    );
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (chunk: Buffer) => {
      stdout += chunk.toString();
    });
    child.stderr.on('data', (chunk: Buffer) => {
      stderr += chunk.toString();
    });
    child.on('close', (code: number | null) => {
      if (code === 0) {
        resolve(stdout);
      } else {
        reject(new Error(stderr || `Exit code ${code}`));
      }
    });
    child.on('error', reject);
  });
}
