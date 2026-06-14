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
const node_child_process_1 = require("node:child_process");
const vscode = __importStar(require("vscode"));
const node_1 = require("vscode-languageclient/node");
const bundled_1 = require("./bundled");
let client;
let outputChannel;
function workspaceRoot(context) {
    return path.resolve(context.extensionPath, '..', '..');
}
/** Installed (Marketplace) extensions run in Production mode. */
function isProduction(context) {
    return context.extensionMode === vscode.ExtensionMode.Production;
}
function serverOptions(context) {
    // Installed extension: run the bundled, platform-specific server binary so
    // end users need neither the repository nor a Rust toolchain.
    if (isProduction(context)) {
        return {
            command: context.asAbsolutePath((0, bundled_1.bundledBinaryRelativePath)(bundled_1.SERVER_BINARY)),
            args: [],
        };
    }
    // Development: build/run the server from the Rust workspace via cargo.
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
    const repositoryRoot = config.get('repositoryRoot', '') || workspaceRoot(context);
    return {
        command,
        args,
        options: {
            cwd: repositoryRoot,
        },
    };
}
async function activate(context) {
    outputChannel = vscode.window.createOutputChannel('PLC VS Code');
    context.subscriptions.push(outputChannel);
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
    context.subscriptions.push(vscode.commands.registerCommand('plc-vscode.runCurrentFile', async () => {
        await runCurrentStructuredTextFile(context);
    }));
    if (vscode.workspace.getConfiguration('plcVscode').get('autoRunOnOpen', false)) {
        const activeEditor = vscode.window.activeTextEditor;
        if (activeEditor && activeEditor.document.languageId === 'structured-text') {
            setTimeout(() => {
                void runCurrentStructuredTextFile(context);
            }, 1000);
        }
    }
    context.subscriptions.push(vscode.window.onDidChangeActiveTextEditor((editor) => {
        if (editor &&
            editor.document.languageId === 'structured-text' &&
            vscode.workspace.getConfiguration('plcVscode').get('autoRunOnOpen', false)) {
            void runCurrentStructuredTextFile(context);
        }
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
async function runCurrentStructuredTextFile(context) {
    const editor = findStructuredTextEditor();
    if (!editor) {
        await vscode.window.showWarningMessage('Open a Structured Text file before running PLC VS Code.');
        return;
    }
    if (editor.document.isDirty) {
        await editor.document.save();
    }
    const config = vscode.workspace.getConfiguration('plcVscode');
    let command;
    let args;
    let spawnOptions;
    if (isProduction(context)) {
        // Installed extension: execute via the bundled CLI binary.
        command = context.asAbsolutePath((0, bundled_1.bundledBinaryRelativePath)(bundled_1.CLI_BINARY));
        args = ['run', editor.document.uri.fsPath];
        spawnOptions = {};
    }
    else {
        const repositoryRoot = config.get('repositoryRoot', '') || workspaceRoot(context);
        command = config.get('cliCommand', 'cargo');
        args = [
            ...config.get('cliArgs', ['run', '--quiet', '--package', 'plc_cli', '--', 'run']),
            editor.document.uri.fsPath,
        ];
        spawnOptions = { cwd: repositoryRoot };
    }
    outputChannel?.clear();
    outputChannel?.appendLine(`$ ${command} ${args.join(' ')}`);
    outputChannel?.show(true);
    await new Promise((resolve) => {
        const child = (0, node_child_process_1.spawn)(command, args, spawnOptions);
        child.stdout.on('data', (chunk) => outputChannel?.append(chunk.toString()));
        child.stderr.on('data', (chunk) => outputChannel?.append(chunk.toString()));
        child.on('error', async (error) => {
            outputChannel?.appendLine(`Failed to run Structured Text: ${error.message}`);
            await vscode.window.showErrorMessage(`PLC VS Code run failed: ${error.message}`);
            resolve();
        });
        child.on('close', async (code) => {
            if (code === 0) {
                await vscode.window.showInformationMessage('PLC VS Code run completed.');
            }
            else {
                await vscode.window.showErrorMessage(`PLC VS Code run failed with exit code ${code}.`);
            }
            resolve();
        });
    });
}
function findStructuredTextEditor() {
    const activeEditor = vscode.window.activeTextEditor;
    if (isStructuredTextEditor(activeEditor)) {
        return activeEditor;
    }
    return vscode.window.visibleTextEditors.find(isStructuredTextEditor);
}
function isStructuredTextEditor(editor) {
    return Boolean(editor &&
        editor.document.uri.scheme === 'file' &&
        editor.document.languageId === 'structured-text');
}
//# sourceMappingURL=extension.js.map