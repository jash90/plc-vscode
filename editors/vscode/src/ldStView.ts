/**
 * LD↔ST dual view (PLC-116): the report's dual-representation principle —
 * the same program editable as a diagram AND inspectable as Structured
 * Text. A read-only virtual document (`plc-ld-st:` scheme) backed by the
 * CLI's LD→ST conversion. The view always reflects the last SAVED state
 * of the .ld file (the command saves first).
 */

import * as path from 'node:path';
import * as vscode from 'vscode';
import { capture } from './ldCapture';
import { resolveRunInvocation } from './ldCli';

/** Virtual scheme for generated ST views of .ld files. */
export const LD_ST_SCHEME = 'plc-ld-st';

/** Map a source .ld URI to its virtual ST view URI. */
export function stViewUri(source: vscode.Uri): vscode.Uri {
  // The .st suffix gives the view the structured-text grammar (highlighting)
  // and a tab title distinct from the diagram's.
  const name = path.basename(source.fsPath).replace(/\.ld$/i, '.st');
  const query = encodeURIComponent(source.toString());
  return vscode.Uri.parse(
    `${LD_ST_SCHEME}://generated/${encodeURIComponent(name)}?${query}`,
  );
}

/** Recover the source .ld URI from a virtual ST view URI. */
export function sourceOfStView(view: vscode.Uri): vscode.Uri | undefined {
  if (view.query.length === 0) {
    return undefined;
  }
  try {
    const source = decodeURIComponent(view.query);
    return source.startsWith('file:') ? vscode.Uri.parse(source) : undefined;
  } catch {
    // Malformed percent-encoding: no source, rendered as a comment.
    return undefined;
  }
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

/**
 * The command: open (or refresh) the generated ST for a .ld file. Accepts
 * the URI VS Code passes when invoked from the editor/title menu (the LD
 * custom editor is not a text editor, so activeTextEditor cannot resolve
 * it there).
 */
export async function showGeneratedSt(
  context: vscode.ExtensionContext,
  uri?: vscode.Uri,
): Promise<void> {
  let source: vscode.Uri | undefined =
    uri?.scheme === 'file' && uri.fsPath.endsWith('.ld')
      ? uri
      : undefined;

  if (!source) {
    const editor = vscode.window.activeTextEditor;
    source =
      editor?.document.languageId === 'ladder-diagram'
        ? editor.document.uri
        : editor?.document.uri.scheme === LD_ST_SCHEME
          ? sourceOfStView(editor.document.uri)
          : undefined;
  }
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
    // The CLI reads from disk — save dirty diagram state first so the view
    // reflects what the user sees (matches the ST run/build commands).
    const active = vscode.window.activeTextEditor;
    void active; // custom editors are not text editors; the save targets
    // the active editor of ANY kind, and no-ops when nothing is dirty.
    try {
      await vscode.commands.executeCommand('workbench.action.files.save');
    } catch {
      // Saving is best-effort (nothing dirty, or no savable editor).
    }
    await generateStText(context, source); // validate before opening
    const doc = await vscode.workspace.openTextDocument(stViewUri(source));
    await vscode.window.showTextDocument(doc, {
      preview: true,
      viewColumn: vscode.ViewColumn.Beside,
    });
  } catch (error) {
    void vscode.window.showErrorMessage(`LD → ST failed: ${(error as Error).message}`);
  }
}

/**
 * Content provider for `plc-ld-st:` virtual documents — always generates
 * fresh (no cache: a stale snapshot is worse than a cargo run), firing
 * `onDidChange` lets the command force an open view to reload.
 */
export class LdStContentProvider implements vscode.TextDocumentContentProvider {
  private readonly _onDidChange = new vscode.EventEmitter<vscode.Uri>();
  readonly onDidChange = this._onDidChange.event;

  constructor(private readonly context: vscode.ExtensionContext) {}

  /** Force any open view of `uri` to re-fetch its content. */
  refresh(uri: vscode.Uri): void {
    this._onDidChange.fire(uri);
  }

  async provideTextDocumentContent(uri: vscode.Uri): Promise<string> {
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
