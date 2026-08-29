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
import { LdDocument } from './ldDocument';
import { resolveRunInvocation } from './ldCli';

/** Open documents by URI so split views share one undo stack. */
const documents = new Map<string, LdDocument>();

export class LdEditorProvider implements vscode.CustomEditorProvider<LdDocument> {
  private readonly _onDidChange =
    new vscode.EventEmitter<vscode.CustomDocumentContentChangeEvent<LdDocument>>();
  readonly onDidChangeCustomDocument = this._onDidChange.event;

  constructor(private readonly context: vscode.ExtensionContext) {}

  async openCustomDocument(
    uri: vscode.Uri,
    _openContext: { backupId?: string },
    _token: vscode.CancellationToken,
  ): Promise<LdDocument> {
    const key = uri.toString();
    const existing = documents.get(key);
    if (existing) {
      return existing;
    }
    const document = await LdDocument.open(uri);
    documents.set(key, document);
    document.onDidChange((event) => this._onDidChange.fire(event));
    return document;
  }

  async saveCustomDocument(
    document: LdDocument,
    _cancellation: vscode.CancellationToken,
  ): Promise<void> {
    await document.save();
  }

  async revertCustomDocument(
    document: LdDocument,
    _cancellation: vscode.CancellationToken,
  ): Promise<void> {
    await document.revert();
    for (const panel of this.panelsOf(document)) {
      this.postState(panel, document);
    }
  }

  async backupCustomDocument(
    document: LdDocument,
    context: { destination: vscode.Uri },
    _cancellation: vscode.CancellationToken,
  ): Promise<vscode.CustomDocumentBackup> {
    return document.backup(context);
  }

  async saveCustomDocumentAs(
    document: LdDocument,
    destination: vscode.Uri,
    _cancellation: vscode.CancellationToken,
  ): Promise<void> {
    await document.saveAs(destination);
    documents.delete(document.uri.toString());
    documents.set(destination.toString(), document);
  }

  /** Webview panels resolved for a document (for state pushes). */
  private readonly panels = new Map<string, Set<vscode.WebviewPanel>>();

  private panelsOf(document: LdDocument): Set<vscode.WebviewPanel> {
    return this.panels.get(document.uri.toString()) ?? new Set();
  }

  private postState(panel: vscode.WebviewPanel, document: LdDocument): void {
    void panel.webview.postMessage({ type: 'state', program: document.current });
  }

  async resolveCustomEditor(
    document: LdDocument,
    webviewPanel: vscode.WebviewPanel,
    _token: vscode.CancellationToken,
  ): Promise<void> {
    webviewPanel.webview.options = { enableScripts: true };
    webviewPanel.webview.html = this.getHtml(webviewPanel.webview);

    const key = document.uri.toString();
    if (!this.panels.has(key)) {
      this.panels.set(key, new Set());
    }
    this.panels.get(key)?.add(webviewPanel);
    webviewPanel.onDidDispose(() => {
      this.panels.get(key)?.delete(webviewPanel);
    });

    const post = (message: unknown): void => {
      void webviewPanel.webview.postMessage(message);
    };

    webviewPanel.webview.onDidReceiveMessage(async (data: unknown) => {
      let message;
      try {
        message = parseWebviewMessage(data);
      } catch (error) {
        post({ type: 'error', message: `protocol: ${(error as Error).message}` });
        return;
      }
      switch (message.type) {
        case 'ready':
          post({ type: 'load', text: JSON.stringify(document.current) });
          break;
        case 'edit':
          document.applyEdit(message.command);
          this.postState(webviewPanel, document);
          break;
        case 'undo':
        case 'redo': {
          if (message.type === 'undo') {
            document.undo();
          } else {
            document.redo();
          }
          this.postState(webviewPanel, document);
          break;
        }
        case 'modelChanged':
          document.replaceProgram(message.program);
          this.postState(webviewPanel, document);
          break;
        case 'save': {
          const buffer = Buffer.from(message.text, 'utf8');
          await vscode.workspace.fs.writeFile(document.uri, buffer);
          await document.revertToText(buffer.toString('utf8'));
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
      // 'unsafe-inline' covers style *attributes* — the inline-rename overlay
      // positions itself dynamically. Scripts stay nonce-gated.
      `style-src ${webview.cspSource} 'unsafe-inline'`,
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
  <div class="spacer"></div>
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
