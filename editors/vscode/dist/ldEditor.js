"use strict";
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
exports.LdEditorProvider = void 0;
const vscode = __importStar(require("vscode"));
const node_child_process_1 = require("node:child_process");
const protocol_1 = require("./ldWebview/protocol");
const ldDocument_1 = require("./ldDocument");
const ldCli_1 = require("./ldCli");
/** Open documents by URI so split views share one undo stack. */
const documents = new Map();
class LdEditorProvider {
    context;
    _onDidChange = new vscode.EventEmitter();
    onDidChangeCustomDocument = this._onDidChange.event;
    constructor(context) {
        this.context = context;
    }
    async openCustomDocument(uri, _openContext, _token) {
        const key = uri.toString();
        const existing = documents.get(key);
        if (existing) {
            return existing;
        }
        const document = await ldDocument_1.LdDocument.open(uri);
        documents.set(key, document);
        document.onDidChange((event) => this._onDidChange.fire(event));
        return document;
    }
    async saveCustomDocument(document, _cancellation) {
        await document.save();
    }
    async revertCustomDocument(document, _cancellation) {
        await document.revert();
        for (const panel of this.panelsOf(document)) {
            this.postState(panel, document);
        }
    }
    async backupCustomDocument(document, context, _cancellation) {
        return document.backup(context);
    }
    async saveCustomDocumentAs(document, destination, _cancellation) {
        await document.saveAs(destination);
        documents.delete(document.uri.toString());
        documents.set(destination.toString(), document);
    }
    /** Webview panels resolved for a document (for state pushes). */
    panels = new Map();
    panelsOf(document) {
        return this.panels.get(document.uri.toString()) ?? new Set();
    }
    postState(panel, document) {
        void panel.webview.postMessage({ type: 'state', program: document.current });
    }
    async resolveCustomEditor(document, webviewPanel, _token) {
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
        const post = (message) => {
            void webviewPanel.webview.postMessage(message);
        };
        webviewPanel.webview.onDidReceiveMessage(async (data) => {
            let message;
            try {
                message = (0, protocol_1.parseWebviewMessage)(data);
            }
            catch (error) {
                post({ type: 'error', message: `protocol: ${error.message}` });
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
                    }
                    else {
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
    async updatePowerFlow(post, uri) {
        try {
            const invocation = (0, ldCli_1.resolveRunInvocation)(this.context, 'ld', [uri.fsPath, '--watch']);
            const result = await capture(invocation);
            post({ type: 'powerFlow', json: result });
        }
        catch (error) {
            vscode.window.showWarningMessage(`LD power-flow evaluation failed: ${error.message}`);
        }
    }
    /** Run the LD file via the CLI, streaming output to the LD channel. */
    async runLdFile(uri) {
        try {
            const invocation = (0, ldCli_1.resolveRunInvocation)(this.context, 'ld', [uri.fsPath]);
            const output = vscode.window.createOutputChannel('PLC LD');
            output.show(true);
            output.appendLine(`$ ${invocation.command} ${invocation.args.join(' ')}`);
            const child = (0, node_child_process_1.spawn)(invocation.command, invocation.args, invocation.cwd ? { cwd: invocation.cwd } : undefined);
            child.stdout.on('data', (chunk) => output.append(chunk.toString()));
            child.stderr.on('data', (chunk) => output.append(chunk.toString()));
            child.on('close', (code) => {
                output.appendLine(code === 0 ? 'LD execution completed.' : `LD execution failed with exit code ${code}.`);
            });
        }
        catch (error) {
            vscode.window.showErrorMessage(`LD run failed: ${error.message}`);
        }
    }
    /** The webview shell: CSP-hardened, external bundle + stylesheet. */
    getHtml(webview) {
        const nonce = getNonce();
        const scriptUri = webview.asWebviewUri(vscode.Uri.joinPath(this.context.extensionUri, 'media', 'ldEditor', 'main.js'));
        const styleUri = webview.asWebviewUri(vscode.Uri.joinPath(this.context.extensionUri, 'media', 'ldEditor', 'main.css'));
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
exports.LdEditorProvider = LdEditorProvider;
function getNonce() {
    const possible = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
    let text = '';
    for (let i = 0; i < 32; i += 1) {
        text += possible.charAt(Math.floor(Math.random() * possible.length));
    }
    return text;
}
/** Run an invocation and resolve with stdout (reject on non-zero exit). */
function capture(invocation) {
    return new Promise((resolve, reject) => {
        const child = (0, node_child_process_1.spawn)(invocation.command, invocation.args, invocation.cwd ? { cwd: invocation.cwd } : undefined);
        let stdout = '';
        let stderr = '';
        child.stdout.on('data', (chunk) => {
            stdout += chunk.toString();
        });
        child.stderr.on('data', (chunk) => {
            stderr += chunk.toString();
        });
        child.on('close', (code) => {
            if (code === 0) {
                resolve(stdout);
            }
            else {
                reject(new Error(stderr || `Exit code ${code}`));
            }
        });
        child.on('error', reject);
    });
}
//# sourceMappingURL=ldEditor.js.map