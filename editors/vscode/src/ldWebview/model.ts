/**
 * TypeScript mirror of the `plc_ld` model (wire format v2) plus the editor
 * helpers that must stay in lockstep with the Rust side:
 *
 * - `parseProgram` — tolerant parse; defaults `schema_version` like serde.
 * - `normalizeIds` — mirror of `plc_ld::normalize_ids` (r{n}/e{n}, preserve
 *   valid ids, first-claim-wins on duplicates, counters seeded past the
 *   highest numeric suffix).
 * - `serializeProgram` — deterministic 2-space JSON for byte-stable saves.
 *
 * Pure (no DOM, no vscode import) so it is unit-testable in Node.
 */

export const CURRENT_SCHEMA_VERSION = 2;

export interface LdProgram {
  name: string;
  schema_version: number;
  rungs: Rung[];
}

export interface Rung {
  id?: string;
  comment?: string;
  branches: SeriesBranch[];
  outputs: OutputElement[];
}

export interface SeriesBranch {
  elements: ContactElement[];
}

export interface ContactElement {
  id?: string;
  name: string;
  negated: boolean;
}

export type OutputElement = CoilOutput | BlockOutput;

export interface CoilOutput {
  kind: 'coil';
  id?: string;
  name: string;
  variant: 'normal' | 'set' | 'reset';
}

export interface BlockOutput {
  kind: 'block';
  id?: string;
  fb_type: string;
  instance: string;
  inputs: BlockArg[];
  outputs: BlockArg[];
}

export interface BlockArg {
  name: string;
  value: string;
}

/** Parse `.ld` JSON text; throws on malformed JSON. */
export function parseProgram(text: string): LdProgram {
  const raw = JSON.parse(text) as Partial<LdProgram> & { rungs?: Rung[] };
  const program: LdProgram = {
    name: typeof raw.name === 'string' ? raw.name : 'NewProgram',
    schema_version:
      typeof raw.schema_version === 'number' ? raw.schema_version : CURRENT_SCHEMA_VERSION,
    rungs: Array.isArray(raw.rungs) ? raw.rungs : [],
  };
  for (const rung of program.rungs) {
    rung.branches = Array.isArray(rung.branches) ? rung.branches : [];
    rung.outputs = Array.isArray(rung.outputs) ? rung.outputs : [];
    for (const branch of rung.branches) {
      branch.elements = Array.isArray(branch.elements) ? branch.elements : [];
    }
  }
  return program;
}

class IdAllocator {
  private readonly used = new Set<string>();
  private next: number;

  constructor(
    private readonly prefix: string,
    next: number,
  ) {
    this.next = next;
  }

  ensure(slot: { id?: string }): void {
    const keep =
      typeof slot.id === 'string' && slot.id.length > this.prefix.length &&
      slot.id.startsWith(this.prefix) && !this.used.has(slot.id)
        ? (this.used.add(slot.id), true)
        : false;
    if (!keep) {
      slot.id = this.allocate();
    }
  }

  private allocate(): string {
    for (;;) {
      const candidate = `${this.prefix}${this.next}`;
      this.next += 1;
      if (!this.used.has(candidate)) {
        this.used.add(candidate);
        return candidate;
      }
    }
  }
}

/** Mirror of `plc_ld::normalize_ids` — see module docs. Mutates in place. */
export function normalizeIds(program: LdProgram): void {
  const rungs = new IdAllocator('r', seed(program, 'r'));
  const elements = new IdAllocator('e', seed(program, 'e'));

  for (const rung of program.rungs) {
    rungs.ensure(rung);
    for (const branch of rung.branches) {
      for (const contact of branch.elements) {
        elements.ensure(contact);
      }
    }
    for (const output of rung.outputs) {
      elements.ensure(output as { id?: string });
    }
  }
}

function seed(program: LdProgram, prefix: string): number {
  let next = 0;
  const consider = (id: string | undefined): void => {
    // Mirror Rust's `strip_prefix` + `parse::<u64>`: the suffix must be a
    // plain (optionally `+`-signed) decimal digit run — no whitespace, hex,
    // exponent, or empty suffix (JS `Number` would accept all of those).
    if (
      typeof id === 'string' &&
      id.length > prefix.length &&
      id.startsWith(prefix) &&
      /^\+?\d+$/.test(id.slice(prefix.length))
    ) {
      const suffix = Number(id.slice(prefix.length));
      if (Number.isInteger(suffix) && suffix >= next && suffix < Number.MAX_SAFE_INTEGER) {
        next = suffix + 1;
      }
    }
  };
  for (const rung of program.rungs) {
    consider(rung.id);
    for (const branch of rung.branches) {
      for (const contact of branch.elements) {
        consider(contact.id);
      }
    }
    for (const output of rung.outputs) {
      consider(output.id);
    }
  }
  return next;
}

/** Collect unique variable names (contacts, coils, block pin values). */
export function allVariables(program: LdProgram): string[] {
  const vars = new Set<string>();
  for (const rung of program.rungs) {
    for (const branch of rung.branches) {
      for (const contact of branch.elements) {
        vars.add(contact.name);
      }
    }
    for (const output of rung.outputs) {
      if (output.kind === 'coil') {
        vars.add(output.name);
      } else {
        for (const arg of [...output.inputs, ...output.outputs]) {
          vars.add(arg.value);
        }
      }
    }
  }
  return [...vars];
}

/** Deterministic serialization: 2-space pretty, insertion-ordered keys. */
export function serializeProgram(program: LdProgram): string {
  return `${JSON.stringify(program, null, 2)}\n`;
}
