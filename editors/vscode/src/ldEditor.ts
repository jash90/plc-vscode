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
import { parseWebviewMessage } from './ldWebview/protocol';
import { LdDocument } from './ldDocument';
import { resolveRunInvocation } from './ldCli';
import { SimClient, asSimChild, ServeEvent } from './ld/simClient';
import { capture } from './ldCapture';

/** Open documents by URI so split views share one undo stack. */
const documents = new Map<string, LdDocument>();

/** The resolved provider instance (set in resolveCustomEditor's owner) —
 * test hooks and commands reach the active document through it. */
let activeProvider: LdEditorProvider | undefined;

export interface SimulationEntry {
  client: SimClient;
  loadedJson?: string;
  lastPowerFlow?: unknown;
  lastWatch?: string[];
  scan?: number;
}

export class LdEditorProvider implements vscode.CustomEditorProvider<LdDocument> {
  private readonly _onDidChange =
    new vscode.EventEmitter<vscode.CustomDocumentContentChangeEvent<LdDocument>>();
  readonly onDidChangeCustomDocument = this._onDidChange.event;

  constructor(private readonly context: vscode.ExtensionContext) {}

  async openCustomDocument(
    uri: vscode.Uri,
    openContext: { backupId?: string },
    _token: vscode.CancellationToken,
  ): Promise<LdDocument> {
    const key = uri.toString();
    const existing = documents.get(key);
    if (existing) {
      return existing;
    }
    // backupId (hot-exit restore) wins over the file on disk.
    const document = await LdDocument.open(uri, openContext.backupId);
    documents.set(key, document);
    document.onDidChange((event) => this._onDidChange.fire(event));
    // Every content change (edits AND undo/redo) syncs all panels of the file.
    document.onDidChangeContent(() => this.postStateToAll(document));
    // Drop the cache entry when VS Code disposes the document.
    document.onDisposed((disposed) => {
      documents.delete(disposed.uri.toString());
      this.panels.delete(disposed.uri.toString());
    });
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
    this.postStateToAll(document);
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
    const panels = this.panels.get(document.uri.toString());
    if (panels) {
      this.panels.set(destination.toString(), panels);
      this.panels.delete(document.uri.toString());
    }
  }

  /** Webview panels resolved for a document (for state pushes). */
  private readonly panels = new Map<string, Set<vscode.WebviewPanel>>();

  private panelsOf(document: LdDocument): Set<vscode.WebviewPanel> {
    return this.panels.get(document.uri.toString()) ?? new Set();
  }

  private postState(panel: vscode.WebviewPanel, document: LdDocument): void {
    void panel.webview.postMessage({ type: 'state', program: document.current });
  }

  /** Sync the document state to every panel editing it (split views). */
  private postStateToAll(document: LdDocument): void {
    for (const panel of this.panelsOf(document)) {
      this.postState(panel, document);
    }
  }

  async resolveCustomEditor(
    document: LdDocument,
    webviewPanel: vscode.WebviewPanel,
    _token: vscode.CancellationToken,
  ): Promise<void> {
    activeProvider = this;
    webviewPanel.webview.options = { enableScripts: true };
    webviewPanel.webview.html = this.getHtml(webviewPanel.webview);

    const key = document.uri.toString();
    if (!this.panels.has(key)) {
      this.panels.set(key, new Set());
    }
    this.panels.get(key)?.add(webviewPanel);
    webviewPanel.onDidDispose(() => {
      this.panels.get(key)?.delete(webviewPanel);
      // Kill the simulation child when the last panel for the file closes —
      // no orphan `plc` processes.
      if ((this.panels.get(key)?.size ?? 0) === 0) {
        this.simulations.get(key)?.client.dispose();
        this.simulations.delete(key);
      }
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
      try {
        switch (message.type) {
          case 'ready':
            post({ type: 'load', text: JSON.stringify(document.current) });
            break;
          case 'edit':
            document.applyEdit(message.command);
            break;
          case 'undo':
            document.undo();
            break;
          case 'redo':
            document.redo();
            break;
          case 'modelChanged':
            document.replaceProgram(message.program);
            break;
          case 'save': {
            // Route through VS Code's save: dirty state clears, undo history
            // is kept, and the deterministic serialization is written. The
            // webview has already synced its model via edit/modelChanged.
            await vscode.commands.executeCommand('workbench.action.files.save');
            await this.updatePowerFlow(post, document.uri);
            break;
          }
          case 'run':
            await this.runLdFile(document.uri);
            break;
          case 'simStart':
          case 'simStop':
          case 'simStep':
          case 'simReset':
          case 'simInput': {
            const sim = await this.ensureSim(document);
            switch (message.type) {
              case 'simStart':
                // Reload only when the model changed — pause → resume must
                // not reset scan/timers/inputs.
                await this.reloadIfChanged(sim, document);
                sim.client.run(100);
                break;
              case 'simStop':
                sim.client.stop();
                break;
              case 'simStep': {
                await this.stepSimulation(sim, document);
                break;
              }
              case 'simReset':
                sim.client.stop();
                sim.client.reload(JSON.stringify(document.current));
                break;
              case 'simInput':
                sim.client.setInput(message.name, message.value);
                break;
              default:
                break;
            }
            break;
          }
          default:
            break;
        }
      } catch (error) {
        post({ type: 'error', message: (error as Error).message });
      }
    });
  }

  /** Compile LD → ST, run, and send power-flow JSON back to the webview. */
  /** Live simulations by document URI (lazy-started, disposed with panels). */
  private readonly simulations = new Map<string, SimulationEntry>();

