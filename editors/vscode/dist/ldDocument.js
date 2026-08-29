"use strict";
/**
 * The LD document: owns the in-memory program and its command history, and
 * implements the full `vscode.CustomDocument` lifecycle — edits fire
 * `onDidChangeCustomDocument` with undo/redo closures (dirty dot, Cmd+Z,
 * hot-exit backups), revert re-reads the file, save-as writes elsewhere.
 *
 * One instance per URI, shared by every webview panel editing that file, so
 * split views join the same undo stack.
 */
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.LdDocument = void 0;
const vscode = __importStar(require("vscode"));
const commands_1 = require("./ldWebview/commands");
const model_1 = require("./ldWebview/model");
class LdDocument {
    uri;
    _onDidChange = new vscode.EventEmitter();
    onDidChange = this._onDidChange.event;
    _onDidRevert = new vscode.EventEmitter();
    onDidRevert = this._onDidRevert.event;
    history = new commands_1.CommandHistory();
    program;
    /** Serialized on disk as of the last save/revert. */
    savedText;
    constructor(uri, text) {
        this.uri = uri;
        this.savedText = text;
        this.program = this.parse(text);
    }
    static async open(uri) {
        const bytes = await vscode.workspace.fs.readFile(uri);
        return new LdDocument(uri, Buffer.from(bytes).toString('utf8'));
    }
    /** Parse and normalize (v1 upgrade + stable ids), tolerating bad JSON. */
    parse(text) {
        try {
            const program = (0, model_1.parseProgram)(text);
            (0, model_1.normalizeIds)(program);
            return program;
        }
        catch {
            return { name: 'NewProgram', schema_version: 2, rungs: [] };
        }
    }
    /** The current (normalized) program — the webview's source of truth. */
    get current() {
        return this.program;
    }
    get isDirty() {
        return this.serialize(this.program) !== this.savedText;
    }
    serialize(program) {
        (0, model_1.normalizeIds)(program);
        return (0, model_1.serializeProgram)(program);
    }
    /** Serialize for saving: normalized ids, deterministic formatting. */
    serializeCurrent() {
        // Clone so serialization-time normalization never mutates `current`.
        return this.serialize(JSON.parse(JSON.stringify(this.program)));
    }
    /** Apply a domain command as one undoable change. */
    applyEdit(command) {
        this.program = (0, commands_1.applyCommand)(this.program, command, this.history);
        this.fireDidChange(command.label);
        return this.program;
    }
    /** Replace the whole model (JSON textarea path) as one undoable change. */
    replaceProgram(program) {
        this.program = (0, commands_1.applyCommand)(this.program, commands_1.commands.replaceProgram(program), this.history);
        this.fireDidChange('Edit JSON');
        return this.program;
    }
    /** Undo the last change (webview button or VS Code timeline). */
    undo() {
        this.program = this.history.undo(this.program);
        this.fireDidChange('Undo');
        return this.program;
    }
    redo() {
        this.program = this.history.redo(this.program);
        this.fireDidChange('Redo');
        return this.program;
    }
    fireDidChange(label) {
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
    dispose() {
        this._onDidChange.dispose();
        this._onDidRevert.dispose();
    }
    /** Hot-exit backup: current content, not last-saved. */
    async backup(context) {
        await vscode.workspace.fs.writeFile(context.destination, Buffer.from(this.serializeCurrent(), 'utf8'));
        return {
            id: context.destination.fsPath,
            delete: () => {
                void vscode.workspace.fs.delete(context.destination).then(undefined, () => { });
            },
        };
    }
    async revert() {
        const bytes = await vscode.workspace.fs.readFile(this.uri);
        await this.revertToText(Buffer.from(bytes).toString('utf8'));
        this._onDidRevert.fire();
    }
    /** Reset the document to exactly the given (saved) text; clears history. */
    async revertToText(text) {
        this.savedText = text;
        this.program = this.parse(text);
        this.history.clear();
    }
    /** Persist the current program and remember it as the saved baseline. */
    async save() {
        this.savedText = this.serializeCurrent();
        await vscode.workspace.fs.writeFile(this.uri, Buffer.from(this.savedText, 'utf8'));
    }
    async saveAs(destination) {
        const text = this.serializeCurrent();
        await vscode.workspace.fs.writeFile(destination, Buffer.from(text, 'utf8'));
    }
}
exports.LdDocument = LdDocument;
//# sourceMappingURL=ldDocument.js.map