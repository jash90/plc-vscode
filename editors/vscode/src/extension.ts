import * as path from 'node:path';
import { spawn } from 'node:child_process';
import * as vscode from 'vscode';
import { LanguageClient, LanguageClientOptions, ServerOptions } from 'vscode-languageclient/node';
import { CLI_BINARY, SERVER_BINARY, bundledBinaryRelativePath } from './bundled';

let client: LanguageClient | undefined;
let outputChannel: vscode.OutputChannel | undefined;

function workspaceRoot(context: vscode.ExtensionContext): string {
  return path.resolve(context.extensionPath, '..', '..');
}

/** Installed (Marketplace) extensions run in Production mode. */
function isProduction(context: vscode.ExtensionContext): boolean {
  return context.extensionMode === vscode.ExtensionMode.Production;
}

interface RunInvocation {
  command: string;
  args: string[];
  cwd?: string;
}

/**
 * Build the command/args to invoke a `plc` subcommand (`run`, `debug`, …),
 * shared by the run command and the debug adapter so both resolve the dev
 * (cargo) vs production (bundled binary) launch the same way.
 */
function resolveRunInvocation(
  context: vscode.ExtensionContext,
  subcommand: string,
  extraArgs: string[],
): RunInvocation {
  if (isProduction(context)) {
    // Installed extension: run the bundled CLI binary directly.
    return {
      command: context.asAbsolutePath(bundledBinaryRelativePath(CLI_BINARY)),
      args: [subcommand, ...extraArgs],
    };
  }

  // Development: drive the workspace CLI via cargo. `cliArgs` ends with the
  // `run` subcommand by default; swap that trailing subcommand for the
  // requested one so `run` and `debug` share the same cargo prefix.
  const config = vscode.workspace.getConfiguration('plcVscode');
  const command = config.get<string>('cliCommand', 'cargo');
  const cliArgs = config.get<string[]>('cliArgs', [
    'run',
    '--quiet',
    '--package',
    'plc_cli',
    '--',
    'run',
  ]);
  const cargoPrefix = cliArgs.slice(0, -1);
  const repositoryRoot = config.get<string>('repositoryRoot', '') || workspaceRoot(context);

  return {
    command,
    args: [...cargoPrefix, subcommand, ...extraArgs],
    cwd: repositoryRoot,
  };
}

