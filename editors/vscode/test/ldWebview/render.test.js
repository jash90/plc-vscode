// PLC-110 — SVG rendering: one group per element keyed by id, energized
// classes exactly where power-flow says so.
'use strict';
const assert = require('node:assert');
const { layout } = require('../../dist/ldWebview/layout.js');
const { parseProgram, normalizeIds } = require('../../dist/ldWebview/model.js');
const { renderSvg } = require('../../dist/ldWebview/render.js');

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

const SIMPLE_LD = `{
  "name": "Simple",
  "rungs": [
    {
      "branches": [
        { "elements": [
          { "name": "Start", "negated": false },
          { "name": "Stop", "negated": true }
        ] }
      ],
      "outputs": [{ "kind": "coil", "name": "Motor", "variant": "normal" }]
    }
  ]
}`;

// Start=true, Stop=false → contact 0 energized, NC Stop passes, coil on.
const FLOW = {
  rungs: [
    {
      contact_energized: [[true, true]],
      branch_energized: [true],
      output_energized: [true],
      rung_result: true,
    },
  ],
};

/** Extract the opening <g> tag for an element id. */
function elementTag(svg, id) {
  const match = svg.match(new RegExp(`<g [^>]*data-id="${id}"[^>]*>`));
  return match ? match[0] : '';
}

/** True when a tag/class list carries the `energized` class exactly. */
function hasEnergizedClass(tagOrSvg) {
  return / energized"/.test(tagOrSvg) || / energized /.test(tagOrSvg);
}

check('one element group per contact and output, keyed by id', () => {
  const program = parseProgram(SIMPLE_LD);
  normalizeIds(program);
  const svg = renderSvg(layout(program), program);
  const groups = svg.match(/<g class="element[^"]*" data-id="[^"]+"/g) || [];
  assert.strictEqual(groups.length, 3, '2 contacts + 1 coil');
  assert.ok(svg.includes('data-id="e0"'));
  assert.ok(svg.includes('data-id="e1"'));
  assert.ok(svg.includes('data-id="e2"'));
});

check('energized classes match power flow exactly', () => {
  const program = parseProgram(SIMPLE_LD);
  normalizeIds(program);
  const svg = renderSvg(layout(program), program, FLOW);
  for (const id of ['e0', 'e1', 'e2']) {
    assert.ok(
      hasEnergizedClass(elementTag(svg, id)),
      `${id} must be energized: ${elementTag(svg, id)}`,
    );
  }

  // Kill the flow: nothing energized.
  const dead = JSON.parse(JSON.stringify(FLOW));
  dead.rungs[0] = {
    contact_energized: [[false, false]],
    branch_energized: [false],
    output_energized: [false],
    rung_result: false,
  };
  const deadSvg = renderSvg(layout(program), program, dead);
  assert.ok(!hasEnergizedClass(deadSvg), 'nothing energized when flow is dead');
});

check('partial flow: dead contact not marked energized', () => {
  const program = parseProgram(SIMPLE_LD);
  normalizeIds(program);
  const partial = JSON.parse(JSON.stringify(FLOW));
  partial.rungs[0].contact_energized = [[true, false]]; // Stop blocks
  partial.rungs[0].branch_energized = [false];
  partial.rungs[0].output_energized = [false];
  const svg = renderSvg(layout(program), program, partial);
  assert.ok(hasEnergizedClass(elementTag(svg, 'e0')), 'first contact energized');
  assert.ok(!hasEnergizedClass(elementTag(svg, 'e1')), 'second contact not energized');
  assert.ok(!hasEnergizedClass(elementTag(svg, 'e2')), 'coil not energized');
});

check('output is svg with rails', () => {
  const program = parseProgram(SIMPLE_LD);
  const svg = renderSvg(layout(program), program);
  assert.ok(svg.startsWith('<svg'));
  assert.ok(svg.includes('</svg>'));
});

if (failures > 0) {
  process.exit(1);
}
console.log('render ok');
