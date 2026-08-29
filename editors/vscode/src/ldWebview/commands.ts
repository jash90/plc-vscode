/**
 * Domain command layer: a pure `apply` over deep-cloned program snapshots
 * plus a snapshot-based undo/redo history. Commands are first-class data
 * (labels, element addressing) so the document lifecycle, palette, and
 * keyboard map all share one vocabulary.
 *
 * Element addressing: `(rung, branch, index)`; `branch === -1` addresses an
 * output element at `index`.
 *
 * Pure (no DOM, no vscode import) — unit-testable in Node.
 */

import { CoilOutput, BlockOutput, LdProgram, OutputElement } from './model';

export interface LdCommand {
  type: string;
  label: string;
  rung: number;
  branch: number;
  index: number;
  [payload: string]: unknown;
}

function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

/** Command factories (also the palette/keyboard vocabulary). */
export const commands = {
  addRung(): LdCommand {
    return { type: 'addRung', label: 'Add rung', rung: -1, branch: -1, index: -1 };
  },
  deleteRung(rung: number): LdCommand {
    return { type: 'deleteRung', label: `Delete rung ${rung + 1}`, rung, branch: -1, index: -1 };
  },
  setRungComment(rung: number, comment: string): LdCommand {
    return { type: 'setRungComment', label: 'Comment rung', rung, branch: -1, index: -1, comment };
  },
  addContact(rung: number, branch: number, name: string, negated: boolean): LdCommand {
    return { type: 'addContact', label: `Add contact ${name}`, rung, branch, index: -1, name, negated };
  },
  /** Insert a contact at a specific position in a branch (drop target). */
  insertContact(
    rung: number,
    branch: number,
    index: number,
    name: string,
    negated: boolean,
  ): LdCommand {
    return { type: 'insertContact', label: `Insert contact ${name}`, rung, branch, index, name, negated };
  },
  /** Move an element to a series position (reorder/branch change). */
  moveElement(
    from: { rung: number; branch: number; index: number },
    to: { rung: number; branch: number; index: number },
  ): LdCommand {
    return {
      type: 'moveElement',
      label: 'Move element',
      rung: from.rung,
      branch: from.branch,
      index: from.index,
      toRung: to.rung,
      toBranch: to.branch,
      toIndex: to.index,
    };
  },
  insertParallelBranch(rung: number, contactName: string): LdCommand {
    return {
      type: 'insertParallelBranch',
      label: 'Add parallel branch',
      rung,
      branch: -1,
      index: -1,
      name: contactName,
    };
  },
  addCoil(rung: number, name: string, variant: CoilOutput['variant']): LdCommand {
    return { type: 'addCoil', label: `Add coil ${name}`, rung, branch: -1, index: -1, name, variant };
  },
  addBlock(rung: number, output: BlockOutput): LdCommand {
    return { type: 'addBlock', label: `Add ${output.fb_type}`, rung, branch: -1, index: -1, output };
  },
  deleteElement(rung: number, branch: number, index: number): LdCommand {
    return { type: 'deleteElement', label: 'Delete element', rung, branch, index };
  },
  toggleNegate(rung: number, branch: number, index: number): LdCommand {
    return { type: 'toggleNegate', label: 'Toggle contact type', rung, branch, index };
  },
  renameVariable(rung: number, branch: number, index: number, name: string): LdCommand {
    return { type: 'renameVariable', label: `Rename to ${name}`, rung, branch, index, name };
  },
  setCoilVariant(rung: number, outputIndex: number, variant: CoilOutput['variant']): LdCommand {
    return {
      type: 'setCoilVariant',
      label: `Coil → (${variant})`,
      rung,
      branch: -1,
      index: outputIndex,
      variant,
    };
  },
  /** Wholesale model replacement (the JSON textarea path), one undo step. */
  replaceProgram(program: LdProgram): LdCommand {
    return {
      type: 'replaceProgram',
      label: 'Edit JSON',
      rung: -1,
      branch: -1,
      index: -1,
      program,
    };
  },
};

