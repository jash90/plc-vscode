"use strict";
/**
 * Typed message protocol between the LD extension host and the webview.
 *
 * Pure (no DOM, no vscode import) so it is unit-testable in Node. The host
 * serializes with JSON.stringify; `parseHostMessage`/`parseWebviewMessage`
 * validate the discriminant on the receiving side and throw loudly on
 * unknown types instead of silently dropping messages.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.parseHostMessage = parseHostMessage;
exports.parseWebviewMessage = parseWebviewMessage;
function asRecord(value) {
    if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
        return value;
    }
    return null;
}
/** Parse a message arriving in the webview; throws on unknown types. */
function parseHostMessage(value) {
    const record = asRecord(value);
    switch (record?.type) {
        case 'load':
            return { type: 'load', text: String(record.text) };
        case 'state':
            return { type: 'state', program: record.program };
        case 'powerFlow':
            return { type: 'powerFlow', json: String(record.json) };
        case 'error':
            return { type: 'error', message: String(record.message) };
        default:
            throw new Error(`unknown host message: ${JSON.stringify(value)}`);
    }
}
/** Parse a message arriving in the extension host; throws on unknown types. */
function parseWebviewMessage(value) {
    const record = asRecord(value);
    switch (record?.type) {
        case 'ready':
            return { type: 'ready' };
        case 'save':
            return { type: 'save', text: String(record.text) };
        case 'run':
            return { type: 'run' };
        case 'modelChanged':
            return { type: 'modelChanged', program: record.program };
        case 'edit':
            return { type: 'edit', command: record.command };
        case 'undo':
            return { type: 'undo' };
        case 'redo':
            return { type: 'redo' };
        default:
            throw new Error(`unknown webview message: ${JSON.stringify(value)}`);
    }
}
//# sourceMappingURL=protocol.js.map