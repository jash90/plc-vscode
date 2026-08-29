// PLC-112 — completion: program variables matching a prefix + the FB
// catalog with pin tables (sourced from plc_ld at the serve boundary later).
'use strict';
const assert = require('node:assert');
const path = require('node:path');
const fs = require('node:fs');
const { variables, fbCatalog, pinsFor } = require('../../dist/ldWebview/completion.js');
const { parseProgram } = require('../../dist/ldWebview/model.js');

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

const MOTOR_V1 = fs.readFileSync(
  path.resolve(__dirname, '..', '..', '..', '..', 'tests', 'ld', 'motor_control.ld'),
  'utf8',
);

check('returns program variables matching a prefix (case-insensitive)', () => {
  const program = parseProgram(MOTOR_V1);
  const hits = variables(program, 'mo');
  assert.ok(hits.includes('Motor'), `Motor in ${hits}`);
  assert.ok(hits.includes('MotorRun'), `MotorRun in ${hits}`);
  assert.ok(!hits.includes('Start'), `Start excluded from ${hits}`);
});

check('excludes the fully typed exact match', () => {
  const program = parseProgram(MOTOR_V1);
  assert.ok(!variables(program, 'Start').includes('Start'));
  assert.ok(variables(program, 'Star').includes('Start'));
});

check('fb catalog lists pins for TON', () => {
  assert.ok(fbCatalog().includes('TON'));
  const ton = pinsFor('TON');
  assert.deepStrictEqual(ton.inputs, ['IN', 'PT']);
  assert.deepStrictEqual(ton.outputs, ['Q', 'ET']);
});

check('fb catalog covers the standard set with pin tables', () => {
  for (const fbType of fbCatalog()) {
    const pins = pinsFor(fbType);
    assert.ok(pins.inputs.length > 0, `${fbType} needs input pins`);
    assert.ok(pins.outputs.length > 0, `${fbType} needs output pins`);
  }
  assert.strictEqual(pinsFor('MAGIC'), undefined);
});

if (failures > 0) {
  process.exit(1);
}
console.log('completion ok');