/**
 * The command sequence a palette click performs, computed against the given
 * program state (pure — the webview computes the whole sequence before
 * sending, so a not-yet-updated local program cannot corrupt addressing).
 * An empty program yields [addRung, element]; otherwise [element].
 */
export function paletteCommands(program: LdProgram, paletteType: string): LdCommand[] {
  const elementCommand = (rung: number): LdCommand | undefined => {
    switch (paletteType) {
      case 'no-contact':
      case 'nc-contact': {
        const branches = program.rungs[rung]?.branches.length ?? 0;
        return commands.addContact(rung, Math.max(branches - 1, 0), 'NewVar', paletteType === 'nc-contact');
      }
      case 'coil':
      case 'set-coil':
      case 'reset-coil':
        return commands.addCoil(
          rung,
          'OutVar',
          paletteType === 'coil' ? 'normal' : paletteType === 'set-coil' ? 'set' : 'reset',
        );
      case 'ton':
        return commands.addBlock(rung, {
          kind: 'block',
          fb_type: 'TON',
          instance: 'TON_inst',
          inputs: [
            { name: 'IN', value: 'NewVar' },
            { name: 'PT', value: 'T#1s' },
          ],
          outputs: [{ name: 'Q', value: 'Done' }],
        });
      case 'ctu':
        return commands.addBlock(rung, {
          kind: 'block',
          fb_type: 'CTU',
          instance: 'CTU_inst',
          inputs: [
            { name: 'CU', value: 'NewVar' },
            { name: 'PV', value: '10' },
          ],
          outputs: [{ name: 'Q', value: 'Done' }],
        });
      default:
        return undefined;
    }
  };

  if (program.rungs.length === 0) {
    const element = elementCommand(0);
    return element ? [commands.addRung(), element] : [];
  }
  const element = elementCommand(program.rungs.length - 1);
  return element ? [element] : [];
}

/** Apply a command to a program; never mutates the input. */
export function applyCommand(
  program: LdProgram,
  command: LdCommand,
  history?: CommandHistory,
): LdProgram {
  const next = clone(program);
  mutate(next, command);
  if (history) {
    history.push({ label: command.label, before: program, after: next });
  }
  return next;
}

interface HistoryEntry {
  label: string;
  before: LdProgram;
  after: LdProgram;
}

/** Snapshot-based undo/redo over command entries. */
export class CommandHistory {
  private readonly undoStack: HistoryEntry[] = [];
  private readonly redoStack: HistoryEntry[] = [];

  get undoDepth(): number {
    return this.undoStack.length;
  }

  get redoDepth(): number {
    return this.redoStack.length;
  }

  /** Undo the last entry; `current` is returned unchanged when empty. */
  undo(current: LdProgram): LdProgram {
    const entry = this.undoStack.pop();
    if (!entry) {
      return current;
    }
    this.redoStack.push(entry);
    return entry.before;
  }

  /** Redo the last undone entry. */
  redo(current: LdProgram): LdProgram {
    const entry = this.redoStack.pop();
    if (!entry) {
      return current;
    }
    this.undoStack.push(entry);
    return entry.after;
  }

  /** Record an applied entry and clear the redo tail (editor semantics). */
  push(entry: HistoryEntry): void {
    this.undoStack.push(entry);
    this.redoStack.length = 0;
  }

  /** Drop all history (document reverted/reloaded). */
  clear(): void {
    this.undoStack.length = 0;
    this.redoStack.length = 0;
  }
}

