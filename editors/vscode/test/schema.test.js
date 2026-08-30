// PLC-118 — the .ld JSON Schema validates every known fixture and rejects
// a deliberately broken one. ajv (draft-07) against the golden corpus.
'use strict';
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

const Ajv = require('ajv');
const ajv = new Ajv({ allErrors: true });
const schema = JSON.parse(
  fs.readFileSync(path.resolve(__dirname, '../syntaxes/ladder-diagram.schema.json'), 'utf8'),
);
const validate = ajv.compile(schema);

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

const fixturesDir = path.resolve(__dirname, '..', '..', '..', 'tests', 'ld');
const fixtures = fs.readdirSync(fixturesDir).filter((name) => name.endsWith('.ld'));

check('fixture corpus exists', () => {
  assert.ok(fixtures.length >= 2, `expected fixtures, got ${fixtures}`);
});

for (const fixture of fixtures) {
  check(`schema validates ${fixture}`, () => {
    const doc = JSON.parse(fs.readFileSync(path.join(fixturesDir, fixture), 'utf8'));
    const valid = validate(doc);
    assert.ok(valid, `${fixture}: ${JSON.stringify(validate.errors)}`);
  });
}

check('schema rejects a broken program', () => {
  const broken = {
    name: 'Broken',
    rungs: [
      { branches: [{ elements: [{ name: 'A', negated: 'yes' }] }], outputs: [] },
    ],
  };
  const valid = validate(broken);
  assert.ok(!valid, 'negated must be boolean');
  // ajv v8 reports instancePath (draft-07 mode still uses instancePath).
  assert.ok(
    validate.errors.some((error) =>
      (error.instancePath ?? error.dataPath ?? '').includes('negated'),
    ),
    JSON.stringify(validate.errors),
  );
});

check('schema rejects an unknown fb_type', () => {
  const broken = {
    name: 'Broken',
    rungs: [
      {
        branches: [],
        outputs: [
          { kind: 'block', fb_type: 'MAGIC', instance: 'T', inputs: [], outputs: [] },
        ],
      },
    ],
  };
  assert.ok(!validate(broken), JSON.stringify(validate.errors));
});

check('schema rejects unknown top-level keys', () => {
  const broken = { name: 'X', rungs: [], surprise: true };
  assert.ok(!validate(broken));
});

if (failures > 0) {
  process.exit(1);
}
console.log('schema ok');
