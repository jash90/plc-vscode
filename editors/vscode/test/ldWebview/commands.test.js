// PLC-111 — domain command layer: pure apply over immutable snapshots with
// an undo/redo history. The webview host and the document lifecycle both
// build on this.
'use strict';
const assert = require('node:assert');
const {
  applyCommand,
  CommandHistory,
  commands,
} = require('../../dist/ldWebview/commands.js');
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

function emptyProgram() {
  return parseProgram('{"name":"P","rungs":[]}');
}

check('addContact applies and records undo', () => {
  const history = new CommandHistory();
  let program = emptyProgram();
  program = applyCommand(program, commands.addRung(), history);
  program = applyCommand(program, commands.addContact(0, 0, 'Start', false), history);

  assert.strictEqual(program.rungs.length, 1);
  assert.strictEqual(program.rungs[0].branches[0].elements.length, 1);
  assert.strictEqual(program.rungs[0].branches[0].elements[0].name, 'Start');
  assert.strictEqual(history.undoDepth, 2);

  program = history.undo(program);
  assert.strictEqual(program.rungs[0].branches.length, 0, 'undo removes the contact (and its branch)');
  program = history.undo(program);
  assert.strictEqual(program.rungs.length, 0, 'undo removes rung');
  assert.strictEqual(history.undoDepth, 0);
});

check('deleteElement shifts and undoes', () => {
  const history = new CommandHistory();
  let program = parseProgram(
    '{"name":"P","rungs":[{"branches":[{"elements":[{"id":"e0","name":"A"},{"id":"e1","name":"B"}]}],"outputs":[]}]}',
  );
  program = applyCommand(program, commands.deleteElement(0, 0, 1), history);
  assert.deepStrictEqual(
    program.rungs[0].branches[0].elements.map((e) => e.name),
    ['A'],
  );
  program = history.undo(program);
  assert.deepStrictEqual(
    program.rungs[0].branches[0].elements.map((e) => e.name),
    ['A', 'B'],
  );
  assert.strictEqual(program.rungs[0].branches[0].elements[1].id, 'e1', 'ids preserved on undo');
});

check('toggleNegate flips back', () => {
  const history = new CommandHistory();
  let program = parseProgram(
    '{"name":"P","rungs":[{"branches":[{"elements":[{"id":"e0","name":"A","negated":false}]}],"outputs":[]}]}',
  );
  program = applyCommand(program, commands.toggleNegate(0, 0, 0), history);
  assert.strictEqual(program.rungs[0].branches[0].elements[0].negated, true);
  program = history.undo(program);
  assert.strictEqual(program.rungs[0].branches[0].elements[0].negated, false);
});

check('insertParallelBranch creates a second branch', () => {
  const history = new CommandHistory();
  let program = parseProgram(
    '{"name":"P","rungs":[{"branches":[{"elements":[{"name":"A"}]}],"outputs":[]}]}',
  );
  program = applyCommand(program, commands.insertParallelBranch(0, 'B'), history);
  assert.strictEqual(program.rungs[0].branches.length, 2);
  assert.strictEqual(program.rungs[0].branches[1].elements[0].name, 'B');
  program = history.undo(program);
  assert.strictEqual(program.rungs[0].branches.length, 1);
});

check('renameVariable updates only the target element', () => {
  const history = new CommandHistory();
  let program = parseProgram(
    '{"name":"P","rungs":[{"branches":[{"elements":[{"id":"e0","name":"A"},{"id":"e1","name":"B"}]}],' +
      '"outputs":[{"kind":"coil","id":"e2","name":"C","variant":"normal"}]}]}',
  );
  program = applyCommand(program, commands.renameVariable(0, -1, 0, 'Motor'), history);
  assert.strictEqual(program.rungs[0].outputs[0].name, 'Motor');
  assert.strictEqual(program.rungs[0].branches[0].elements[0].name, 'A', 'contacts untouched');
  program = history.undo(program);
  assert.strictEqual(program.rungs[0].outputs[0].name, 'C');
});

