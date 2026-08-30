/**
 * Shared resolution of `plc` CLI invocations (dev cargo vs production
 * bundled binary). Extracted from extension.ts so the LD editor and the
 * run/debug commands resolve the binary identically — fixing the PLC-105
 * bug where the LD editor looked for `./dist/plc` while binaries ship
 * under `server/`.
 */

import * as path from 'node:path';
import * as vscode from 'vscode';
import { CLI_BINARY, bundledBinaryRelativePath } from './bundled';

export interface RunInvocation {
  command: string;
  args: string[];
  cwd?: string;
}

export function isProduction(context: vscode.ExtensionContext): boolean {
  return context.extensionMode === vscode.ExtensionMode.Production;
}

function workspaceRoot(context: vscode.ExtensionContext): string {
  const configured = vscode.workspace.getConfiguration('plcVscode').get<string>('repositoryRoot', '');
  return configured || path.resolve(context.extensionPath, '..', '..');
}

/**
 * Build the command/args to invoke a `plc` subcommand (`run`, `ld`, `debug`,
 * …). Production runs the bundled binary directly; development drives the
 * workspace CLI via cargo, swapping the trailing `run` subcommand from
 * `cliArgs` for the requested one.
 */
export function resolveRunInvocation(
  context: vscode.ExtensionContext,
  subcommand: string,
  extraArgs: string[],
): RunInvocation {
  if (isProduction(context)) {
    return {
      command: context.asAbsolutePath(bundledBinaryRelativePath(CLI_BINARY)),
      args: [subcommand, ...extraArgs],
    };
  }

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
  return {
    command,
    args: [...cargoPrefix, subcommand, ...extraArgs],
    cwd: workspaceRoot(context),
  };
}
