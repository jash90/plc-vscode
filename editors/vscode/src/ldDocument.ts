/**
 * The LD document: owns the in-memory program and its command history, and
 * implements the full `vscode.CustomDocument` lifecycle — edits fire
 * `onDidChangeCustomDocument` with undo/redo closures (dirty dot, Cmd+Z,
 * hot-exit backups), revert re-reads the file, save-as writes elsewhere.
 *
 * One instance per URI, shared by every webview panel editing that file, so
 * split views join the same undo stack.
 */

import * as vscode from 'vscode';
import { CommandHistory, LdCommand, applyCommand, commands } from './ldWebview/commands';
import { LdProgram, normalizeIds, parseProgram, serializeProgram } from './ldWebview/model';

export class LdDocument implements vscode.CustomDocument {
  readonly uri: vscode.Uri;

  private readonly _onDidChange = new vscode.EventEmitter<vscode.CustomDocumentEditEvent<LdDocument>>();
  readonly onDidChange: vscode.Event<vscode.CustomDocumentEditEvent<LdDocument>> =
    this._onDidChange.event;

  private readonly _onDidRevert = new vscode.EventEmitter<void>();
  readonly onDidRevert: vscode.Event<void> = this._onDidRevert.event;

  private readonly history = new CommandHistory();
  private program: LdProgram;
  /** Serialized on disk as of the last save/revert. */
  private savedText: string;

  private constructor(uri: vscode.Uri, text: string) {
    this.uri = uri;
    this.savedText = text;
    this.program = this.parse(text);
  }

  static async open(uri: vscode.Uri): Promise<LdDocument> {
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

  get isDirty(): boolean {
    return this.serialize(this.program) !== this.savedText;
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
    this.fireDidChange(command.label);
    return this.program;
  }

  /** Replace the whole model (JSON textarea path) as one undoable change. */
  replaceProgram(program: LdProgram): LdProgram {
    this.program = applyCommand(this.program, commands.replaceProgram(program), this.history);
    this.fireDidChange('Edit JSON');
    return this.program;
  }

  /** Undo the last change (webview button or VS Code timeline). */
  undo(): LdProgram {
    this.program = this.history.undo(this.program);
    this.fireDidChange('Undo');
    return this.program;
  }

  redo(): LdProgram {
    this.program = this.history.redo(this.program);
    this.fireDidChange('Redo');
    return this.program;
  }

  private fireDidChange(label: string): void {
    this._onDidChange.fire({
      document: this,
      label,
      undo: () => {
        this.undo();
      },
      redo: () => {
        this.redo();
      },
    });
  }


  /** vscode.CustomDocument */

  dispose(): void {
    this._onDidChange.dispose();
    this._onDidRevert.dispose();
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
    await this.revertToText(Buffer.from(bytes).toString('utf8'));
    this._onDidRevert.fire();
  }

  /** Reset the document to exactly the given (saved) text; clears history. */
  async revertToText(text: string): Promise<void> {
    this.savedText = text;
    this.program = this.parse(text);
    this.history.clear();
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
