"use strict";
/**
 * Pure layout for the LD diagram: positions every element and wire of an
 * [`LdProgram`] on an absolute-coordinate grid. No DOM — SVG-free so it is
 * unit-testable in Node.
 *
 * Structure:
 *
 * - Left and right rails are global (aligned across all rungs), like a real
 *   ladder diagram. Each rung occupies a horizontal band between them.
 * - Parallel (OR) branches stack vertically inside the band; series contacts
 *   share a column grid so the OR-join wires line up.
 * - A vertical **collector** wire on the left joins the branch starts; a
 *   vertical **tee** on the right joins the branch ends into the outputs.
 * - Outputs sit in their own column before the right rail; blocks are taller
 *   than coils to fit the instance sublabel.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.CELL = void 0;
exports.layout = layout;
exports.hitTest = hitTest;
exports.CELL = {
    contactW: 72,
    contactH: 28,
    blockW: 84,
    blockH: 60,
    coilW: 64,
    coilH: 28,
    gapX: 16,
    branchGapY: 10,
    rungGapY: 22,
    railPadX: 26,
    outerPad: 12,
};
function contactLabel(contact) {
    return `${contact.negated ? '|/|' : '| |'} ${contact.name}`;
}
function coilLabel(coil) {
    const symbol = coil.variant === 'set' ? '(S)' : coil.variant === 'reset' ? '(R)' : '( )';
    return `${symbol} ${coil.name}`;
}
function outputSize(output, cell) {
    return output.kind === 'block'
        ? { width: cell.blockW, height: cell.blockH }
        : { width: cell.coilW, height: cell.coilH };
}
/** Compute the absolute layout of a program. Deterministic for a program. */
function layout(program, cell = exports.CELL) {
    const leftRailX = cell.outerPad;
    // Pass 1: per-rung band extents → the global right rail x.
    const maxContacts = Math.max(0, ...program.rungs.map((r) => Math.max(0, ...r.branches.map((b) => b.elements.length)), 0));
    const contactX = (column) => leftRailX + cell.railPadX + column * (cell.contactW + cell.gapX);
    const outputsX = contactX(maxContacts) + cell.gapX;
    const maxOutputW = Math.max(cell.coilW, ...program.rungs.flatMap((r) => r.outputs.map((o) => outputSize(o, cell).width)));
    const rightRailX = outputsX + maxOutputW + cell.railPadX;
    const elements = [];
    const wires = [];
    let y = cell.outerPad;
    for (let rungIndex = 0; rungIndex < program.rungs.length; rungIndex += 1) {
        const rung = program.rungs[rungIndex];
        const branchCount = Math.max(rung.branches.length, 1);
        const rowH = cell.contactH;
        const branchTop = (branch) => y + branch * (rowH + cell.branchGapY);
        const branchMid = (branch) => branchTop(branch) + rowH / 2;
        const outputsTotalH = rung.outputs.reduce((sum, output, index) => sum + outputSize(output, cell).height + (index > 0 ? cell.branchGapY : 0), 0);
        const branchesTotalH = branchCount * rowH + (branchCount - 1) * cell.branchGapY;
        const bandHeight = Math.max(branchesTotalH, outputsTotalH);
        const bandTop = y;
        const outputsTop = bandTop + Math.max(0, (bandHeight - outputsTotalH) / 2);
        const outputY = (index) => {
            let oy = outputsTop;
            for (let i = 0; i < index; i += 1) {
                oy += outputSize(rung.outputs[i], cell).height + cell.branchGapY;
            }
            return oy;
        };
        const outputMid = (index) => {
            const size = outputSize(rung.outputs[index], cell);
            return outputY(index) + size.height / 2;
        };
        // Rails (per-band segments of the global rails).
        wires.push({
            x1: leftRailX, y1: bandTop, x2: leftRailX, y2: bandTop + bandHeight,
            kind: 'rail', carrier: { type: 'source' },
            rung: rungIndex,
        });
        wires.push({
            x1: rightRailX, y1: bandTop, x2: rightRailX, y2: bandTop + bandHeight,
            kind: 'rail', carrier: { type: 'return' },
            rung: rungIndex,
        });
        const firstBranchMid = branchMid(0);
        const lastBranchMid = branchMid(branchCount - 1);
        const collectorX = leftRailX + cell.railPadX / 2;
        const teeX = outputsX - cell.gapX / 2;
        // Left rail → collector → branch starts.
        wires.push({
            x1: leftRailX, y1: firstBranchMid, x2: collectorX, y2: firstBranchMid,
            kind: 'series', carrier: { type: 'source' },
            rung: rungIndex,
        });
        if (rung.branches.length > 1) {
            wires.push({
                x1: collectorX, y1: firstBranchMid, x2: collectorX, y2: lastBranchMid,
                kind: 'collector', carrier: { type: 'source' },
                rung: rungIndex,
            });
        }
        for (let b = 0; b < rung.branches.length; b += 1) {
            wires.push({
                x1: collectorX, y1: branchMid(b), x2: contactX(0), y2: branchMid(b),
                kind: 'series', carrier: { type: 'source' },
                rung: rungIndex,
            });
        }
        // Rung with outputs but no branches: a straight source→output feed.
        if (rung.branches.length === 0 && rung.outputs.length > 0) {
            wires.push({
                x1: leftRailX, y1: branchMid(0), x2: teeX, y2: branchMid(0),
                kind: 'series', carrier: { type: 'source' },
                rung: rungIndex,
            });
        }
        // Contacts, column-aligned across branches, with series wires between.
        for (let b = 0; b < rung.branches.length; b += 1) {
            const branch = rung.branches[b];
            for (let i = 0; i < branch.elements.length; i += 1) {
                const contact = branch.elements[i];
                if (i > 0) {
                    wires.push({
                        x1: contactX(i - 1) + cell.contactW,
                        y1: branchMid(b),
                        x2: contactX(i),
                        y2: branchMid(b),
                        kind: 'series',
                        carrier: { type: 'contact', branch: b, after: i - 1 },
                        rung: rungIndex,
                    });
                }
                elements.push({
                    id: contact.id,
                    rung: rungIndex,
                    branch: b,
                    index: i,
                    kind: 'contact',
                    x: contactX(i),
                    y: branchTop(b),
                    width: cell.contactW,
                    height: rowH,
                    label: contactLabel(contact),
                });
            }
            // Trailing wire from the branch end to the tee column.
            const branchEndX = branch.elements.length > 0
                ? contactX(branch.elements.length - 1) + cell.contactW
                : contactX(0);
            wires.push({
                x1: branchEndX, y1: branchMid(b), x2: teeX, y2: branchMid(b),
                kind: 'series', carrier: { type: 'branch', branch: b },
                rung: rungIndex,
            });
        }
        // Right tee: branch ends join into the output column.
        if (rung.branches.length > 1) {
            wires.push({
                x1: teeX, y1: firstBranchMid, x2: teeX, y2: lastBranchMid,
                kind: 'tee', carrier: { type: 'rung' },
                rung: rungIndex,
            });
        }
        // Outputs (vertically centered in the band), fed orthogonally from the tee.
        for (let o = 0; o < rung.outputs.length; o += 1) {
            const output = rung.outputs[o];
            const size = outputSize(output, cell);
            elements.push({
                id: output.id,
                rung: rungIndex,
                branch: -1,
                index: o,
                kind: output.kind === 'block' ? 'block' : 'coil',
                x: outputsX,
                y: outputY(o),
                width: size.width,
                height: size.height,
                label: output.kind === 'block' ? output.fb_type : coilLabel(output),
                sublabel: output.kind === 'block' ? output.instance : undefined,
            });
            // Elbow: vertical at the tee column, then horizontal into the output.
            if (Math.abs(outputMid(o) - firstBranchMid) > 0.5) {
                wires.push({
                    x1: teeX, y1: firstBranchMid, x2: teeX, y2: outputMid(o),
                    kind: 'tee', carrier: { type: 'rung' },
                    rung: rungIndex,
                });
            }
            wires.push({
                x1: teeX, y1: outputMid(o), x2: outputsX, y2: outputMid(o),
                kind: 'tee', carrier: { type: 'rung' },
                rung: rungIndex,
            });
            wires.push({
                x1: outputsX + size.width, y1: outputMid(o), x2: rightRailX, y2: outputMid(o),
                kind: 'series', carrier: { type: 'output', index: o },
                rung: rungIndex,
            });
        }
        y += bandHeight + cell.rungGapY;
    }
    const height = Math.max(y - cell.rungGapY + cell.outerPad, cell.outerPad * 2);
    return {
        width: rightRailX + cell.outerPad,
        height,
        leftRailX,
        rightRailX,
        elements,
        wires,
    };
}
/**
 * Map a canvas point to an insertion location for a drop or a keyboard
 * cursor. Pure geometry over the computed layout:
 *
 * - on an element body → move/inspect target (`element`)
 * - between two contacts of a branch (the wire row) → `series` insertion
 * - below a rung's contact band but within its vertical slack → `parallel`
 * - on an output slot → `output`
 * - past the last rung → `newRung`
 */
