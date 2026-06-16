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
/**
 * Build the command/args to invoke a `plc` subcommand (`run`, `debug`, …),
 * shared by the run command and the debug adapter so both resolve the dev
 * (cargo) vs production (bundled binary) launch the same way.
 */
function resolveRunInvocation(context, subcommand, extraArgs) {
    if (isProduction(context)) {
        // Installed extension: run the bundled CLI binary directly.
        return {
            command: context.asAbsolutePath((0, bundled_1.bundledBinaryRelativePath)(bundled_1.CLI_BINARY)),
            args: [subcommand, ...extraArgs],
        };
    }
    // Development: drive the workspace CLI via cargo. `cliArgs` ends with the
    // `run` subcommand by default; swap that trailing subcommand for the
    // requested one so `run` and `debug` share the same cargo prefix.
    const config = vscode.workspace.getConfiguration('plcVscode');
    const command = config.get('cliCommand', 'cargo');
    const cliArgs = config.get('cliArgs', [
        'run',
        '--quiet',
        '--package',
        'plc_cli',
        '--',
        'run',
    ]);
    const cargoPrefix = cliArgs.slice(0, -1);
    const repositoryRoot = config.get('repositoryRoot', '') || workspaceRoot(context);
    return {
        command,
        args: [...cargoPrefix, subcommand, ...extraArgs],
        cwd: repositoryRoot,
    };
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
    context.subscriptions.push(vscode.commands.registerCommand('plc-vscode.runCurrentFile', async (resource) => {
        await runCurrentStructuredTextFile(context, resource);
    }), vscode.commands.registerCommand('plc-vscode.debugCurrentFile', async (resource) => {
        await debugCurrentStructuredTextFile(resource);
    }));
    // Stepping debugger: contribute the `plc-st` debug type. The provider fills a
    // default launch config for F5-without-launch.json; the factory spawns the
    // `plc debug` DAP adapter.
    const debugProvider = new PlcDebugConfigurationProvider();
    context.subscriptions.push(vscode.debug.registerDebugConfigurationProvider('plc-st', debugProvider), vscode.debug.registerDebugConfigurationProvider('plc-st', debugProvider, vscode.DebugConfigurationProviderTriggerKind.Dynamic), vscode.debug.registerDebugAdapterDescriptorFactory('plc-st', new PlcDebugAdapterFactory(context)));
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
async function runCurrentStructuredTextFile(context, resource) {
    const target = await resolveStructuredTextTarget(resource);
    if (!target) {
        await vscode.window.showWarningMessage('Open a Structured Text file before running PLC VS Code.');
        return;
    }
    const invocation = resolveRunInvocation(context, 'run', [target]);
    const command = invocation.command;
    const args = invocation.args;
    const spawnOptions = invocation.cwd ? { cwd: invocation.cwd } : {};
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
/**
 * Supplies `plc-st` debug configurations: a default for F5 with no launch.json
 * (resolveDebugConfiguration) and an entry for the Run dropdown / launch.json
 * creation (provideDebugConfigurations).
 */
class PlcDebugConfigurationProvider {
    resolveDebugConfiguration(_folder, config) {
        // Launched with no configuration (F5, no launch.json): synthesize one for
        // the active Structured Text editor.
        if (!config.type && !config.request && !config.name) {
            const editor = findStructuredTextEditor();
            if (!editor) {
                void vscode.window.showInformationMessage('Open a Structured Text (.st) file to debug.');
                return undefined;
            }
            config.type = 'plc-st';
            config.request = 'launch';
            config.name = 'PLC: Debug Structured Text';
            config.program = '${file}';
            config.scans = 25;
        }
        if (!config.program) {
            config.program = '${file}';
        }
        return config;
    }
    provideDebugConfigurations() {
        return [
            {
                type: 'plc-st',
                request: 'launch',
                name: 'PLC: Debug Structured Text',
                program: '${file}',
                scans: 25,
            },
        ];
    }
}
/** Spawns the `plc debug` DAP adapter (dev: cargo; production: bundled binary). */
class PlcDebugAdapterFactory {
    context;
    constructor(context) {
        this.context = context;
    }
    createDebugAdapterDescriptor() {
        const invocation = resolveRunInvocation(this.context, 'debug', []);
        const options = invocation.cwd
            ? { cwd: invocation.cwd }
            : undefined;
        return new vscode.DebugAdapterExecutable(invocation.command, invocation.args, options);
    }
}
/**
 * Resolve the Structured Text file to act on. Context-menu commands
 * (editor/explorer/title) pass the resource Uri; Command Palette / keybinding /
 * autoRun pass nothing, in which case we fall back to the active editor. Saves
 * the file first if it is open with unsaved changes.
 */
async function resolveStructuredTextTarget(resource) {
    // Invoked from a context menu: VS Code passes the resource Uri. The menu's
    // `when` clause already guarantees it is Structured Text; the file may not be
    // open at all (explorer right-click), so we do not read languageId here.
    if (resource && resource.scheme === 'file') {
        const openDoc = vscode.workspace.textDocuments.find((doc) => doc.uri.fsPath === resource.fsPath);
        if (openDoc && openDoc.isDirty) {
            await openDoc.save();
        }
        return resource.fsPath;
    }
    // Invoked from the Command Palette / keybinding / autoRun: use the active editor.
    const editor = findStructuredTextEditor();
    if (!editor) {
        return undefined;
    }
    if (editor.document.isDirty) {
        await editor.document.save();
    }
    return editor.document.uri.fsPath;
}
/**
 * Start a `plc-st` debug session against the resolved target file. Passes an
 * explicit absolute `program` (not `${file}`) so the correct file is debugged
 * even when launched from the explorer on a non-active file.
 */
async function debugCurrentStructuredTextFile(resource) {
    const target = await resolveStructuredTextTarget(resource);
    if (!target) {
        await vscode.window.showWarningMessage('Open a Structured Text file before debugging PLC VS Code.');
        return;
    }
    const folder = vscode.workspace.getWorkspaceFolder(vscode.Uri.file(target));
    const started = await vscode.debug.startDebugging(folder, {
        type: 'plc-st',
        request: 'launch',
        name: 'PLC: Debug Structured Text',
        program: target,
        scans: 25,
    });
    if (!started) {
        await vscode.window.showErrorMessage(`PLC VS Code debug failed to start for ${target}.`);
    }
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