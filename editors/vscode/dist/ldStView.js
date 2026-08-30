"use strict";
/**
 * LD↔ST dual view (PLC-116): the report's dual-representation principle —
 * the same program editable as a diagram AND inspectable as Structured
 * Text. A read-only virtual document (`plc-ld-st:` scheme) backed by the
 * CLI's LD→ST conversion; re-running the command refreshes it.
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
exports.LdStContentProvider = exports.LD_ST_SCHEME = void 0;
exports.stViewUri = stViewUri;
exports.sourceOfStView = sourceOfStView;
exports.generateStText = generateStText;
exports.showGeneratedSt = showGeneratedSt;
const node_events_1 = require("node:events");
const vscode = __importStar(require("vscode"));
const ldCapture_1 = require("./ldCapture");
const ldCli_1 = require("./ldCli");
/** Virtual scheme for generated ST views of .ld files. */
exports.LD_ST_SCHEME = 'plc-ld-st';
/** Map a source .ld URI to its virtual ST view URI. */
function stViewUri(source) {
    const query = encodeURIComponent(source.toString());
    return vscode.Uri.parse(`${exports.LD_ST_SCHEME}://generated/${encodeURIComponent(source.fsPath.split('/').pop() ?? 'program.st')}?${query}`);
}
/** Recover the source .ld URI from a virtual ST view URI. */
function sourceOfStView(view) {
    const encoded = view.query;
    if (encoded.length === 0) {
        return undefined;
    }
    const source = decodeURIComponent(encoded);
    return source.startsWith('file:') ? vscode.Uri.parse(source) : undefined;
}
/** Generate the ST text for an .ld file via the CLI. */
async function generateStText(context, source) {
    const invocation = (0, ldCli_1.resolveRunInvocation)(context, 'convert', [
        'ld',
        'st',
        source.fsPath,
    ]);
    return (0, ldCapture_1.capture)(invocation);
}
/** The command: open (or refresh) the generated ST for a .ld file. */
async function showGeneratedSt(context) {
    const editor = vscode.window.activeTextEditor;
    let source = editor?.document.languageId === 'ladder-diagram'
        ? editor.document.uri
        : editor?.document.uri.scheme === exports.LD_ST_SCHEME
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
    }
    catch (error) {
        void vscode.window.showErrorMessage(`LD → ST failed: ${error.message}`);
    }
}
/**
 * Tiny in-memory content store for the virtual documents. The provider
 * consults it first (so the command can push fresh text) and falls back
 * to generating on demand.
 */
const store = new Map();
const changeEmitter = new node_events_1.EventEmitter();
function refreshStore(uri, text) {
    store.set(uri.toString(), text);
    changeEmitter.emit('change', uri.toString());
}
/** Content provider for `plc-ld-st:` virtual documents. */
class LdStContentProvider {
    context;
    _onDidChange = new vscode.EventEmitter();
    onDidChange = this._onDidChange.event;
    constructor(context) {
        this.context = context;
        changeEmitter.on('change', (key) => {
            this._onDidChange.fire(vscode.Uri.parse(key));
        });
    }
    async provideTextDocumentContent(uri) {
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
        }
        catch (error) {
            return `// LD → ST generation failed: ${error.message}`;
        }
    }
    dispose() {
        this._onDidChange.dispose();
    }
}
exports.LdStContentProvider = LdStContentProvider;
//# sourceMappingURL=ldStView.js.map