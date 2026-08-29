// PLC-110 — TS model mirror: v1→v2 migration, id normalization parity with
// plc_ld::normalize_ids, variable collection, deterministic serialization.
'use strict';
const assert = require('node:assert');
const {
  parseProgram,
  normalizeIds,
  allVariables,
  serializeProgram,
  CURRENT_SCHEMA_VERSION,
} = require('../../dist/ldWebview/model.js');

const path = require('node:path');
const fs = require('node:fs');
const FIXTURE_DIR = path.resolve(__dirname, '..', '..', '..', '..', 'tests', 'ld');
const MOTOR_V1 = fs.readFileSync(path.join(FIXTURE_DIR, 'motor_control.ld'), 'utf8');
const MOTOR_V2 = fs.readFileSync(path.join(FIXTURE_DIR, 'motor_control_v2.ld'), 'utf8');

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

check('v1 fixture parses and upgrades to schema v2', () => {
  const program = parseProgram(MOTOR_V1);
  assert.strictEqual(program.schema_version, CURRENT_SCHEMA_VERSION);
  assert.strictEqual(program.rungs.length, 4);
});

check('v1 comments survive parsing', () => {
  const program = parseProgram(MOTOR_V1);
  assert.ok(program.rungs.every((r) => typeof r.comment === 'string' && r.comment.length > 0));
});

check('normalize assigns ids matching the Rust fixture', () => {
  // The v2 fixture is the canonical normalization of the v1 fixture
  // (pinned by plc_ld tests); the TS mirror must agree exactly.
  const migrated = parseProgram(MOTOR_V1);
  normalizeIds(migrated);
  const canonical = parseProgram(MOTOR_V2);
  assert.deepStrictEqual(migrated, canonical);
});

check('normalize is idempotent and preserves custom ids', () => {
  const program = parseProgram(MOTOR_V1);
  program.rungs[0].id = 'r-custom';
  program.rungs[0].branches[0].elements[0].id = 'e-keep';
  normalizeIds(program);
  assert.strictEqual(program.rungs[0].id, 'r-custom');
  assert.strictEqual(program.rungs[0].branches[0].elements[0].id, 'e-keep');
  const once = JSON.parse(JSON.stringify(program));
  normalizeIds(program);
  assert.deepStrictEqual(program, once);
});

check('duplicate id claims keep the first and regenerate the second', () => {
  const program = parseProgram(MOTOR_V1);
  program.rungs[0].branches[0].elements[0].id = 'e0';
  program.rungs[0].branches[1].elements[0].id = 'e0';
  normalizeIds(program);
  const first = program.rungs[0].branches[0].elements[0].id;
  const second = program.rungs[0].branches[1].elements[0].id;
  assert.strictEqual(first, 'e0');
  assert.notStrictEqual(first, second);
});

check('allVariables collects unique names across the program', () => {
  const program = parseProgram(MOTOR_V1);
  const vars = allVariables(program);
  for (const expected of ['Start', 'Motor', 'MotorRun', 'Done', 'Pulse', 'Reached']) {
    assert.ok(vars.includes(expected), `missing ${expected}`);
  }
  // FB instance names are not variables.
  assert.ok(!vars.includes('Timer'));
});

check('serialization is deterministic and 2-space pretty', () => {
  const program = parseProgram(MOTOR_V2);
  const first = serializeProgram(program);
  const second = serializeProgram(parseProgram(first));
  assert.strictEqual(first, second);
  assert.ok(first.includes('\n  "name"'));
});

check('id-less program serializes without id keys (v1 stability)', () => {
  const program = parseProgram(MOTOR_V1);
  for (const rung of program.rungs) {
    delete rung.id;
    delete rung.comment;
  }
  const json = serializeProgram(program);
  assert.ok(!json.includes('"id"'), json);
  assert.ok(!json.includes('"comment"'), json);
});

if (failures > 0) {
  process.exit(1);
}
console.log('model ok');
