// PLC-110 — pure layout: series elements left-to-right, parallel branches
// stacked vertically without overlap, OR collector/tee joins, rails present.
'use strict';
const assert = require('node:assert');
const { layout } = require('../../dist/ldWebview/layout.js');
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

function overlaps(a, b) {
  return (
    a.x < b.x + b.width &&
    b.x < a.x + a.width &&
    a.y < b.y + b.height &&
    b.y < a.y + a.height
  );
}

const PARALLEL_LD = `{
  "name": "Parallel",
  "rungs": [
    {
      "branches": [
        { "elements": [{ "name": "A", "negated": false }, { "name": "B", "negated": false }] },
        { "elements": [{ "name": "C", "negated": false }] }
      ],
      "outputs": [{ "kind": "coil", "name": "Out", "variant": "normal" }]
    }
  ]
}`;

check('parallel branches stack vertically without overlap', () => {
  const geometry = layout(parseProgram(PARALLEL_LD));
  const contacts = geometry.elements.filter((e) => e.kind === 'contact');
  const branch0 = contacts.filter((e) => e.branch === 0);
  const branch1 = contacts.filter((e) => e.branch === 1);
  assert.ok(branch0.length > 0 && branch1.length > 0, 'both branches present');
  for (const a of branch0) {
    for (const b of branch1) {
      assert.ok(!overlaps(a, b), `branch elements must not overlap: ${JSON.stringify([a, b])}`);
    }
  }
});

check('series elements lay out left to right', () => {
  const geometry = layout(parseProgram(PARALLEL_LD));
  const branch0 = geometry.elements
    .filter((e) => e.kind === 'contact' && e.branch === 0)
    .sort((a, b) => a.index - b.index);
  for (let i = 1; i < branch0.length; i += 1) {
    assert.ok(
      branch0[i].x > branch0[i - 1].x + branch0[i - 1].width - 1,
      `element ${i} must start right of element ${i - 1}`,
    );
  }
});

check('OR joins connect branch ends to the output', () => {
  const geometry = layout(parseProgram(PARALLEL_LD));
  // A vertical collector on the left of the branch stack…
  const collectors = geometry.wires.filter((w) => w.kind === 'collector');
  assert.ok(collectors.length > 0, 'left collector present');
  // …and a vertical tee on the right feeding the output column.
  const tees = geometry.wires.filter((w) => w.kind === 'tee');
  assert.ok(tees.length > 0, 'right tee present');
});

check('rails present and outside the element band', () => {
  const geometry = layout(parseProgram(PARALLEL_LD));
  assert.strictEqual(geometry.leftRailX >= 0, true);
  assert.ok(geometry.rightRailX > geometry.leftRailX, 'right rail right of left rail');
  for (const element of geometry.elements) {
    assert.ok(element.x > geometry.leftRailX, 'elements start right of the left rail');
    assert.ok(
      element.x + element.width <= geometry.rightRailX,
      'elements end left of the right rail',
    );
  }
});

check('rungs stack vertically with breathing room', () => {
  const program = parseProgram(PARALLEL_LD);
  program.rungs.push(JSON.parse(JSON.stringify(program.rungs[0])));
  const geometry = layout(program);
  const rung0 = geometry.elements.filter((e) => e.rung === 0);
  const rung1 = geometry.elements.filter((e) => e.rung === 1);
  const max0 = Math.max(...rung0.map((e) => e.y + e.height));
  const min1 = Math.min(...rung1.map((e) => e.y));
  assert.ok(min1 >= max0, 'rung 1 starts below the end of rung 0');
});

check('layout grows with content (deterministic size)', () => {
  const one = layout(parseProgram(PARALLEL_LD));
  const program = parseProgram(PARALLEL_LD);
  program.rungs[0].branches[0].elements.push({ name: 'D', negated: false });
  const two = layout(program);
  assert.ok(two.width > one.width, 'adding a contact widens the layout');
});

if (failures > 0) {
  process.exit(1);
}
console.log('layout ok');