check('setCoilVariant round-trips', () => {
  const history = new CommandHistory();
  let program = parseProgram(
    '{"name":"P","rungs":[{"branches":[],"outputs":[{"kind":"coil","id":"e0","name":"C","variant":"normal"}]}]}',
  );
  program = applyCommand(program, commands.setCoilVariant(0, 0, 'set'), history);
  assert.strictEqual(program.rungs[0].outputs[0].variant, 'set');
  program = history.undo(program);
  assert.strictEqual(program.rungs[0].outputs[0].variant, 'normal');
});

check('setRungComment round-trips', () => {
  const history = new CommandHistory();
  let program = emptyProgram();
  program = applyCommand(program, commands.addRung(), history);
  program = applyCommand(program, commands.setRungComment(0, 'Seal-in'), history);
  assert.strictEqual(program.rungs[0].comment, 'Seal-in');
  program = history.undo(program);
  assert.strictEqual(program.rungs[0].comment, undefined);
});

check('deleteRung shifts later rungs and undoes', () => {
  const history = new CommandHistory();
  let program = parseProgram(
    '{"name":"P","rungs":[{"branches":[],"outputs":[]},{"branches":[{"elements":[{"id":"e0","name":"X"}]}],"outputs":[]}]}',
  );
  program = applyCommand(program, commands.deleteRung(0), history);
  assert.strictEqual(program.rungs.length, 1);
  assert.strictEqual(program.rungs[0].branches[0].elements[0].name, 'X');
  program = history.undo(program);
  assert.strictEqual(program.rungs.length, 2);
});

check('commands carry label and element id', () => {
  const command = commands.renameVariable(0, 0, 1, 'Motor');
  assert.ok(typeof command.label === 'string' && command.label.length > 0);
  const program = parseProgram(
    '{"name":"P","rungs":[{"branches":[{"elements":[{"id":"e0","name":"A"},{"id":"e1","name":"B"}]}],"outputs":[]}]}',
  );
  const applied = applyCommand(program, command, new CommandHistory());
  void applied;
  // The command itself is data: serialization round-trips.
  assert.deepStrictEqual(JSON.parse(JSON.stringify(command)), command);
});

check('normalize runs on load and before save', () => {
  const history = new CommandHistory();
  let program = parseProgram('{"name":"P","rungs":[{"branches":[{"elements":[{"name":"A"}]}],"outputs":[]}]}');
  normalizeIds(program); // load normalization
  program = applyCommand(program, commands.addContact(0, 0, 'B', false), history);
  normalizeIds(program); // save normalization
  const ids = program.rungs[0].branches[0].elements.map((e) => e.id);
  assert.deepStrictEqual(ids, ['e0', 'e1'], `stable ids across edit: ${ids}`);
});

check('applyCommand never mutates the input program', () => {
  const program = parseProgram(
    '{"name":"P","rungs":[{"branches":[{"elements":[{"id":"e0","name":"A"}]}],"outputs":[]}]}',
  );
  const before = JSON.stringify(program);
  applyCommand(program, commands.renameVariable(0, 0, 0, 'Renamed'), new CommandHistory());
  assert.strictEqual(JSON.stringify(program), before, 'input must stay immutable');
});

check('redo after undo restores the edited state', () => {
  const history = new CommandHistory();
  let program = emptyProgram();
  program = applyCommand(program, commands.addRung(), history);
  program = history.undo(program);
  assert.strictEqual(program.rungs.length, 0);
  program = history.redo(program);
  assert.strictEqual(program.rungs.length, 1, 'redo restores');
  // A new command clears the redo tail (standard editor semantics).
  program = history.undo(program);
  program = applyCommand(program, commands.addRung(), history);
  assert.strictEqual(history.redoDepth, 0, 'redo tail cleared by new command');
});


