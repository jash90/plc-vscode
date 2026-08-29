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
  | { type: 'error'; message: string }
  | { type: 'simState'; scan: number; timeMs: number; watch: string[]; forced: string[] };

/** Messages the webview sends to the extension host. */
export type WebviewToHost =
  | { type: 'ready' }
  /** Triggers VS Code's save flow (dirty clears, undo history kept). */
  | { type: 'save' }
  | { type: 'run' }
  | { type: 'modelChanged'; program: LdProgram }
  | { type: 'edit'; command: LdCommand }
  | { type: 'undo' }
  | { type: 'redo' }
  | { type: 'simStart' }
  | { type: 'simStop' }
  | { type: 'simStep' }
  | { type: 'simReset' }
  | { type: 'simInput'; name: string; value: boolean };

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
    case 'simState':
      return {
        type: 'simState',
        scan: Number(record.scan),
        timeMs: Number(record.timeMs),
        watch: Array.isArray(record.watch) ? (record.watch as string[]) : [],
        forced: Array.isArray(record.forced) ? (record.forced as string[]) : [],
      };
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
      return { type: 'save' };
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
    case 'simStart':
      return { type: 'simStart' };
    case 'simStop':
      return { type: 'simStop' };
    case 'simStep':
      return { type: 'simStep' };
    case 'simReset':
      return { type: 'simReset' };
    case 'simInput':
      return { type: 'simInput', name: String(record.name), value: record.value === true };
    default:
      throw new Error(`unknown webview message: ${JSON.stringify(value)}`);
  }
}
