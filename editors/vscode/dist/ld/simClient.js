"use strict";
/**
 * SimClient: the extension-host side of the `plc ld --serve` protocol
 * (PLC-114). Owns the child process, frames its line-delimited JSON into
 * typed events, and drives tick pacing with a host-side timer — the server
 * is synchronous, so the CLIENT defines 'running'.
 *
 * The client is constructed over any child-like object (real `ChildProcess`
 * in the extension, fakes in tests) — no `vscode` import, node-testable.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.SimClient = void 0;
exports.parseServeEvent = parseServeEvent;
exports.asSimChild = asSimChild;
/** Parse one serve-protocol line; throws loudly on junk. */
function parseServeEvent(line) {
    let parsed;
    try {
        parsed = JSON.parse(line);
    }
    catch (error) {
        throw new Error(`bad serve event: ${line}: ${error.message}`);
    }
    if (typeof parsed !== 'object' ||
        parsed === null ||
        typeof parsed.event !== 'string') {
        throw new Error(`unknown serve event: ${line}`);
    }
    const event = parsed.event;
    switch (event) {
        case 'ready':
        case 'loaded':
        case 'error':
        case 'state':
        case 'powerFlow':
        case 'diagnostics':
            return parsed;
        default:
            throw new Error(`unknown serve event: ${event}`);
    }
}
function asSimChild(child) {
    return {
        // The serve protocol guarantees stdin/stdout wiring; the cast mirrors
        // how the extension spawns the CLI.
        stdin: child.stdin,
        stdout: child.stdout,
        events: child,
        kill: () => child.kill(),
    };
}
class SimClient {
    child;
    onEvent;
    buffer = '';
    timer;
    disposed = false;
    constructor(child, onEvent) {
        this.child = child;
        this.onEvent = onEvent;
        this.child.stdout.on('data', (chunk) => this.receive(chunk));
        // Child death (crash, external kill, failed spawn): stop pacing and
        // mark disposed so sends never hit a dead pipe. Without this, the
        // tick interval keeps writing to a closed stdin → EPIPE crashes.
        this.child.events.on('close', () => {
            this.disposed = true;
            this.stop();
        });
        this.child.events.on('error', () => {
            this.disposed = true;
            this.stop();
        });
    }
    /** Handshake: hello → ready (with the FB catalog). */
    start() {
        this.send({ op: 'hello' });
    }
    /** Load (or live-reload) a model — carries the in-memory JSON, unsaved. */
    reload(modelJson) {
        this.send({ op: 'load', json: modelJson });
    }
    setInput(name, value) {
        this.send({ op: 'setInput', name, value });
    }
    force(name, value) {
        this.send({ op: 'force', name, value });
    }
    unforce(name) {
        this.send({ op: 'unforce', name });
    }
    /** One scan, immediately. */
    tick() {
        this.send({ op: 'tick' });
    }
    /** Continuous run: align the server clock, then arm the tick pacer. */
    run(intervalMs) {
        this.stop();
        this.send({ op: 'setInterval', ms: intervalMs });
        this.timer = setInterval(() => this.tick(), intervalMs);
    }
    /** Pause: disarm the pacer (the server keeps its state). */
    stop() {
        if (this.timer !== undefined) {
            clearInterval(this.timer);
            this.timer = undefined;
        }
    }
    /** Tear down: stop pacing and kill the child. */
    dispose() {
        if (this.disposed) {
            return;
        }
        this.disposed = true;
        this.stop();
        this.child.kill();
    }
    send(op) {
        if (!this.disposed) {
            this.child.stdin.write(`${JSON.stringify(op)}\n`);
        }
    }
    receive(chunk) {
        this.buffer += chunk.toString();
        for (;;) {
            const newline = this.buffer.indexOf('\n');
            if (newline === -1) {
                return;
            }
            const line = this.buffer.slice(0, newline).trim();
            this.buffer = this.buffer.slice(newline + 1);
            if (line.length === 0) {
                continue;
            }
            let event;
            try {
                event = parseServeEvent(line);
            }
            catch {
                // Unknown/junk lines never take the session down; consumer errors
                // must propagate instead of being swallowed per line.
                continue;
            }
            this.onEvent(event);
        }
    }
}
exports.SimClient = SimClient;
//# sourceMappingURL=simClient.js.map