/**
 * LD↔ST dual view (PLC-116): the report's dual-representation principle —
 * the same program editable as a diagram AND inspectable as Structured
 * Text. A read-only virtual document (`plc-ld-st:` scheme) backed by the
 * CLI's LD→ST conversion; re-running the command refreshes it.
 */

import { EventEmitter } from 'node:events';
import * as vscode from 'vscode';
import { capture } from './ldCapture';
import { resolveRunInvocation } from './ldCli';

/** Virtual scheme for generated ST views of .ld files. */
export const LD_ST_SCHEME = 'plc-ld-st';

/** Map a source .ld URI to its virtual ST view URI. */
export function stViewUri(source: vscode.Uri): vscode.Uri {
  const query = encodeURIComponent(source.toString());
  return vscode.Uri.parse(`${LD_ST_SCHEME}://generated/${encodeURIComponent(source.fsPath.split('/').pop() ?? 'program.st')}?${query}`);
}

/** Recover the source .ld URI from a virtual ST view URI. */
export function sourceOfStView(view: vscode.Uri): vscode.Uri | undefined {
  const encoded = view.query;
  if (encoded.length === 0) {
    return undefined;
  }
  const source = decodeURIComponent(encoded);
  return source.startsWith('file:') ? vscode.Uri.parse(source) : undefined;
}

/** Generate the ST text for an .ld file via the CLI. */
export async function generateStText(
  context: vscode.ExtensionContext,
  source: vscode.Uri,
): Promise<string> {
  const invocation = resolveRunInvocation(context, 'convert', [
    'ld',
    'st',
    source.fsPath,
  ]);
  return capture(invocation);
}

/** The command: open (or refresh) the generated ST for a .ld file. */
export async function showGeneratedSt(context: vscode.ExtensionContext): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  let source: vscode.Uri | undefined =
    editor?.document.languageId === 'ladder-diagram'
      ? editor.document.uri
      : editor?.document.uri.scheme === LD_ST_SCHEME
        ? sourceOfStView(editor.document.uri)
        : undefined;
  if (!source) {
    const picked = await vscode.window.showOpenDialog({
      canSelectMany: false,
      openLabel: 'Choose the .ld file',
      filters: { 'Ladder Diagram': ['ld'] },
    });
    source = picked?.[0];
  }
  if (!source) {
    return;
  }
  try {
    const text = await generateStText(context, source);
    const doc = await vscode.workspace.openTextDocument(stViewUri(source));
    // Refresh the visible view with the freshly generated text.
    void refreshStore(doc.uri, text);
    await vscode.window.showTextDocument(doc, { preview: true, viewColumn: vscode.ViewColumn.Beside });
  } catch (error) {
    void vscode.window.showErrorMessage(`LD → ST failed: ${(error as Error).message}`);
  }
}

/**
 * Tiny in-memory content store for the virtual documents. The provider
 * consults it first (so the command can push fresh text) and falls back
 * to generating on demand.
 */
const store = new Map<string, string>();
const changeEmitter = new EventEmitter();

function refreshStore(uri: vscode.Uri, text: string): void {
  store.set(uri.toString(), text);
  changeEmitter.emit('change', uri.toString());
}

/** Content provider for `plc-ld-st:` virtual documents. */
export class LdStContentProvider implements vscode.TextDocumentContentProvider {
  private readonly _onDidChange = new vscode.EventEmitter<vscode.Uri>();
  readonly onDidChange = this._onDidChange.event;

  constructor(private readonly context: vscode.ExtensionContext) {
    changeEmitter.on('change', (key: string) => {
      this._onDidChange.fire(vscode.Uri.parse(key));
    });
  }

  async provideTextDocumentContent(uri: vscode.Uri): Promise<string> {
    const cached = store.get(uri.toString());
    if (cached !== undefined) {
      return cached;
    }
    const source = sourceOfStView(uri);
    if (!source) {
      return '// no source .ld file associated with this view';
    }
    try {
      return await generateStText(this.context, source);
    } catch (error) {
      return `// LD → ST generation failed: ${(error as Error).message}`;
    }
  }

  dispose(): void {
    this._onDidChange.dispose();
  }
}