function serverOptions(context: vscode.ExtensionContext): ServerOptions {
  // Installed extension: run the bundled, platform-specific server binary so
  // end users need neither the repository nor a Rust toolchain.
  if (isProduction(context)) {
    return {
      command: context.asAbsolutePath(bundledBinaryRelativePath(SERVER_BINARY)),
      args: [],
    };
  }

  // Development: build/run the server from the Rust workspace via cargo.
  const config = vscode.workspace.getConfiguration('plcVscode');
  const command = config.get<string>('serverCommand', 'cargo');
  const args = config.get<string[]>('serverArgs', [
    'run',
    '--package',
    'plc_lsp_server',
    '--bin',
    'plc-lsp-server',
    '--',
  ]);

  const repositoryRoot = config.get<string>('repositoryRoot', '') || workspaceRoot(context);

  return {
    command,
    args,
    options: {
      cwd: repositoryRoot,
    },
  };
}

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  outputChannel = vscode.window.createOutputChannel('PLC VS Code');
  context.subscriptions.push(outputChannel);

  const status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
  status.text = 'PLC VS Code';
  status.tooltip = 'PLC VS Code language support is active';
  status.command = 'plc-vscode.showStatus';
  status.show();
  context.subscriptions.push(status);

  context.subscriptions.push(
    vscode.commands.registerCommand('plc-vscode.showStatus', async () => {
      const state = client ? 'running' : 'not started';
      await vscode.window.showInformationMessage(`PLC VS Code extension active; language server is ${state}.`);
    }),
  );



  context.subscriptions.push(
    vscode.commands.registerCommand(
      'plc-vscode.runCurrentFile',
      async (resource?: vscode.Uri) => {
        await runCurrentStructuredTextFile(context, resource);
      },
    ),
    vscode.commands.registerCommand(
      'plc-vscode.debugCurrentFile',
      async (resource?: vscode.Uri) => {
        await debugCurrentStructuredTextFile(resource);
      },
    ),
    vscode.commands.registerCommand(
      'plc-vscode.buildCpdev',
      async (resource?: vscode.Uri) => {
        await compileCurrentFileToCpdev(context, resource);
      },
    ),
  );

  // Stepping debugger: contribute the `plc-st` debug type. The provider fills a
  // default launch config for F5-without-launch.json; the factory spawns the
  // `plc debug` DAP adapter.
  const debugProvider = new PlcDebugConfigurationProvider();
  context.subscriptions.push(
    vscode.debug.registerDebugConfigurationProvider('plc-st', debugProvider),
    vscode.debug.registerDebugConfigurationProvider(
      'plc-st',
      debugProvider,
      vscode.DebugConfigurationProviderTriggerKind.Dynamic,
    ),
    vscode.debug.registerDebugAdapterDescriptorFactory(
      'plc-st',
      new PlcDebugAdapterFactory(context),
    ),
  );


  if (vscode.workspace.getConfiguration('plcVscode').get<boolean>('autoRunOnOpen', false)) {
    const activeEditor = vscode.window.activeTextEditor;
    if (activeEditor && activeEditor.document.languageId === 'structured-text') {
      setTimeout(() => {
        void runCurrentStructuredTextFile(context);
      }, 1000);
    }
  }

  context.subscriptions.push(
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      if (
        editor &&
        editor.document.languageId === 'structured-text' &&
        vscode.workspace.getConfiguration('plcVscode').get<boolean>('autoRunOnOpen', false)
      ) {
        void runCurrentStructuredTextFile(context);
      }
    }),
  );

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: 'file', language: 'structured-text' }],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher('**/*.{st,iecst,plcst}'),
    },
  };

  client = new LanguageClient(
    'plc-vscode-lsp',
    'PLC VS Code Language Server',
    serverOptions(context),
    clientOptions,
  );

  context.subscriptions.push(client);
  await client.start();
}

export async function deactivate(): Promise<void> {
  const runningClient = client;
  client = undefined;
  if (runningClient) {
    await runningClient.stop();
  }
}


async function runCurrentStructuredTextFile(
  context: vscode.ExtensionContext,
  resource?: vscode.Uri,
): Promise<void> {
  const target = await resolveStructuredTextTarget(resource);
  if (!target) {
    await vscode.window.showWarningMessage('Open a Structured Text file before running PLC VS Code.');
    return;
  }

  const invocation = resolveRunInvocation(context, 'run', [target]);
  const command = invocation.command;
  const args = invocation.args;
  const spawnOptions: { cwd?: string } = invocation.cwd ? { cwd: invocation.cwd } : {};

  outputChannel?.clear();
  outputChannel?.appendLine(`$ ${command} ${args.join(' ')}`);
  outputChannel?.show(true);

  await new Promise<void>((resolve) => {
    const child = spawn(command, args, spawnOptions);
    child.stdout.on('data', (chunk: Buffer) => outputChannel?.append(chunk.toString()));
    child.stderr.on('data', (chunk: Buffer) => outputChannel?.append(chunk.toString()));
    child.on('error', async (error: Error) => {
      outputChannel?.appendLine(`Failed to run Structured Text: ${error.message}`);
      await vscode.window.showErrorMessage(`PLC VS Code run failed: ${error.message}`);
      resolve();
    });
    child.on('close', async (code: number | null) => {
      if (code === 0) {
        await vscode.window.showInformationMessage('PLC VS Code run completed.');
      } else {
        await vscode.window.showErrorMessage(`PLC VS Code run failed with exit code ${code}.`);
      }
      resolve();
    });
  });
}


