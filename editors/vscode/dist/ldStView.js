"use strict";
/**
 * LD↔ST dual view (PLC-116): the report's dual-representation principle —
 * the same program editable as a diagram AND inspectable as Structured
 * Text. A read-only virtual document (`plc-ld-st:` scheme) backed by the
 * CLI's LD→ST conversion. The view always reflects the last SAVED state
 * of the .ld file (the command saves first).
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
const path = __importStar(require("node:path"));
const vscode = __importStar(require("vscode"));
const ldCapture_1 = require("./ldCapture");
const ldCli_1 = require("./ldCli");
/** Virtual scheme for generated ST views of .ld files. */
exports.LD_ST_SCHEME = 'plc-ld-st';
/** Map a source .ld URI to its virtual ST view URI. */
function stViewUri(source) {
    // The .st suffix gives the view the structured-text grammar (highlighting)
    // and a tab title distinct from the diagram's.
    const name = path.basename(source.fsPath).replace(/\.ld$/i, '.st');
    const query = encodeURIComponent(source.toString());
    return vscode.Uri.parse(`${exports.LD_ST_SCHEME}://generated/${encodeURIComponent(name)}?${query}`);
}
/** Recover the source .ld URI from a virtual ST view URI. */
function sourceOfStView(view) {
    if (view.query.length === 0) {
        return undefined;
    }
    try {
        const source = decodeURIComponent(view.query);
        return source.startsWith('file:') ? vscode.Uri.parse(source) : undefined;
    }
    catch {
        // Malformed percent-encoding: no source, rendered as a comment.
        return undefined;
    }
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
/**
 * The command: open (or refresh) the generated ST for a .ld file. Accepts
 * the URI VS Code passes when invoked from the editor/title menu (the LD
 * custom editor is not a text editor, so activeTextEditor cannot resolve
 * it there).
 */
async function showGeneratedSt(context, uri) {
    let source = uri?.scheme === 'file' && uri.fsPath.endsWith('.ld')
        ? uri
        : undefined;
    if (!source) {
        const editor = vscode.window.activeTextEditor;
        source =
            editor?.document.languageId === 'ladder-diagram'
                ? editor.document.uri
                : editor?.document.uri.scheme === exports.LD_ST_SCHEME
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
        await vscode.commands.executeCommand('workbench.action.files.save');
        await generateStText(context, source); // validate before opening
        const doc = await vscode.workspace.openTextDocument(stViewUri(source));
        await vscode.window.showTextDocument(doc, {
            preview: true,
            viewColumn: vscode.ViewColumn.Beside,
        });
    }
    catch (error) {
        void vscode.window.showErrorMessage(`LD → ST failed: ${error.message}`);
    }
}
/**
 * Content provider for `plc-ld-st:` virtual documents — always generates
 * fresh (no cache: a stale snapshot is worse than a cargo run), firing
 * `onDidChange` lets the command force an open view to reload.
 */
class LdStContentProvider {
    context;
    _onDidChange = new vscode.EventEmitter();
    onDidChange = this._onDidChange.event;
    constructor(context) {
        this.context = context;
    }
    /** Force any open view of `uri` to re-fetch its content. */
    refresh(uri) {
        this._onDidChange.fire(uri);
    }
    async provideTextDocumentContent(uri) {
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