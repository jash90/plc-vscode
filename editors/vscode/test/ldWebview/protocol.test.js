// PLC-110 — typed webview protocol: every message round-trips through
// parse/serialize; unknown types throw.
'use strict';
const assert = require('node:assert');
const {
  parseHostMessage,
  parseWebviewMessage,
} = require('../../dist/ldWebview/protocol.js');

let failures = 0;
function check(name, fn) {
  try {
    fn();
    console.log(`ok - ${name}`);
  } catch (error) {
    failures += 1;
    console.error(`FAIL - ${name}: ${error.message}`);
  }
}

check('host load message round-trips', () => {
  const message = { type: 'load', text: '{"name":"P"}' };
  assert.deepStrictEqual(parseHostMessage(JSON.parse(JSON.stringify(message))), message);
});

check('host powerFlow message round-trips', () => {
  const message = { type: 'powerFlow', json: '{"rungs":[]}' };
  assert.deepStrictEqual(parseHostMessage(message), message);
});

check('webview save message round-trips', () => {
  const message = { type: 'save', text: '{}' };
  assert.deepStrictEqual(parseWebviewMessage(message), message);
});

check('webview run message round-trips', () => {
  assert.deepStrictEqual(parseWebviewMessage({ type: 'run' }), { type: 'run' });
});

check('unknown host message type throws', () => {
  assert.throws(() => parseHostMessage({ type: 'definitely-not-a-thing' }), /unknown host message/);
});

check('non-object host message throws', () => {
  assert.throws(() => parseHostMessage('load'), /unknown host message/);
});

check('unknown webview message type throws', () => {
  assert.throws(() => parseWebviewMessage({ type: 'nope' }), /unknown webview message/);
});


check('webview edit message round-trips with its command', () => {
  const message = { type: 'edit', command: { type: 'addContact', label: 'Add contact A', rung: 0, branch: 0, index: -1, name: 'A', negated: false } };
  assert.deepStrictEqual(parseWebviewMessage(message), message);
});

check('webview undo and redo round-trip', () => {
  assert.deepStrictEqual(parseWebviewMessage({ type: 'undo' }), { type: 'undo' });
  assert.deepStrictEqual(parseWebviewMessage({ type: 'redo' }), { type: 'redo' });
});

if (failures > 0) {
  process.exit(1);
}
console.log('protocol ok');