function mutate(program: LdProgram, command: LdCommand): void {
  const rung = program.rungs[command.rung];
  switch (command.type) {
    case 'addRung':
      program.rungs.push({ branches: [], outputs: [] });
      return;
    case 'deleteRung':
      program.rungs.splice(command.rung, 1);
      return;
    case 'setRungComment': {
      if (rung) {
        rung.comment = command.comment as string;
      }
      return;
    }
    case 'addContact': {
      if (!rung) {
        return;
      }
      while (rung.branches.length <= command.branch) {
        rung.branches.push({ elements: [] });
      }
      rung.branches[command.branch].elements.push({
        name: command.name as string,
        negated: Boolean(command.negated),
      });
      return;
    }
    case 'insertContact': {
      if (!rung) {
        return;
      }
      while (rung.branches.length <= command.branch) {
        rung.branches.push({ elements: [] });
      }
      rung.branches[command.branch].elements.splice(
        Math.max(command.index, 0),
        0,
        { name: command.name as string, negated: Boolean(command.negated) },
      );
      return;
    }
    case 'moveElement': {
      const fromRung = program.rungs[command.rung];
      const toRung = program.rungs[command.toRung as number];
      if (!fromRung || !toRung) {
        return;
      }
      if (command.branch === -1) {
        // Move an output to another output slot.
        const [output] = fromRung.outputs.splice(command.index, 1);
        if (output) {
          toRung.outputs.splice(Math.max(command.toIndex as number, 0), 0, output);
        }
        return;
      }
      const sourceBranch = fromRung.branches[command.branch];
      const [contact] = sourceBranch?.elements.splice(command.index, 1) ?? [];
      if (!contact) {
        return;
      }
      // Prune the emptied source branch — an empty branch is an LD0001
      // error state the editor must never create.
      if (sourceBranch && sourceBranch.elements.length === 0) {
        fromRung.branches.splice(command.branch, 1);
      }
      while (toRung.branches.length <= (command.toBranch as number)) {
        toRung.branches.push({ elements: [] });
      }
      toRung.branches[command.toBranch as number]?.elements.splice(
        Math.max(command.toIndex as number, 0),
        0,
        contact,
      );
      return;
    }
    case 'insertParallelBranch': {
      if (!rung) {
        return;
      }
      rung.branches.push({
        elements: [{ name: command.name as string, negated: false }],
      });
      return;
    }
    case 'addCoil': {
      if (!rung) {
        return;
      }
      rung.outputs.push({
        kind: 'coil',
        name: command.name as string,
        variant: command.variant as CoilOutput['variant'],
      });
      return;
    }
    case 'addBlock': {
      if (!rung) {
        return;
      }
      rung.outputs.push(clone(command.output as BlockOutput));
      return;
    }
    case 'deleteElement': {
      if (!rung) {
        return;
      }
      if (command.branch === -1) {
        rung.outputs.splice(command.index, 1);
      } else {
        const branch = rung.branches[command.branch];
        branch?.elements.splice(command.index, 1);
        if (branch && branch.elements.length === 0) {
          rung.branches.splice(command.branch, 1);
        }
      }
      return;
    }
    case 'toggleNegate': {
      const contact = rung?.branches[command.branch]?.elements[command.index];
      if (contact) {
        contact.negated = !contact.negated;
      }
      return;
    }
    case 'renameVariable': {
      const contact = rung?.branches[command.branch]?.elements[command.index];
      if (contact) {
        contact.name = command.name as string;
        return;
      }
      const output = command.branch === -1 ? rung?.outputs[command.index] : undefined;
      if (output && output.kind === 'coil') {
        (output as CoilOutput).name = command.name as string;
      }
      return;
    }
    case 'setCoilVariant': {
      const output: OutputElement | undefined = rung?.outputs[command.index];
      if (output && output.kind === 'coil') {
        output.variant = command.variant as CoilOutput['variant'];
      }
      return;
    }
    case 'replaceProgram': {
      const replacement = command.program as LdProgram;
      program.name = replacement.name;
      program.schema_version = replacement.schema_version;
      program.rungs = clone(replacement.rungs);
      return;
    }
    default:
      return;
  }
}
