/**
 * PLCopen XML export/import commands (PLC-115). Both drive the `plc` CLI's
 * special-cased `convert` (model-level interchange through plc_plcopen) —
 * the extension never reimplements the mapping. Destinations go through
 * save dialogs so nothing is silently overwritten.
 */

import * as vscode from 'vscode';
import { capture } from './ldCapture';
import { resolveRunInvocation } from './ldCli';

export async function exportPlcopen(context: vscode.ExtensionContext): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  const target =
    editor?.document.languageId === 'ladder-diagram'
      ? editor.document.uri
      : await pickFile('Choose the .ld file to export');
  if (!target) {
    return;
  }
  const destination = await vscode.window.showSaveDialog({
    defaultUri: vscode.Uri.file(target.fsPath.replace(/\.ld$/i, '.plcopen')),
    filters: { 'PLCopen XML': ['plcopen', 'xml'] },
  });
  if (!destination) {
    return;
  }

  const invocation = resolveRunInvocation(context, 'convert', [
    'ld',
    'plcopen',
    target.fsPath,
  ]);
  try {
    const xml = await capture(invocation);
    await vscode.workspace.fs.writeFile(destination, Buffer.from(xml, 'utf8'));
    void vscode.window.showInformationMessage(`Exported ${destination.fsPath}`);
  } catch (error) {
    void vscode.window.showErrorMessage(`PLCopen export failed: ${(error as Error).message}`);
  }
}

export async function importPlcopen(context: vscode.ExtensionContext): Promise<void> {
  const picked = await vscode.window.showOpenDialog({
    canSelectMany: false,
    filters: { 'PLCopen XML': ['plcopen', 'xml'] },
  });
  if (!picked || picked.length === 0) {
    return;
  }
  const source = picked[0];
  const destination = await vscode.window.showSaveDialog({
    defaultUri: vscode.Uri.file(source.fsPath.replace(/\.(plcopen|xml)$/i, '.ld')),
    filters: { 'Ladder Diagram': ['ld'] },
  });
  if (!destination) {
    return;
  }

  const invocation = resolveRunInvocation(context, 'convert', [
    'plcopen',
    'ld',
    source.fsPath,
  ]);
  try {
    const json = await capture(invocation);
    await vscode.workspace.fs.writeFile(destination, Buffer.from(json, 'utf8'));
    void vscode.window.showTextDocument(destination);
    void vscode.window.showInformationMessage(`Imported ${destination.fsPath}`);
  } catch (error) {
    void vscode.window.showErrorMessage(`PLCopen import failed: ${(error as Error).message}`);
  }
}

async function pickFile(label: string): Promise<vscode.Uri | undefined> {
  const picked = await vscode.window.showOpenDialog({
    canSelectMany: false,
    openLabel: label,
    filters: { 'Ladder Diagram': ['ld'] },
  });
  return picked?.[0];
}
