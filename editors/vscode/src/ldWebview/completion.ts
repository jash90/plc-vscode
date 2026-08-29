/**
 * Completion sources for the LD editor: program variables (prefix match)
 * and the standard function-block catalog with pin tables.
 *
 * The pin tables mirror `plc_ld::validate::{STANDARD_FB_TYPES, fb_pins}`;
 * the serve protocol (later task) will carry the Rust catalog as the
 * authoritative source. Pure — unit-testable in Node.
 */

import { LdProgram } from './model';

export interface FbPinTable {
  fbType: string;
  inputs: string[];
  outputs: string[];
}

/** Standard FB catalog (keep in sync with plc_ld::validate). */
const FB_CATALOG: FbPinTable[] = [
  { fbType: 'TON', inputs: ['IN', 'PT'], outputs: ['Q', 'ET'] },
  { fbType: 'TOF', inputs: ['IN', 'PT'], outputs: ['Q', 'ET'] },
  { fbType: 'TP', inputs: ['IN', 'PT'], outputs: ['Q', 'ET'] },
  { fbType: 'CTU', inputs: ['CU', 'RESET', 'PV'], outputs: ['Q', 'CV'] },
  { fbType: 'CTD', inputs: ['CD', 'LOAD', 'PV'], outputs: ['Q', 'CV'] },
  { fbType: 'CTUD', inputs: ['CU', 'CD', 'RESET', 'LOAD', 'PV'], outputs: ['QU', 'QD', 'CV'] },
  { fbType: 'R_TRIG', inputs: ['CLK'], outputs: ['Q'] },
  { fbType: 'F_TRIG', inputs: ['CLK'], outputs: ['Q'] },
];

/** Program variables matching a prefix (case-insensitive), excluding an
 * exact full match (nothing left to complete). */
export function variables(program: LdProgram, prefix: string): string[] {
  const needle = prefix.trim().toLowerCase();
  if (needle.length === 0) {
    return [];
  }
  const seen = new Set<string>();
  const result: string[] = [];
  for (const name of collectVariables(program)) {
    const lower = name.toLowerCase();
    if (lower.startsWith(needle) && lower !== needle && !seen.has(name)) {
      seen.add(name);
      result.push(name);
    }
  }
  return result;
}

function collectVariables(program: LdProgram): string[] {
  const names: string[] = [];
  for (const rung of program.rungs) {
    for (const branch of rung.branches) {
      for (const contact of branch.elements) {
        names.push(contact.name);
      }
    }
    for (const output of rung.outputs) {
      if (output.kind === 'coil') {
        names.push(output.name);
      } else {
        for (const arg of [...output.inputs, ...output.outputs]) {
          names.push(arg.value);
        }
      }
    }
  }
  return names;
}

/** The catalog's FB type names. */
export function fbCatalog(): string[] {
  return FB_CATALOG.map((entry) => entry.fbType);
}

/** Pin table for a standard FB (case-insensitive), or undefined. */
export function pinsFor(fbType: string): FbPinTable | undefined {
  return FB_CATALOG.find((entry) => entry.fbType.toLowerCase() === fbType.trim().toLowerCase());
}
