/**
 * Ladder Diagram (LD) custom editor provider for VS Code.
 *
 * Reads/writes `.ld` JSON files and renders an interactive Canvas-based ladder
 * diagram editor in a webview.  After each edit the editor compiles the LD model
 * via the CLI (`plc ld --watch`) to get a power-flow result and colors elements
 * green (energized) or gray (de-energized).
 */

import * as vscode from 'vscode';
import { spawn } from 'node:child_process';
import * as path from 'node:path';

/** Palette of LD elements the user can drag onto the canvas. */
const ELEMENT_PALETTE = [
  { type: 'no-contact', label: '| |', title: 'Normally-Open Contact' },
  { type: 'nc-contact', label: '|/|', title: 'Normally-Closed Contact' },
  { type: 'coil', label: '( )', title: 'Coil (Normal)' },
  { type: 'set-coil', label: '(S)', title: 'SET Coil' },
  { type: 'reset-coil', label: '(R)', title: 'RESET Coil' },
  { type: 'ton', label: 'TON', title: 'Timer On Delay' },
  { type: 'ctu', label: 'CTU', title: 'Count Up' },
];

export class LdEditorProvider implements vscode.CustomEditorProvider<vscode.CustomDocument> {
  /** Fired when the document changes (required by the interface). */
  private readonly _onDidChange = new vscode.EventEmitter<vscode.CustomDocumentContentChangeEvent<vscode.CustomDocument>>();
  readonly onDidChangeCustomDocument = this._onDidChange.event;

  constructor(private readonly context: vscode.ExtensionContext) {}

  /** Called when a `.ld` file is opened. */
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

  /** Called to back up a document during hot-exit. */
  async saveCustomDocument(
    document: vscode.CustomDocument,
    cancellation: vscode.CancellationToken,
  ): Promise<void> {
    // Delegated to the standard document save in resolveCustomEditor.
  }

  /** Called to revert a document. */
  async revertCustomDocument(
    document: vscode.CustomDocument,
    cancellation: vscode.CancellationToken,
  ): Promise<void> {
    // No-op: the webview is the source of truth.
  }

  /** Called to save the document to a different location. */
  async backupCustomDocument(
    document: vscode.CustomDocument,
    context: { destination: vscode.Uri },
    cancellation: vscode.CancellationToken,
  ): Promise<{ id: string; delete(): void }> {
    return { id: '', delete: () => {} };
  }

  /** Called to save the document. */
  async saveCustomDocumentAs(
    document: vscode.CustomDocument,
    destination: vscode.Uri,
    cancellation: vscode.CancellationToken,
  ): Promise<void> {
    // No-op: handled via WorkspaceEdit in resolveCustomEditor.
  }

  /** Called to create the webview for a custom document. */
  async resolveCustomEditor(
    document: vscode.CustomDocument,
    webviewPanel: vscode.WebviewPanel,
    _token: vscode.CancellationToken,
  ): Promise<void> {
    webviewPanel.webview.options = {
      enableScripts: true,
    };

    webviewPanel.webview.html = this.getHtml(webviewPanel.webview);

    // Send initial content to the webview.
    const content = await vscode.workspace.fs.readFile(document.uri);
    webviewPanel.webview.postMessage({
      type: 'load',
      text: Buffer.from(content).toString('utf8'),
    });

    // Listen for save requests from the webview.
    webviewPanel.webview.onDidReceiveMessage(async (message) => {
      if (message.type === 'save') {
        const content = Buffer.from(message.text, 'utf8');
        await vscode.workspace.fs.writeFile(document.uri, content);
        // After saving, compile and evaluate power-flow.
        await this.updatePowerFlow(webviewPanel.webview, document.uri);
      } else if (message.type === 'run') {
        await this.runLdFile(document.uri);
      }
    });
  }

  /** Compile LD → ST, run, and send power-flow JSON back to the webview. */
  private async updatePowerFlow(webview: vscode.Webview, uri: vscode.Uri): Promise<void> {
    try {
      const invocation = this.resolveLdInvocation(uri.fsPath, '--watch');
      const result = await new Promise<string>((resolve, reject) => {
        const child = spawn(invocation.command, invocation.args, invocation.options);
        let stdout = '';
        let stderr = '';
        child.stdout.on('data', (chunk: Buffer) => (stdout += chunk.toString()));
        child.stderr.on('data', (chunk: Buffer) => (stderr += chunk.toString()));
        child.on('close', (code: number | null) => {
          if (code === 0) resolve(stdout);
          else reject(new Error(stderr || `Exit code ${code}`));
        });
        child.on('error', reject);
      });

      // Parse the power-flow JSON from stdout.
      webview.postMessage({ type: 'powerFlow', json: result });
    } catch (error) {
      vscode.window.showWarningMessage(
        `LD power-flow evaluation failed: ${(error as Error).message}`,
      );
    }
  }

