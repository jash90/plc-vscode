"use strict";
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
Object.defineProperty(exports, "__esModule", { value: true });
exports.CURRENT_SCHEMA_VERSION = void 0;
exports.parseProgram = parseProgram;
exports.normalizeIds = normalizeIds;
exports.allVariables = allVariables;
exports.serializeProgram = serializeProgram;
exports.CURRENT_SCHEMA_VERSION = 2;
/** Parse `.ld` JSON text; throws on malformed JSON. */
function parseProgram(text) {
    const raw = JSON.parse(text);
    const program = {
        name: typeof raw.name === 'string' ? raw.name : 'NewProgram',
        schema_version: typeof raw.schema_version === 'number' ? raw.schema_version : exports.CURRENT_SCHEMA_VERSION,
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
    prefix;
    used = new Set();
    next;
    constructor(prefix, next) {
        this.prefix = prefix;
        this.next = next;
    }
    ensure(slot) {
        const keep = typeof slot.id === 'string' && slot.id.length > this.prefix.length &&
            slot.id.startsWith(this.prefix) && !this.used.has(slot.id)
            ? (this.used.add(slot.id), true)
            : false;
        if (!keep) {
            slot.id = this.allocate();
        }
    }
    allocate() {
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
function normalizeIds(program) {
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
            elements.ensure(output);
        }
    }
}
function seed(program, prefix) {
    let next = 0;
    const consider = (id) => {
        // Mirror Rust's `strip_prefix` + `parse::<u64>`: the suffix must be a
        // plain (optionally `+`-signed) decimal digit run — no whitespace, hex,
        // exponent, or empty suffix (JS `Number` would accept all of those).
        if (typeof id === 'string' &&
            id.length > prefix.length &&
            id.startsWith(prefix) &&
            /^\+?\d+$/.test(id.slice(prefix.length))) {
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
function allVariables(program) {
    const vars = new Set();
    for (const rung of program.rungs) {
        for (const branch of rung.branches) {
            for (const contact of branch.elements) {
                vars.add(contact.name);
            }
        }
        for (const output of rung.outputs) {
            if (output.kind === 'coil') {
                vars.add(output.name);
            }
            else {
                for (const arg of [...output.inputs, ...output.outputs]) {
                    vars.add(arg.value);
                }
            }
        }
    }
    return [...vars];
}
/** Deterministic serialization: 2-space pretty, insertion-ordered keys. */
function serializeProgram(program) {
    return `${JSON.stringify(program, null, 2)}\n`;
}
//# sourceMappingURL=model.js.map