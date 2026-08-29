/**
 * Typed message protocol between the LD extension host and the webview.
 *
 * Pure (no DOM, no vscode import) so it is unit-testable in Node. The host
 * serializes with JSON.stringify; `parseHostMessage`/`parseWebviewMessage`
 * validate the discriminant on the receiving side and throw loudly on
 * unknown types instead of silently dropping messages.
 */

import { LdCommand } from './commands';
import { LdProgram } from './model';

/** Messages the extension host sends to the webview. */
export type HostToWebview =
  | { type: 'load'; text: string }
  | { type: 'state'; program: LdProgram }
  | { type: 'powerFlow'; json: string }
  | { type: 'error'; message: string };

/** Messages the webview sends to the extension host. */
export type WebviewToHost =
  | { type: 'ready' }
  | { type: 'save'; text: string }
  | { type: 'run' }
  | { type: 'modelChanged'; program: LdProgram }
  | { type: 'edit'; command: LdCommand }
  | { type: 'undo' }
  | { type: 'redo' };

function asRecord(value: unknown): Record<string, unknown> | null {
  if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
    return value as Record<string, unknown>;
  }
  return null;
}

/** Parse a message arriving in the webview; throws on unknown types. */
export function parseHostMessage(value: unknown): HostToWebview {
  const record = asRecord(value);
  switch (record?.type) {
    case 'load':
      return { type: 'load', text: String(record.text) };
    case 'state':
      return { type: 'state', program: record.program as LdProgram };
    case 'powerFlow':
      return { type: 'powerFlow', json: String(record.json) };
    case 'error':
      return { type: 'error', message: String(record.message) };
    default:
      throw new Error(`unknown host message: ${JSON.stringify(value)}`);
  }
}

/** Parse a message arriving in the extension host; throws on unknown types. */
export function parseWebviewMessage(value: unknown): WebviewToHost {
  const record = asRecord(value);
  switch (record?.type) {
    case 'ready':
      return { type: 'ready' };
    case 'save':
      return { type: 'save', text: String(record.text) };
    case 'run':
      return { type: 'run' };
    case 'modelChanged':
      return { type: 'modelChanged', program: record.program as LdProgram };
    case 'edit':
      return { type: 'edit', command: record.command as LdCommand };
    case 'undo':
      return { type: 'undo' };
    case 'redo':
      return { type: 'redo' };
    default:
      throw new Error(`unknown webview message: ${JSON.stringify(value)}`);
  }
}
