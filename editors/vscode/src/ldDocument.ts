/**
 * The LD document: owns the in-memory program and its command history, and
 * implements the full `vscode.CustomDocument` lifecycle — edits fire
 * `onDidChangeCustomDocument` with undo/redo closures (dirty dot, Cmd+Z,
 * hot-exit backups), revert re-reads the file, save-as writes elsewhere.
 *
 * Eventing follows the pawDraw contract with two emitters: `onDidChange`
 * fires ONLY for user edits (what VS Code tracks as undoable changes);
 * `onDidChangeContent` fires for every content mutation including
 * undo/redo, and is what syncs the webview panels. Undo/redo closures
 * therefore never push phantom entries onto VS Code's undo stack.
 *
 * One instance per URI, shared by every webview panel editing that file.
 */

import * as vscode from 'vscode';
import { CommandHistory, LdCommand, applyCommand, commands } from './ldWebview/commands';
import { LdProgram, normalizeIds, parseProgram, serializeProgram } from './ldWebview/model';

export class LdDocument implements vscode.CustomDocument {
  readonly uri: vscode.Uri;

  /** User edits only — becomes VS Code's undoable change stream. */
  private readonly _onDidChange =
    new vscode.EventEmitter<vscode.CustomDocumentEditEvent<LdDocument>>();
  readonly onDidChange = this._onDidChange.event;

  /** Every content change (edits + undo/redo) — syncs webview panels. */
  private readonly _onDidChangeContent = new vscode.EventEmitter<LdProgram>();
  readonly onDidChangeContent = this._onDidChangeContent.event;

  /** Fired from dispose() so the provider can drop its cache entry. */
  private readonly _onDisposed = new vscode.EventEmitter<LdDocument>();
  readonly onDisposed = this._onDisposed.event;

  private readonly history = new CommandHistory();
  private program: LdProgram;
  /** Serialized on disk as of the last save/revert. */
  private savedText: string;

  private constructor(uri: vscode.Uri, text: string) {
    this.uri = uri;
    this.savedText = text;
    this.program = this.parse(text);
  }

  /**
   * Open from disk. When `backupId` is present (hot-exit restore), the
   * backed-up content wins over the file on disk.
   */
  static async open(uri: vscode.Uri, backupId?: string): Promise<LdDocument> {
    if (backupId) {
      try {
        const backupBytes = await vscode.workspace.fs.readFile(vscode.Uri.file(backupId));
        return new LdDocument(uri, Buffer.from(backupBytes).toString('utf8'));
      } catch {
        // Fall through to the file on disk.
      }
    }
    const bytes = await vscode.workspace.fs.readFile(uri);
    return new LdDocument(uri, Buffer.from(bytes).toString('utf8'));
  }

  /** Parse and normalize (v1 upgrade + stable ids), tolerating bad JSON. */
  private parse(text: string): LdProgram {
    try {
      const program = parseProgram(text);
      normalizeIds(program);
      return program;
    } catch {
      return { name: 'NewProgram', schema_version: 2, rungs: [] };
    }
  }

  /** The current (normalized) program — the webview's source of truth. */
  get current(): LdProgram {
    return this.program;
  }

  private serialize(program: LdProgram): string {
    normalizeIds(program);
    return serializeProgram(program);
  }

  /** Serialize for saving: normalized ids, deterministic formatting. */
  serializeCurrent(): string {
    // Clone so serialization-time normalization never mutates `current`.
    return this.serialize(JSON.parse(JSON.stringify(this.program)) as LdProgram);
  }

  /** Apply a domain command as one undoable change. */
  applyEdit(command: LdCommand): LdProgram {
    this.program = applyCommand(this.program, command, this.history);
    this.fireEdit(command.label);
    return this.program;
  }

  /** Replace the whole model (JSON textarea path) as one undoable change. */
  replaceProgram(program: LdProgram): LdProgram {
    this.program = applyCommand(this.program, commands.replaceProgram(program), this.history);
    this.fireEdit('Edit JSON');
    return this.program;
  }

  /** Undo the last change (webview button or VS Code timeline). */
  undo(): void {
    const before = this.history.undoDepth;
    this.program = this.history.undo(this.program);
    if (this.history.undoDepth !== before) {
      this.fireContent();
    }
  }

  redo(): void {
    const before = this.history.redoDepth;
    this.program = this.history.redo(this.program);
    if (this.history.redoDepth !== before) {
      this.fireContent();
    }
  }

  /** A user edit: VS Code change event (with undo closures) + panel sync. */
  private fireEdit(label: string): void {
    this._onDidChange.fire({
      document: this,
      label,
      undo: () => this.undo(),
      redo: () => this.redo(),
    });
    this.fireContent();
  }

  private fireContent(): void {
    this._onDidChangeContent.fire(this.program);
  }

  /** vscode.CustomDocument */

  dispose(): void {
    this._onDisposed.fire(this);
    this._onDidChange.dispose();
    this._onDidChangeContent.dispose();
    this._onDisposed.dispose();
  }

  /** Hot-exit backup: current content, not last-saved. */
  async backup(
    context: vscode.CustomDocumentBackupContext,
  ): Promise<vscode.CustomDocumentBackup> {
    await vscode.workspace.fs.writeFile(
      context.destination,
      Buffer.from(this.serializeCurrent(), 'utf8'),
    );
    return {
      id: context.destination.fsPath,
      delete: () => {
        void vscode.workspace.fs.delete(context.destination).then(undefined, () => {});
      },
    };
  }

  async revert(): Promise<void> {
    const bytes = await vscode.workspace.fs.readFile(this.uri);
    this.savedText = Buffer.from(bytes).toString('utf8');
    this.program = this.parse(this.savedText);
    this.history.clear();
    this.fireContent();
  }

  /** Reset the document to exactly the given (saved) text; clears history. */
  async revertToText(text: string): Promise<void> {
    this.savedText = text;
    this.program = this.parse(text);
    this.history.clear();
    this.fireContent();
  }

  /** Persist the current program and remember it as the saved baseline. */
  async save(): Promise<void> {
    this.savedText = this.serializeCurrent();
    await vscode.workspace.fs.writeFile(this.uri, Buffer.from(this.savedText, 'utf8'));
  }

  async saveAs(destination: vscode.Uri): Promise<void> {
    const text = this.serializeCurrent();
    await vscode.workspace.fs.writeFile(destination, Buffer.from(text, 'utf8'));
  }
}
