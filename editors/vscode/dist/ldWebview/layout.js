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
    const outputsX = contactX(maxContacts) + (maxContacts > 0 ? cell.gapX : cell.gapX);
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
        const bandMid = bandTop + bandHeight / 2;
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
        wires.push({ x1: leftRailX, y1: bandTop, x2: leftRailX, y2: bandTop + bandHeight, kind: 'rail' });
        wires.push({ x1: rightRailX, y1: bandTop, x2: rightRailX, y2: bandTop + bandHeight, kind: 'rail' });
        const firstBranchMid = branchMid(0);
        const lastBranchMid = branchMid(branchCount - 1);
        const collectorX = leftRailX + cell.railPadX / 2;
        const teeX = outputsX - cell.gapX / 2;
        // Left rail → collector → branch starts.
        wires.push({ x1: leftRailX, y1: firstBranchMid, x2: collectorX, y2: firstBranchMid, kind: 'series' });
        if (rung.branches.length > 1) {
            wires.push({
                x1: collectorX,
                y1: firstBranchMid,
                x2: collectorX,
                y2: lastBranchMid,
                kind: 'collector',
            });
        }
        for (let b = 0; b < rung.branches.length; b += 1) {
            wires.push({ x1: collectorX, y1: branchMid(b), x2: contactX(0), y2: branchMid(b), kind: 'series' });
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
            wires.push({ x1: branchEndX, y1: branchMid(b), x2: teeX, y2: branchMid(b), kind: 'series' });
        }
        // Right tee: branch ends join into the output column.
        if (rung.branches.length > 1) {
            wires.push({ x1: teeX, y1: firstBranchMid, x2: teeX, y2: lastBranchMid, kind: 'tee' });
        }
        // Outputs (vertically centered in the band).
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
            wires.push({ x1: teeX, y1: firstBranchMid, x2: outputsX, y2: outputMid(o), kind: 'tee' });
            wires.push({
                x1: outputsX + size.width,
                y1: outputMid(o),
                x2: rightRailX,
                y2: outputMid(o),
                kind: 'series',
            });
        }
        // Rung comment sits in the band's slack space, if any.
        void bandMid;
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
//# sourceMappingURL=layout.js.map