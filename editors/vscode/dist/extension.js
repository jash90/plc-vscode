"use strict";
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
exports.activate = activate;
exports.deactivate = deactivate;
const path = __importStar(require("node:path"));
const vscode = __importStar(require("vscode"));
const node_1 = require("vscode-languageclient/node");
let client;
function workspaceRoot(context) {
    return path.resolve(context.extensionPath, '..', '..');
}
function serverOptions(context) {
    const config = vscode.workspace.getConfiguration('plcVscode');
    const command = config.get('serverCommand', 'cargo');
    const args = config.get('serverArgs', [
        'run',
        '--package',
        'plc_lsp_server',
        '--bin',
        'plc-lsp-server',
        '--',
    ]);
    return {
        command,
        args,
        options: {
            cwd: workspaceRoot(context),
        },
    };
}
async function activate(context) {
    const status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
    status.text = 'PLC VS Code';
    status.tooltip = 'PLC VS Code language support is active';
    status.command = 'plc-vscode.showStatus';
    status.show();
    context.subscriptions.push(status);
    context.subscriptions.push(vscode.commands.registerCommand('plc-vscode.showStatus', async () => {
        const state = client ? 'running' : 'not started';
        await vscode.window.showInformationMessage(`PLC VS Code extension active; language server is ${state}.`);
    }));
    const clientOptions = {
        documentSelector: [{ scheme: 'file', language: 'structured-text' }],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.{st,iecst,plcst}'),
        },
    };
    client = new node_1.LanguageClient('plc-vscode-lsp', 'PLC VS Code Language Server', serverOptions(context), clientOptions);
    context.subscriptions.push(client);
    await client.start();
}
async function deactivate() {
    const runningClient = client;
    client = undefined;
    if (runningClient) {
        await runningClient.stop();
    }
}
//# sourceMappingURL=extension.js.map