// Appended (PLC-111): wholesale model replacement is one undo step.
check('replaceProgram swaps the model and undoes', () => {
  const history = new CommandHistory();
  let program = emptyProgram();
  program = applyCommand(program, commands.addRung(), history);
  const replacement = parseProgram(
    '{"name":"Q","rungs":[{"branches":[{"elements":[{"id":"k0","name":"Z"}]}],"outputs":[]}]}',
  );
  program = applyCommand(program, commands.replaceProgram(replacement), history);
  assert.strictEqual(program.name, 'Q');
  assert.strictEqual(program.rungs[0].branches[0].elements[0].name, 'Z');
  program = history.undo(program);
  assert.strictEqual(program.name, 'P', 'undo restores the previous model');
  assert.strictEqual(program.rungs.length, 1, 'pre-replacement state intact');
  program = history.redo(program);
  assert.strictEqual(program.name, 'Q', 'redo reapplies the replacement');
});


// Appended (review): palette sequences are computed purely against a program.
check('palette on empty program yields addRung then the element', () => {
  const { paletteCommands } = require('../../dist/ldWebview/commands.js');
  const program = emptyProgram();
  const sequence = paletteCommands(program, 'no-contact');
  assert.deepStrictEqual(
    sequence.map((c) => c.type),
    ['addRung', 'addContact'],
    'empty program: rung first, then the contact addressed at rung 0',
  );
  assert.strictEqual(sequence[1].rung, 0);
  assert.strictEqual(sequence[1].name, 'NewVar');
});

check('palette on existing program addresses the last rung once', () => {
  const { paletteCommands } = require('../../dist/ldWebview/commands.js');
  const program = parseProgram(
    '{"name":"P","rungs":[{"branches":[{"elements":[{"name":"A"}]}],"outputs":[]}]}',
  );
  const sequence = paletteCommands(program, 'ton');
  assert.strictEqual(sequence.length, 1);
  assert.strictEqual(sequence[0].type, 'addBlock');
  assert.strictEqual(sequence[0].rung, 0);
  assert.strictEqual(sequence[0].output.fb_type, 'TON');
});


// Appended (PLC-112): drop-target commands.
check('insertContact splices at the drop index and undoes', () => {
  const history = new CommandHistory();
  let program = parseProgram(
    '{"name":"P","rungs":[{"branches":[{"elements":[{"id":"e0","name":"A"},{"id":"e1","name":"B"}]}],"outputs":[]}]}',
  );
  program = applyCommand(program, commands.insertContact(0, 0, 1, 'X', false), history);
  assert.deepStrictEqual(
    program.rungs[0].branches[0].elements.map((e) => e.name),
    ['A', 'X', 'B'],
  );
  program = history.undo(program);
  assert.deepStrictEqual(
    program.rungs[0].branches[0].elements.map((e) => e.name),
    ['A', 'B'],
  );
});

check('moveElement reorders within a branch', () => {
  const history = new CommandHistory();
  let program = parseProgram(
    '{"name":"P","rungs":[{"branches":[{"elements":[{"id":"e0","name":"A"},{"id":"e1","name":"B"},{"id":"e2","name":"C"}]}],"outputs":[]}]}',
  );
  program = applyCommand(program, commands.moveElement({ rung: 0, branch: 0, index: 2 }, { rung: 0, branch: 0, index: 0 }), history);
  assert.deepStrictEqual(
    program.rungs[0].branches[0].elements.map((e) => e.name),
    ['C', 'A', 'B'],
  );
  program = history.undo(program);
  assert.deepStrictEqual(
    program.rungs[0].branches[0].elements.map((e) => e.name),
    ['A', 'B', 'C'],
  );
});

check('moveElement relocates across branches', () => {
  const history = new CommandHistory();
  let program = parseProgram(
    '{"name":"P","rungs":[{"branches":[{"elements":[{"id":"e0","name":"A"},{"id":"e1","name":"B"}]},{"elements":[{"id":"e2","name":"C"}]}],"outputs":[]}]}',
  );
  program = applyCommand(program, commands.moveElement({ rung: 0, branch: 0, index: 1 }, { rung: 0, branch: 1, index: 0 }), history);
  assert.deepStrictEqual(program.rungs[0].branches[0].elements.map((e) => e.name), ['A']);
  assert.deepStrictEqual(program.rungs[0].branches[1].elements.map((e) => e.name), ['B', 'C']);
});

if (failures > 0) {
  process.exit(1);
}
console.log('commands ok');