  /** Run the LD file via the CLI. */
  private async runLdFile(uri: vscode.Uri): Promise<void> {
    try {
      const invocation = this.resolveLdInvocation(uri.fsPath);
      const output = vscode.window.createOutputChannel('PLC LD');
      output.show(true);
      output.appendLine(`$ ${invocation.command} ${invocation.args.join(' ')}`);

      const child = spawn(invocation.command, invocation.args, invocation.options);
      child.stdout.on('data', (chunk: Buffer) => output.append(chunk.toString()));
      child.stderr.on('data', (chunk: Buffer) => output.append(chunk.toString()));
      child.on('close', (code: number | null) => {
        if (code === 0) {
          output.appendLine('LD execution completed.');
        } else {
          output.appendLine(`LD execution failed with exit code ${code}.`);
        }
      });
    } catch (error) {
      vscode.window.showErrorMessage(`LD run failed: ${(error as Error).message}`);
    }
  }

  /** Resolve the CLI invocation for the `ld` subcommand. */
  private resolveLdInvocation(
    filePath: string,
    ...extraArgs: string[]
  ): { command: string; args: string[]; options?: { cwd?: string } } {
    if (this.context.extensionMode === vscode.ExtensionMode.Production) {
      return {
        command: this.context.asAbsolutePath('./dist/plc'),
        args: ['ld', filePath, ...extraArgs],
      };
    }
    const config = vscode.workspace.getConfiguration('plcVscode');
    const command = config.get<string>('cliCommand', 'cargo');
    const cliArgs = config.get<string[]>('cliArgs', [
      'run',
      '--quiet',
      '--package',
      'plc_cli',
      '--',
      'run',
    ]);
    const repositoryRoot =
      config.get<string>('repositoryRoot', '') ||
      path.resolve(this.context.extensionPath, '..', '..');
    // Replace the trailing 'run' subcommand with 'ld'.
    const cargoPrefix = cliArgs.slice(0, -1);
    return {
      command,
      args: [...cargoPrefix, 'ld', filePath, ...extraArgs],
      options: { cwd: repositoryRoot },
    };
  }

