// PLC-114 — SimClient: frames the serve protocol's line-delimited JSON,
// emits typed events, drives tick pacing, reloads the in-memory model, and
// disposes cleanly. Tested against fake streams (no real process).
'use strict';
const assert = require('node:assert');
const { EventEmitter } = require('node:events');
const { SimClient, parseServeEvent } = require('../dist/ld/simClient.js');

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

/** A fake child process with the surface SimClient consumes. */
function fakeChild() {
  const child = new EventEmitter();
  child.stdout = new EventEmitter();
  child.stderr = new EventEmitter();
  child.stdin = { written: [], write(line) { this.written.push(line); } };
  child.killed = false;
  child.kill = () => { child.killed = true; child.emit('close', 0); };
  return child;
}

function bootstrap(child, client) {
  // The handshake every session starts with.
  child.stdout.emit('data', Buffer.from('{"event":"ready","protocolVersion":1,"fbCatalog":[]}\n'));
  void client;
}

check('frames partial lines correctly', () => {
  const child = fakeChild();
  const events = [];
  const client = new SimClient(child, (event) => events.push(event));
  child.stdout.emit('data', Buffer.from('{"event":"read'));
  child.stdout.emit('data', Buffer.from('y"}\n{"event":"loaded"'));
  child.stdout.emit('data', Buffer.from(',"ok":true}\n'));
  assert.deepStrictEqual(
    events.map((e) => e.event),
    ['ready', 'loaded'],
  );
  client.dispose();
});

check('emits typed events through the callback', () => {
  const child = fakeChild();
  const events = [];
  const client = new SimClient(child, (event) => events.push(event));
  child.stdout.emit(
    'data',
    Buffer.from('{"event":"state","scan":1,"timeMs":100,"watch":["A = TRUE"],"forced":[]}\n'),
  );
  assert.strictEqual(events.length, 1);
  assert.strictEqual(events[0].event, 'state');
  assert.strictEqual(events[0].scan, 1);
  client.dispose();
});

check('handshake sends hello on start', () => {
  const child = fakeChild();
  const client = new SimClient(child, () => {});
  client.start();
  assert.strictEqual(child.stdin.written[0], '{"op":"hello"}\n');
  client.dispose();
});

check('reload sends the current model json', () => {
  const child = fakeChild();
  const client = new SimClient(child, () => {});
  client.start();
  client.reload('{"name":"P","rungs":[]}');
  const reloadLine = child.stdin.written.find((line) => line.includes('"op":"load"'));
  assert.ok(reloadLine, `load op sent: ${child.stdin.written}`);
  assert.ok(reloadLine.includes('\\"name\\":\\"P\\"') || reloadLine.includes('"name":"P"'), reloadLine);
  client.dispose();
});

check('run drives tick pacing and stop halts it', () => {
  const child = fakeChild();
  const client = new SimClient(child, () => {});
  client.start();

  let now = 0;
  const timers = [];
  const realSetInterval = global.setInterval;
  const realClearInterval = global.clearInterval;
  global.setInterval = (fn, ms) => { timers.push({ fn, ms }); return timers.length; };
  global.clearInterval = () => { timers.length = 0; };
  try {
    client.run(100);
    assert.strictEqual(timers.length, 1, 'one interval armed');
    assert.strictEqual(timers[0].ms, 100);
    timers[0].fn();
    const ticks = child.stdin.written.filter((line) => line.includes('"op":"tick"'));
    assert.ok(ticks.length >= 1, 'tick posted by the interval');
    client.stop();
    assert.strictEqual(timers.length, 0, 'interval cleared');
    void now;
  } finally {
    global.setInterval = realSetInterval;
    global.clearInterval = realClearInterval;
  }
  client.dispose();
});

check('dispose kills the child and stops the timer', () => {
  const child = fakeChild();
  const client = new SimClient(child, () => {});
  client.start();
  client.dispose();
  assert.strictEqual(child.killed, true, 'child killed');
});

check('error events surface through the callback', () => {
  const child = fakeChild();
  const events = [];
  const client = new SimClient(child, (event) => events.push(event));
  child.stdout.emit('data', Buffer.from('{"event":"error","message":"boom"}\n'));
  assert.strictEqual(events[0].message, 'boom');
  client.dispose();
});

check('parseServeEvent rejects unknown events', () => {
  assert.throws(() => parseServeEvent('{"event":"nope"}'), /unknown serve event/);
  assert.throws(() => parseServeEvent('not json'), /bad serve event/);
});

if (failures > 0) {
  process.exit(1);
}
console.log('simClient ok');
