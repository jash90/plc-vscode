"use strict";
/**
 * SVG string rendering of a laid-out LD program. Pure: builds a string, no
 * DOM, so it is unit-testable in Node.
 *
 * Energization rules (from `plc ld --watch` power-flow JSON):
 * - A contact is energized when `contact_energized[branch][index]` is true
 *   (cumulative energy to the right of that contact).
 * - A coil/block output is energized when `output_energized[index]` is true.
 * - Wires between two energized points carry the `energized` class; the
 *   classes exist alongside shapes, never as the only signal.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.renderSvg = renderSvg;
function escapeXml(text) {
    return text
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;');
}
function contactEnergized(flow, element) {
    const rung = flow?.rungs?.[element.rung];
    if (!rung?.contact_energized) {
        return false;
    }
    return rung.contact_energized[element.branch]?.[element.index] === true;
}
function outputEnergized(flow, element) {
    const rung = flow?.rungs?.[element.rung];
    if (!rung?.output_energized) {
        return false;
    }
    return rung.output_energized[element.index] === true;
}
/** Render the diagram as a standalone `<svg>…</svg>` string. */
function renderSvg(geometry, program, flow) {
    const parts = [];
    const width = Math.ceil(geometry.width);
    const height = Math.ceil(geometry.height);
    parts.push(`<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}" class="ld-diagram">`);
    // Wires.
    for (const wire of geometry.wires) {
        const energized = wireEnergized(wire, geometry, flow);
        parts.push(`<line x1="${wire.x1}" y1="${wire.y1}" x2="${wire.x2}" y2="${wire.y2}"` +
            ` class="wire wire-${wire.kind}${energized ? ' energized' : ''}" />`);
    }
    // Elements.
    for (const element of geometry.elements) {
        const energized = element.kind === 'contact'
            ? contactEnergized(flow, element)
            : outputEnergized(flow, element);
        const classes = `element element-${element.kind}${energized ? ' energized' : ' not-energized'}`;
        const idAttr = element.id ? ` data-id="${escapeXml(element.id)}"` : '';
        const labelY = element.sublabel
            ? element.y + element.height / 2 - 2
            : element.y + element.height / 2 + 4;
        const sublabel = element.sublabel
            ? `<text class="element-sublabel" x="${element.x + element.width / 2}" y="${element.y + element.height / 2 + 13}" text-anchor="middle">${escapeXml(element.sublabel)}</text>`
            : '';
        parts.push(`<g class="${classes}"${idAttr} data-rung="${element.rung}" data-branch="${element.branch}" data-index="${element.index}">` +
            `<rect x="${element.x}" y="${element.y}" width="${element.width}" height="${element.height}" rx="3" />` +
            `<text class="element-label" x="${element.x + element.width / 2}" y="${labelY}" text-anchor="middle">${escapeXml(element.label)}</text>` +
            sublabel +
            `</g>`);
    }
    // Rung comments, right-aligned past the rail.
    for (let r = 0; r < program.rungs.length; r += 1) {
        const comment = program.rungs[r].comment;
        if (!comment) {
            continue;
        }
        const band = geometry.elements.filter((e) => e.rung === r);
        const minY = band.length > 0 ? Math.min(...band.map((e) => e.y)) : r * 40;
        parts.push(`<text class="rung-comment" x="${geometry.leftRailX + 4}" y="${Math.max(minY - 4, 12)}">${escapeXml(comment)}</text>`);
    }
    parts.push('</svg>');
    return parts.join('\n');
}
function wireEnergized(wire, geometry, flow) {
    if (wire.kind === 'rail') {
        // Rails are live when any element on the rung band is energized.
        const midY = (wire.y1 + wire.y2) / 2;
        const band = geometry.elements.filter((e) => e.y <= midY && e.y + e.height >= wire.y1);
        return band.some((element) => isElementEnergized(element, flow));
    }
    if (wire.kind === 'series') {
        // A series wire is live when the element immediately left of it passes
        // power: find the closest contact ending at the wire's start.
        const left = geometry.elements
            .filter((e) => e.kind === 'contact')
            .filter((e) => Math.abs(e.y + e.height / 2 - wire.y1) < 0.5 && e.x + e.width <= wire.x1 + 1)
            .sort((a, b) => b.x - a.x)[0];
        return left ? isElementEnergized(left, flow) : false;
    }
    // collector/tee wires follow the branches they join.
    const branches = geometry.elements
        .filter((e) => e.kind === 'contact' && e.index === 0)
        .filter((e) => e.y + e.height / 2 >= Math.min(wire.y1, wire.y2) - 0.5)
        .filter((e) => e.y + e.height / 2 <= Math.max(wire.y1, wire.y2) + 0.5);
    return branches.some((element) => isElementEnergized(element, flow));
}
function isElementEnergized(element, flow) {
    return element.kind === 'contact'
        ? contactEnergized(flow, element)
        : outputEnergized(flow, element);
}
//# sourceMappingURL=render.js.map