  /** The HTML/JS content for the webview. */
  private getHtml(webview: vscode.Webview): string {
    const nonce = getNonce();

    return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Ladder Diagram Editor</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body {
    font-family: var(--vscode-font-family, monospace);
    background: var(--vscode-editor-background, #1e1e1e);
    color: var(--vscode-editor-foreground, #d4d4d4);
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  #toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px;
    border-bottom: 1px solid var(--vscode-panel-border, #333);
    flex-shrink: 0;
  }
  #palette {
    display: flex;
    gap: 6px;
  }
  .palette-item {
    padding: 4px 10px;
    border: 1px solid var(--vscode-button-border, #555);
    border-radius: 3px;
    cursor: pointer;
    font-size: 13px;
    background: var(--vscode-button-secondaryBackground, #3a3d41);
    color: var(--vscode-button-secondaryForeground, #fff);
  }
  .palette-item:hover {
    background: var(--vscode-button-hoverBackground, #454545);
  }
  #toolbar button {
    padding: 4px 12px;
    cursor: pointer;
    border: 1px solid var(--vscode-button-border, #0e639c);
    border-radius: 2px;
    background: var(--vscode-button-background, #0e639c);
    color: var(--vscode-button-foreground, #fff);
    font-size: 12px;
  }
  #canvas-container {
    flex: 1;
    overflow: auto;
    padding: 16px;
  }
  #ld-textarea {
    width: 100%;
    min-height: 120px;
    margin-top: 8px;
    font-family: monospace;
    font-size: 12px;
    background: var(--vscode-input-background, #3c3c3c);
    color: var(--vscode-input-foreground, #d4d4d4);
    border: 1px solid var(--vscode-input-border, #555);
    padding: 8px;
    display: none;
  }
  .rung {
    display: flex;
    align-items: center;
    min-height: 40px;
    margin-bottom: 4px;
    border-bottom: 1px dashed #333;
    padding: 0 8px;
  }
  .left-rail, .right-rail {
    width: 3px;
    min-height: 30px;
    background: #888;
    flex-shrink: 0;
  }
  .element {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 50px;
    height: 30px;
    margin: 0 4px;
    border: 1px solid var(--vscode-editorWidget-border, #666);
    border-radius: 2px;
    font-size: 12px;
    cursor: pointer;
  }
  .element.energized {
    border-color: #4caf50;
    background: rgba(76, 175, 80, 0.15);
    color: #66bb6a;
  }
  .element.not-energized {
    opacity: 0.6;
  }
  .wire {
    width: 20px;
    height: 2px;
    background: #555;
    flex-shrink: 0;
  }
  .wire.energized {
    background: #4caf50;
  }
  #status-bar {
    padding: 4px 8px;
    border-top: 1px solid var(--vscode-panel-border, #333);
    font-size: 11px;
    color: var(--vscode-descriptionForeground, #888);
    flex-shrink: 0;
  }
</style>
</head>
<body>
  <div id="toolbar">
    <div id="palette">
      ${ELEMENT_PALETTE.map(
        (e) => `<div class="palette-item" data-type="${e.type}" title="${e.title}">${e.label}</div>`,
      ).join('')}
    </div>
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

<script nonce="${nonce}">
  const vscode = acquireVsCodeApi();
  let ldProgram = null;
  let powerFlow = null;

  // Receive messages from the extension.
  window.addEventListener('message', (event) => {
    const msg = event.data;
    if (msg.type === 'load') {
      try {
        ldProgram = JSON.parse(msg.text);
      } catch (e) {
        ldProgram = { name: 'NewProgram', rungs: [] };
      }
      renderCanvas();
      updateTextarea();
    } else if (msg.type === 'powerFlow') {
      try {
        powerFlow = JSON.parse(msg.json);
        renderCanvas();
      } catch (e) {
        document.getElementById('status-bar').textContent = 'Power-flow parse error: ' + e.message;
      }
    }
  });

  // Palette drag: add a new rung with the selected element type.
  document.querySelectorAll('.palette-item').forEach((item) => {
    item.addEventListener('click', () => {
      addElement(item.dataset.type);
    });
  });

  function addElement(type) {
    if (!ldProgram) ldProgram = { name: 'NewProgram', rungs: [] };

    let element;
    switch (type) {
      case 'no-contact':
        element = { name: 'NewVar', negated: false };
        break;
      case 'nc-contact':
        element = { name: 'NewVar', negated: true };
        break;
      case 'coil':
      case 'set-coil':
      case 'reset-coil':
        const variant = type === 'coil' ? 'normal' : type === 'set-coil' ? 'set' : 'reset';
        const coil = { kind: 'coil', name: 'OutVar', variant };
        if (ldProgram.rungs.length === 0) {
          ldProgram.rungs.push({ branches: [{ elements: [] }], outputs: [] });
        }
        ldProgram.rungs[ldProgram.rungs.length - 1].outputs.push(coil);
        renderCanvas();
        updateTextarea();
        return;
      case 'ton':
      case 'ctu':
        const fbType = type.toUpperCase();
        const fb = {
          kind: 'block',
          fb_type: fbType,
          instance: fbType + '_inst',
          inputs: [
            { name: type === 'ton' ? 'IN' : 'CU', value: 'NewVar' },
            ...(type === 'ton'
              ? [{ name: 'PT', value: 'T#1s' }]
              : [{ name: 'PV', value: '10' }]),
          ],
          outputs: [{ name: 'Q', value: 'Done' }],
        };
        if (ldProgram.rungs.length === 0) {
          ldProgram.rungs.push({ branches: [{ elements: [] }], outputs: [] });
        }
        ldProgram.rungs[ldProgram.rungs.length - 1].outputs.push(fb);
        renderCanvas();
        updateTextarea();
        return;
    }

    // Add contact to the last rung's first branch.
    if (ldProgram.rungs.length === 0) {
      ldProgram.rungs.push({ branches: [{ elements: [] }], outputs: [] });
    }
    const rung = ldProgram.rungs[ldProgram.rungs.length - 1];
    if (rung.branches.length === 0) {
      rung.branches.push({ elements: [] });
    }
    rung.branches[0].elements.push(element);
    renderCanvas();
    updateTextarea();
  }

  function renderCanvas() {
    const canvas = document.getElementById('ld-canvas');
    canvas.innerHTML = '';

    if (!ldProgram || !ldProgram.rungs) return;

    ldProgram.rungs.forEach((rung, rungIdx) => {
      const rungDiv = document.createElement('div');
      rungDiv.className = 'rung';

      const rail = document.createElement('div');
      rail.className = 'left-rail';
      rungDiv.appendChild(rail);

      // Determine if this rung is energized.
      const rungEnergized = powerFlow && powerFlow.rungs[rungIdx]
        ? powerFlow.rungs[rungIdx].rung_result
        : false;

      // Render contacts in the first branch (series/AND).
      if (rung.branches && rung.branches.length > 0) {
        rung.branches.forEach((branch, branchIdx) => {
          if (branch.elements) {
            branch.elements.forEach((contact, contactIdx) => {
              const wire = document.createElement('div');
              wire.className = 'wire' + (rungEnergized ? ' energized' : '');
              rungDiv.appendChild(wire);

              const el = document.createElement('div');
              const branchEnergized = powerFlow && powerFlow.rungs[rungIdx] &&
                powerFlow.rungs[rungIdx].branch_energized &&
                powerFlow.rungs[rungIdx].branch_energized[branchIdx];
              el.className = 'element' + (branchEnergized ? ' energized' : ' not-energized');
              const symbol = contact.negated ? '|/|' : '| |';
              el.textContent = symbol + ' ' + contact.name;
              el.title = 'Click to rename';
              el.addEventListener('click', () => {
                const name = prompt('Variable name:', contact.name);
                if (name !== null) {
                  contact.name = name;
                  renderCanvas();
                  updateTextarea();
                }
              });
              rungDiv.appendChild(el);
            });
          }
        });
      }

      // Wire to outputs.
      const wireOut = document.createElement('div');
      wireOut.style.flex = '1';
      wireOut.style.maxWidth = '40px';
      wireOut.style.height = '2px';
      wireOut.style.background = rungEnergized ? '#4caf50' : '#555';
      rungDiv.appendChild(wireOut);

      // Render outputs.
      if (rung.outputs) {
        rung.outputs.forEach((output, outIdx) => {
          const outEnergized = powerFlow && powerFlow.rungs[rungIdx] &&
            powerFlow.rungs[rungIdx].output_energized &&
            powerFlow.rungs[rungIdx].output_energized[outIdx];
          const el = document.createElement('div');
          el.className = 'element' + (outEnergized ? ' energized' : ' not-energized');
          if (output.kind === 'coil') {
            const symbol = output.variant === 'set' ? '(S)' : output.variant === 'reset' ? '(R)' : '( )';
            el.textContent = symbol + ' ' + output.name;
          } else if (output.kind === 'block') {
            el.textContent = output.fb_type + ' [' + output.instance + ']';
          }
          el.title = 'Click to rename';
          el.addEventListener('click', () => {
            if (output.kind === 'coil') {
              const name = prompt('Variable name:', output.name);
              if (name !== null) {
                output.name = name;
                renderCanvas();
                updateTextarea();
              }
            }
          });
          rungDiv.appendChild(el);
        });
      }

      const rightRail = document.createElement('div');
      rightRail.className = 'right-rail';
      rungDiv.appendChild(rightRail);

      canvas.appendChild(rungDiv);
    });

    // Update status bar.
    if (powerFlow) {
      const energized = powerFlow.rungs.filter(r => r.rung_result).length;
      document.getElementById('status-bar').textContent =
        powerFlow.rungs.length + ' rungs, ' + energized + ' energized.';
    } else {
      document.getElementById('status-bar').textContent =
        (ldProgram.rungs ? ldProgram.rungs.length : 0) + ' rungs. Save to evaluate power-flow.';
    }
  }

  function updateTextarea() {
    document.getElementById('ld-textarea').value = JSON.stringify(ldProgram, null, 2);
  }

  document.getElementById('btn-save').addEventListener('click', () => {
    const text = document.getElementById('ld-textarea').style.display !== 'none'
      ? document.getElementById('ld-textarea').value
      : JSON.stringify(ldProgram, null, 2);
    vscode.postMessage({ type: 'save', text });
  });

  document.getElementById('btn-run').addEventListener('click', () => {
    vscode.postMessage({ type: 'run' });
  });

  document.getElementById('btn-toggle-json').addEventListener('click', () => {
    const ta = document.getElementById('ld-textarea');
    ta.style.display = ta.style.display === 'none' ? 'block' : 'none';
  });

  // Sync textarea changes back to model.
  document.getElementById('ld-textarea').addEventListener('input', (e) => {
    try {
      ldProgram = JSON.parse(e.target.value);
      renderCanvas();
    } catch (err) {
      // Ignore parse errors while typing.
    }
  });
</script>
</body>
</html>`;
  }
}

function getNonce(): string {
  let text = '';
  const possible = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
  for (let i = 0; i < 32; i++) {
    text += possible.charAt(Math.floor(Math.random() * possible.length));
  }
  return text;
}