async function compileCurrentFileToCpdev(
  context: vscode.ExtensionContext,
  resource?: vscode.Uri,
): Promise<void> {
  const target = await resolveStructuredTextTarget(resource);
  if (!target) {
    await vscode.window.showWarningMessage(
      'Open a Structured Text file before compiling to CPDev .xcp.',
    );
    return;
  }

  // `plc build` defaults to `--target cpdev` and writes `<file>.xcp` plus its
  // `.dcp` sidecar next to the source. It is pure Rust, so it works on every
  // platform regardless of whether the VM (`cpdev` feature) is bundled.
  const invocation = resolveRunInvocation(context, 'build', [target]);
  const command = invocation.command;
  const args = invocation.args;
  const spawnOptions: { cwd?: string } = invocation.cwd ? { cwd: invocation.cwd } : {};

  outputChannel?.clear();
  outputChannel?.appendLine(`$ ${command} ${args.join(' ')}`);
  outputChannel?.show(true);

  await new Promise<void>((resolve) => {
    const child = spawn(command, args, spawnOptions);
    child.stdout.on('data', (chunk: Buffer) => outputChannel?.append(chunk.toString()));
    child.stderr.on('data', (chunk: Buffer) => outputChannel?.append(chunk.toString()));
    child.on('error', async (error: Error) => {
      outputChannel?.appendLine(`Failed to compile to CPDev .xcp: ${error.message}`);
      await vscode.window.showErrorMessage(`PLC VS Code compile failed: ${error.message}`);
      resolve();
    });
    child.on('close', async (code: number | null) => {
      if (code === 0) {
        await vscode.window.showInformationMessage('Compiled to CPDev .xcp (with .dcp sidecar).');
      } else {
        await vscode.window.showErrorMessage(`PLC VS Code compile failed with exit code ${code}.`);
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
class PlcDebugConfigurationProvider implements vscode.DebugConfigurationProvider {
  resolveDebugConfiguration(
    _folder: vscode.WorkspaceFolder | undefined,
    config: vscode.DebugConfiguration,
  ): vscode.ProviderResult<vscode.DebugConfiguration> {
    // Launched with no configuration (F5, no launch.json): synthesize one for
    // the active Structured Text editor.
    if (!config.type && !config.request && !config.name) {
      const editor = findStructuredTextEditor();
      if (!editor) {
        void vscode.window.showInformationMessage(
          'Open a Structured Text (.st) file to debug.',
        );
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

  provideDebugConfigurations(): vscode.ProviderResult<vscode.DebugConfiguration[]> {
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
class PlcDebugAdapterFactory implements vscode.DebugAdapterDescriptorFactory {
  constructor(private readonly context: vscode.ExtensionContext) {}

  createDebugAdapterDescriptor(): vscode.ProviderResult<vscode.DebugAdapterDescriptor> {
    const invocation = resolveRunInvocation(this.context, 'debug', []);
    const options: vscode.DebugAdapterExecutableOptions | undefined = invocation.cwd
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
async function resolveStructuredTextTarget(resource?: vscode.Uri): Promise<string | undefined> {
  // Invoked from a context menu: VS Code passes the resource Uri. The menu's
  // `when` clause already guarantees it is Structured Text; the file may not be
  // open at all (explorer right-click), so we do not read languageId here.
  if (resource && resource.scheme === 'file') {
    const openDoc = vscode.workspace.textDocuments.find(
      (doc) => doc.uri.fsPath === resource.fsPath,
    );
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
async function debugCurrentStructuredTextFile(resource?: vscode.Uri): Promise<void> {
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

function findStructuredTextEditor(): vscode.TextEditor | undefined {
  const activeEditor = vscode.window.activeTextEditor;
  if (isStructuredTextEditor(activeEditor)) {
    return activeEditor;
  }

  return vscode.window.visibleTextEditors.find(isStructuredTextEditor);
}

function isStructuredTextEditor(editor: vscode.TextEditor | undefined): editor is vscode.TextEditor {
  return Boolean(
    editor &&
      editor.document.uri.scheme === 'file' &&
      editor.document.languageId === 'structured-text',
  );
}
