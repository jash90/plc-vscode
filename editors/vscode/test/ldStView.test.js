// PLC-116 — LD↔ST dual view: virtual-document URI mapping and contract.
'use strict';
const assert = require('node:assert');

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

// The URI helpers import 'vscode' transitively through vscode.Uri — but the
// scheme mapping is string-pure, so we exercise it via a minimal Uri shim.
const vscodeShim = {
  Uri: {
    parse: (value) => ({ toString: () => value, query: value.split('?')[1] ?? '', fsPath: '' }),
    file: (path) => ({ toString: () => `file://${path}`, fsPath: path }),
  },
};
const Module = require('node:module');
const origResolve = Module._resolveFilename;
Module._resolveFilename = function (request, ...rest) {
  if (request === 'vscode') {
    return 'vscode';
  }
  return origResolve.call(this, request, ...rest);
};
require.cache.vscode = { id: 'vscode', filename: 'vscode', loaded: true, exports: vscodeShim };

const { LD_ST_SCHEME, stViewUri, sourceOfStView } = require('../dist/ldStView.js');

check('scheme constant', () => {
  assert.strictEqual(LD_ST_SCHEME, 'plc-ld-st');
});

check('st view uri round-trips to the source', () => {
  const source = vscodeShim.Uri.file('/tmp/motor.ld');
  const view = stViewUri(source);
  assert.ok(view.toString().startsWith('plc-ld-st://'), view.toString());
  assert.ok(view.toString().includes('motor.ld'), view.toString());
  const back = sourceOfStView(view);
  assert.strictEqual(back.toString(), source.toString());
});

check('view uri distinguishes two sources', () => {
  const a = stViewUri(vscodeShim.Uri.file('/tmp/a.ld'));
  const b = stViewUri(vscodeShim.Uri.file('/tmp/b.ld'));
  assert.notStrictEqual(a.toString(), b.toString());
});

check('view without query yields no source', () => {
  const bare = vscodeShim.Uri.parse('plc-ld-st://generated/program.st');
  assert.strictEqual(sourceOfStView(bare), undefined);
});

if (failures > 0) {
  process.exit(1);
}
console.log('ldStView ok');
