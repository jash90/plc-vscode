"use strict";
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
Object.defineProperty(exports, "__esModule", { value: true });
exports.CommandHistory = exports.commands = void 0;
exports.paletteCommands = paletteCommands;
exports.applyCommand = applyCommand;
function clone(value) {
    return JSON.parse(JSON.stringify(value));
}
/** Command factories (also the palette/keyboard vocabulary). */
exports.commands = {
    addRung() {
        return { type: 'addRung', label: 'Add rung', rung: -1, branch: -1, index: -1 };
    },
    deleteRung(rung) {
        return { type: 'deleteRung', label: `Delete rung ${rung + 1}`, rung, branch: -1, index: -1 };
    },
    setRungComment(rung, comment) {
        return { type: 'setRungComment', label: 'Comment rung', rung, branch: -1, index: -1, comment };
    },
    addContact(rung, branch, name, negated) {
        return { type: 'addContact', label: `Add contact ${name}`, rung, branch, index: -1, name, negated };
    },
    insertParallelBranch(rung, contactName) {
        return {
            type: 'insertParallelBranch',
            label: 'Add parallel branch',
            rung,
            branch: -1,
            index: -1,
            name: contactName,
        };
    },
    addCoil(rung, name, variant) {
        return { type: 'addCoil', label: `Add coil ${name}`, rung, branch: -1, index: -1, name, variant };
    },
    addBlock(rung, output) {
        return { type: 'addBlock', label: `Add ${output.fb_type}`, rung, branch: -1, index: -1, output };
    },
    deleteElement(rung, branch, index) {
        return { type: 'deleteElement', label: 'Delete element', rung, branch, index };
    },
    toggleNegate(rung, branch, index) {
        return { type: 'toggleNegate', label: 'Toggle contact type', rung, branch, index };
    },
    renameVariable(rung, branch, index, name) {
        return { type: 'renameVariable', label: `Rename to ${name}`, rung, branch, index, name };
    },
    setCoilVariant(rung, outputIndex, variant) {
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
    replaceProgram(program) {
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
function paletteCommands(program, paletteType) {
    const elementCommand = (rung) => {
        switch (paletteType) {
            case 'no-contact':
            case 'nc-contact': {
                const branches = program.rungs[rung]?.branches.length ?? 0;
                return exports.commands.addContact(rung, Math.max(branches - 1, 0), 'NewVar', paletteType === 'nc-contact');
            }
            case 'coil':
            case 'set-coil':
            case 'reset-coil':
                return exports.commands.addCoil(rung, 'OutVar', paletteType === 'coil' ? 'normal' : paletteType === 'set-coil' ? 'set' : 'reset');
            case 'ton':
                return exports.commands.addBlock(rung, {
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
                return exports.commands.addBlock(rung, {
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
        return element ? [exports.commands.addRung(), element] : [];
    }
    const element = elementCommand(program.rungs.length - 1);
    return element ? [element] : [];
}
/** Apply a command to a program; never mutates the input. */
function applyCommand(program, command, history) {
    const next = clone(program);
    mutate(next, command);
    if (history) {
        history.push({ label: command.label, before: program, after: next });
    }
    return next;
}
/** Snapshot-based undo/redo over command entries. */
class CommandHistory {
    undoStack = [];
    redoStack = [];
    get undoDepth() {
        return this.undoStack.length;
    }
    get redoDepth() {
        return this.redoStack.length;
    }
    /** Undo the last entry; `current` is returned unchanged when empty. */
    undo(current) {
        const entry = this.undoStack.pop();
        if (!entry) {
            return current;
        }
        this.redoStack.push(entry);
        return entry.before;
    }
    /** Redo the last undone entry. */
    redo(current) {
        const entry = this.redoStack.pop();
        if (!entry) {
            return current;
        }
        this.undoStack.push(entry);
        return entry.after;
    }
    /** Record an applied entry and clear the redo tail (editor semantics). */
    push(entry) {
        this.undoStack.push(entry);
        this.redoStack.length = 0;
    }
    /** Drop all history (document reverted/reloaded). */
    clear() {
        this.undoStack.length = 0;
        this.redoStack.length = 0;
    }
}
exports.CommandHistory = CommandHistory;
function mutate(program, command) {
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
                rung.comment = command.comment;
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
                name: command.name,
                negated: Boolean(command.negated),
            });
            return;
        }
        case 'insertParallelBranch': {
            if (!rung) {
                return;
            }
            rung.branches.push({
                elements: [{ name: command.name, negated: false }],
            });
            return;
        }
        case 'addCoil': {
            if (!rung) {
                return;
            }
            rung.outputs.push({
                kind: 'coil',
                name: command.name,
                variant: command.variant,
            });
            return;
        }
        case 'addBlock': {
            if (!rung) {
                return;
            }
            rung.outputs.push(clone(command.output));
            return;
        }
        case 'deleteElement': {
            if (!rung) {
                return;
            }
            if (command.branch === -1) {
                rung.outputs.splice(command.index, 1);
            }
            else {
                rung.branches[command.branch]?.elements.splice(command.index, 1);
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
                contact.name = command.name;
                return;
            }
            const output = command.branch === -1 ? rung?.outputs[command.index] : undefined;
            if (output && output.kind === 'coil') {
                output.name = command.name;
            }
            return;
        }
        case 'setCoilVariant': {
            const output = rung?.outputs[command.index];
            if (output && output.kind === 'coil') {
                output.variant = command.variant;
            }
            return;
        }
        case 'replaceProgram': {
            const replacement = command.program;
            program.name = replacement.name;
            program.schema_version = replacement.schema_version;
            program.rungs = clone(replacement.rungs);
            return;
        }
        default:
            return;
    }
}
//# sourceMappingURL=commands.js.map