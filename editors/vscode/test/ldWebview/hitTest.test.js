// PLC-112 — drag-drop hit testing: pure geometry → insertion locations the
// command layer can consume.
'use strict';
const assert = require('node:assert');
const { layout, hitTest } = require('../../dist/ldWebview/layout.js');
const { parseProgram, normalizeIds } = require('../../dist/ldWebview/model.js');

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

const PROGRAM = `{
  "name": "Hit",
  "rungs": [
    {
      "branches": [
        { "elements": [
          { "id": "e0", "name": "A", "negated": false },
          { "id": "e1", "name": "B", "negated": false }
        ] }
      ],
      "outputs": [{ "kind": "coil", "id": "e2", "name": "Out", "variant": "normal" }]
    }
  ]
}`;

function geometryOf(text = PROGRAM) {
  const program = parseProgram(text);
  normalizeIds(program);
  return { program, geometry: layout(program) };
}

function center(element) {
  return { x: element.x + element.width / 2, y: element.y + element.height / 2 };
}

check('drop between contacts returns series insertion', () => {
  const { geometry } = geometryOf();
  const a = geometry.elements.find((e) => e.id === 'e0');
  const b = geometry.elements.find((e) => e.id === 'e1');
  const midpoint = { x: (a.x + a.width + b.x) / 2, y: a.y + a.height / 2 };
  const hit = hitTest(geometry, midpoint.x, midpoint.y);
  assert.strictEqual(hit.kind, 'series');
  assert.strictEqual(hit.rung, 0);
  assert.strictEqual(hit.branch, 0);
  assert.strictEqual(hit.index, 1, 'inserted before B');
});

check('drop below a rung returns parallel branch insertion', () => {
  const { geometry } = geometryOf();
  const a = geometry.elements.find((e) => e.id === 'e0');
  const below = { x: a.x + a.width / 2, y: a.y + a.height + 8 };
  const hit = hitTest(geometry, below.x, below.y);
  assert.strictEqual(hit.kind, 'parallel');
  assert.strictEqual(hit.rung, 0);
});

check('drop in the output column below the coil returns an output slot', () => {
  const { geometry } = geometryOf();
  const coil = geometry.elements.find((e) => e.id === 'e2');
  // Directly on the coil body is an element target (move/inspect)…
  const on = center(coil);
  assert.strictEqual(hitTest(geometry, on.x, on.y).kind, 'element');
  // …while the output column below it is a slot for a NEW output.
  const below = { x: on.x, y: coil.y + coil.height + 8 };
  const hit = hitTest(geometry, below.x, below.y);
  assert.strictEqual(hit.kind, 'output');
  assert.strictEqual(hit.rung, 0);
  assert.strictEqual(hit.index, 1, 'slot appended after the existing output');
});

check('drop on empty canvas below all rungs returns newRung', () => {
  const { geometry } = geometryOf();
  const hit = hitTest(geometry, geometry.width / 2, geometry.height + 60);
  assert.strictEqual(hit.kind, 'newRung');
});

check('drop on an existing element returns a move target', () => {
  const { geometry } = geometryOf();
  const a = geometry.elements.find((e) => e.id === 'e0');
  const point = center(a);
  const hit = hitTest(geometry, point.x, point.y - 6);
  // Directly on the contact body (not its center wire row) → move target.
  assert.strictEqual(hit.kind, 'element');
  assert.strictEqual(hit.id, 'e0');
});

check('hitTest never crashes on empty programs', () => {
  const { geometry } = geometryOf('{"name":"E","rungs":[]}');
  const hit = hitTest(geometry, 100, 100);
  assert.strictEqual(hit.kind, 'newRung');
});

if (failures > 0) {
  process.exit(1);
}
console.log('hitTest ok');
