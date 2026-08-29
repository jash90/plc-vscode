/**
 * SimClient: the extension-host side of the `plc ld --serve` protocol
 * (PLC-114). Owns the child process, frames its line-delimited JSON into
 * typed events, and drives tick pacing with a host-side timer — the server
 * is synchronous, so the CLIENT defines 'running'.
 *
 * The client is constructed over any child-like object (real `ChildProcess`
 * in the extension, fakes in tests) — no `vscode` import, node-testable.
 */

import { ChildProcess } from 'node:child_process';

export interface ServeEvent {
  event: string;
  [field: string]: unknown;
}

/** Parse one serve-protocol line; throws loudly on junk. */
export function parseServeEvent(line: string): ServeEvent {
  let parsed: unknown;
  try {
    parsed = JSON.parse(line);
  } catch (error) {
    throw new Error(`bad serve event: ${line}: ${(error as Error).message}`);
  }
  if (
    typeof parsed !== 'object' ||
    parsed === null ||
    typeof (parsed as { event?: unknown }).event !== 'string'
  ) {
    throw new Error(`unknown serve event: ${line}`);
  }
  const event = (parsed as ServeEvent).event;
  switch (event) {
    case 'ready':
    case 'loaded':
    case 'error':
    case 'state':
    case 'powerFlow':
    case 'diagnostics':
      return parsed as ServeEvent;
    default:
      throw new Error(`unknown serve event: ${event}`);
  }
}

/** The child-process surface SimClient consumes (real or fake). */
export interface SimChild {
  stdin: { write(line: string): void };
  stdout: NodeJS.EventEmitter;
  kill(): void;
}

export function asSimChild(child: ChildProcess): SimChild {
  return {
    // The serve protocol guarantees stdin/stdout wiring; the cast mirrors
    // how the extension spawns the CLI.
    stdin: child.stdin as unknown as { write(line: string): void },
    stdout: child.stdout as unknown as NodeJS.EventEmitter,
    kill: () => child.kill(),
  };
}

type Timer = ReturnType<typeof setInterval> | undefined;

export class SimClient {
  private buffer = '';
  private timer: Timer;
  private disposed = false;

  constructor(
    private readonly child: SimChild,
    private readonly onEvent: (event: ServeEvent) => void,
  ) {
    this.child.stdout.on('data', (chunk: Buffer) => this.receive(chunk));
  }

  /** Handshake: hello → ready (with the FB catalog). */
  start(): void {
    this.send({ op: 'hello' });
  }

  /** Load (or live-reload) a model — carries the in-memory JSON, unsaved. */
  reload(modelJson: string): void {
    this.send({ op: 'load', json: modelJson });
  }

  setInput(name: string, value: boolean): void {
    this.send({ op: 'setInput', name, value });
  }

  force(name: string, value: boolean): void {
    this.send({ op: 'force', name, value });
  }

  unforce(name: string): void {
    this.send({ op: 'unforce', name });
  }

  /** One scan, immediately. */
  tick(): void {
    this.send({ op: 'tick' });
  }

  /** Continuous run: arm the host-side tick pacer. */
  run(intervalMs: number): void {
    this.stop();
    this.timer = setInterval(() => this.tick(), intervalMs);
  }

  /** Pause: disarm the pacer (the server keeps its state). */
  stop(): void {
    if (this.timer !== undefined) {
      clearInterval(this.timer);
      this.timer = undefined;
    }
  }

  /** Tear down: stop pacing and kill the child. */
  dispose(): void {
    if (this.disposed) {
      return;
    }
    this.disposed = true;
    this.stop();
    this.child.kill();
  }

  private send(op: Record<string, unknown>): void {
    if (!this.disposed) {
      this.child.stdin.write(`${JSON.stringify(op)}\n`);
    }
  }

  private receive(chunk: Buffer): void {
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
      try {
        this.onEvent(parseServeEvent(line));
      } catch {
        // Unknown/junk lines never take the session down.
      }
    }
  }
}