  /** Test/debug access to the simulations map (read-only view). */
  simulationsView(): ReadonlyMap<string, SimulationEntry> {
    return this.simulations;
  }

  /** One scan from the CURRENT state — reload only if the model changed
   * (a raw reload resets scan and discards staged inputs). Shared by the
   * webview handler and the test hook so the invariant lives once. */
  async stepSimulation(sim: SimulationEntry, document: LdDocument): Promise<void> {
    sim.client.stop();
    await this.reloadIfChanged(sim, document);
    sim.client.tick();
  }

  /** Reload only when the serialized model differs from the last load. */
  async reloadIfChanged(
    sim: { client: SimClient; loadedJson?: string },
    document: LdDocument,
  ): Promise<void> {
    const json = JSON.stringify(document.current);
    if (sim.loadedJson !== json) {
      sim.client.reload(json);
      sim.loadedJson = json;
    }
  }

  /**
   * Lazily spawn the `plc ld --serve` child for a document and forward its
   * events to the webview as protocol messages.
   */
  async ensureSim(document: LdDocument): Promise<SimulationEntry> {
    const key = document.uri.toString();
    const existing = this.simulations.get(key);
    if (existing) {
      return existing;
    }
    const invocation = resolveRunInvocation(this.context, 'ld', ['--serve']);
    const child = spawn(
      invocation.command,
      invocation.args,
      invocation.cwd ? { cwd: invocation.cwd } : undefined,
    );
    // A failed spawn (missing binary, no cargo on PATH) surfaces as an
    // async 'error' event — without a listener it crashes the host.
    child.on('error', (error) => {
      this.simulations.delete(key);
      for (const panel of this.panels.get(key) ?? []) {
        void panel.webview.postMessage({
          type: 'error',
          message: `simulation failed to start: ${error.message}`,
        });
      }
    });
    const client = new SimClient(asSimChild(child), (event: ServeEvent) => {
      const entry = this.simulations.get(key);
      if (entry) {
        if (event.event === 'powerFlow') {
          entry.lastPowerFlow = event;
        } else if (event.event === 'state') {
          entry.lastWatch = event.watch as string[];
          entry.scan = event.scan as number;
        }
      }
      this.forwardServeEvent(key, event);
    });
    client.start();
    const entry = { client };
    this.simulations.set(key, entry);
    this.context.subscriptions.push({
      dispose: () => {
        entry.client.dispose();
        this.simulations.delete(key);
      },
    });
    return entry;
  }

  /** Translate serve events into webview protocol messages for all panels. */
  private forwardServeEvent(key: string, event: ServeEvent): void {
    const panels = this.panels.get(key) ?? new Set<vscode.WebviewPanel>();
    const post = (message: unknown): void => {
      for (const panel of panels) {
        void panel.webview.postMessage(message);
      }
    };
    switch (event.event) {
      case 'state':
        post({
          type: 'simState',
          scan: event.scan,
          timeMs: event.timeMs,
          watch: event.watch,
          forced: event.forced,
        });
        break;
      case 'powerFlow':
        post({ type: 'powerFlow', json: JSON.stringify({ rungs: event.rungs }) });
        break;
      case 'diagnostics': {
        // Summarize LD codes so a failing model is never a dead end.
        const items = Array.isArray(event.items) ? event.items : [];
        const summary = items
          .map((item) => {
            const code = (item as { code?: unknown }).code;
            const message = (item as { message?: unknown }).message;
            return `${code}: ${message}`;
          })
          .join('; ');
        if (summary.length > 0) {
          post({ type: 'error', message: summary });
        }
        break;
      }
      case 'error':
        post({ type: 'error', message: String(event.message) });
        break;
      default:
        break;
    }
  }

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
  <button id="btn-sim-run" title="Run the simulation continuously (no save needed)">▶ Sim</button>
  <button id="btn-sim-pause" title="Pause the simulation">⏸</button>
  <button id="btn-sim-step" title="One scan">⏭</button>
  <button id="btn-sim-reset" title="Reset the simulation">⟲</button>
  <button id="btn-toggle-json">JSON</button>
</div>
<div id="canvas-container">
  <div id="ld-canvas"></div>
  <textarea id="ld-textarea" spellcheck="false"></textarea>
</div>
<div id="status-bar">Ready.</div>
<div id="sim-panel"></div>
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


/**
 * The ACTIVE LD document (test hooks): the active tab's document first,
 * then any open tab's. With requireTab, no document is returned unless a
 * tab actually shows it — mutating hooks use this so they can never hit
 * an arbitrary unfocused document.
 */
export function activeLdDocument(requireTab = false): LdDocument | undefined {
  if (!activeProvider) {
    return undefined;
  }
  const byTab = (tab: vscode.Tab | undefined): LdDocument | undefined => {
    const input = (tab?.input as { uri?: vscode.Uri } | undefined)?.uri;
    return input ? documents.get(input.toString()) : undefined;
  };
  const active = byTab(vscode.window.tabGroups.activeTabGroup?.activeTab);
  if (active) {
    return active;
  }
  for (const group of vscode.window.tabGroups.all) {
    for (const tab of group.tabs) {
      const doc = byTab(tab);
      if (doc) {
        return doc;
      }
    }
  }
  return requireTab ? undefined : [...documents.values()][0];
}

/** The resolved provider (test hooks). */
export function activeLdProvider(): LdEditorProvider | undefined {
  return activeProvider;
}

/** The simulation entry for the active document (test hooks). */
export function activeLdSimulation(): SimulationEntry | undefined {
  const provider = activeProvider;
  const doc = activeLdDocument();
  if (!provider || !doc) {
    return undefined;
  }
  return provider.simulationsView().get(doc.uri.toString());
}