function hitTest(geometry, x, y) {
    // 1. Element bodies first (they sit on the wire rows but are narrower).
    for (const element of geometry.elements) {
        if (x >= element.x &&
            x <= element.x + element.width &&
            y >= element.y &&
            y <= element.y + element.height) {
            return {
                kind: 'element',
                id: element.id ?? `${element.rung}:${element.branch}:${element.index}`,
                rung: element.rung,
                branch: element.branch,
                index: element.index,
            };
        }
    }
    // 2. Below everything → a fresh rung.
    if (y > geometry.height) {
        return { kind: 'newRung' };
    }
    // 3. Determine the rung band from element positions.
    const rungs = [...new Set(geometry.elements.map((e) => e.rung))].sort((a, b) => a - b);
    if (rungs.length === 0) {
        return { kind: 'newRung' };
    }
    let rung = rungs[rungs.length - 1];
    for (const candidate of rungs) {
        const band = geometry.elements.filter((e) => e.rung === candidate);
        const top = Math.min(...band.map((e) => e.y));
        const bottom = Math.max(...band.map((e) => e.y + e.height));
        if (y >= top - 14 && y <= bottom + 24) {
            rung = candidate;
            break;
        }
    }
    const band = geometry.elements.filter((e) => e.rung === rung);
    const contacts = band.filter((e) => e.kind === 'contact');
    const outputs = band.filter((e) => e.kind !== 'contact');
    const bandTop = Math.min(...band.map((e) => e.y));
    const bandBottom = Math.max(...band.map((e) => e.y + e.height));
    // 4. Output column → output slot.
    if (outputs.length > 0) {
        const outputLeft = Math.min(...outputs.map((e) => e.x));
        const outputRight = Math.max(...outputs.map((e) => e.x + e.width));
        if (x >= outputLeft - 8 && x <= outputRight + 8 && y >= bandTop - 8 && y <= bandBottom + 8) {
            let index = outputs.length;
            for (let i = 0; i < outputs.length; i += 1) {
                if (y < outputs[i].y + outputs[i].height / 2) {
                    index = i;
                    break;
                }
            }
            return { kind: 'output', rung, index };
        }
    }
    // 5. Below the branch rows but within the band slack → parallel branch,
    //    unless the drop is in the output column (a new output slot).
    const branchRows = [...new Set(contacts.map((e) => e.branch))].sort((a, b) => a - b);
    const rowsBottom = contacts.length
        ? Math.max(...contacts.map((e) => e.y + e.height))
        : bandTop;
    if (y > rowsBottom && y <= bandBottom + 24) {
        const inOutputColumn = outputs.length > 0
            ? x >= Math.min(...outputs.map((e) => e.x)) - 8
            : false;
        if (inOutputColumn) {
            return { kind: 'output', rung, index: outputs.length };
        }
        return { kind: 'parallel', rung };
    }
    // 6. Between contacts of the closest branch row → series insertion.
    if (contacts.length > 0 && branchRows.length > 0) {
        let branch = branchRows[0];
        for (const candidate of branchRows) {
            const row = contacts.filter((e) => e.branch === candidate);
            const top = Math.min(...row.map((e) => e.y));
            const bottom = Math.max(...row.map((e) => e.y + e.height));
            if (y >= top - 6 && y <= bottom + 6) {
                branch = candidate;
                break;
            }
        }
        const row = contacts
            .filter((e) => e.branch === branch)
            .sort((a, b) => a.index - b.index);
        let index = row.length;
        for (let i = 0; i < row.length; i += 1) {
            if (x < row[i].x + row[i].width / 2) {
                index = i;
                break;
            }
        }
        return { kind: 'series', rung, branch, index };
    }
    return { kind: 'parallel', rung };
}
//# sourceMappingURL=layout.js.map