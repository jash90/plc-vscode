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

import { LdProgram } from './model';
import { LayoutGeometry, PositionedElement, WireSegment } from './layout';

export interface PowerFlowRung {
  contact_energized?: boolean[][];
  branch_energized?: boolean[];
  output_energized?: boolean[];
  rung_result?: boolean;
}

export interface PowerFlow {
  rungs?: PowerFlowRung[];
}

function escapeXml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function contactEnergized(flow: PowerFlow | undefined, element: PositionedElement): boolean {
  const rung = flow?.rungs?.[element.rung];
  if (!rung?.contact_energized) {
    return false;
  }
  return rung.contact_energized[element.branch]?.[element.index] === true;
}

function outputEnergized(flow: PowerFlow | undefined, element: PositionedElement): boolean {
  const rung = flow?.rungs?.[element.rung];
  if (!rung?.output_energized) {
    return false;
  }
  return rung.output_energized[element.index] === true;
}

/** Render the diagram as a standalone `<svg>…</svg>` string. */
export function renderSvg(
  geometry: LayoutGeometry,
  program: LdProgram,
  flow?: PowerFlow,
): string {
  const parts: string[] = [];
  const width = Math.ceil(geometry.width);
  const height = Math.ceil(geometry.height);

  parts.push(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}" class="ld-diagram">`,
  );

  // Wires. Classes carry the kind (geometry) and the carrier (what the wire
  // conducts) so coloring is testable and stylable.
  for (const wire of geometry.wires) {
    const energized = wireEnergized(wire, flow);
    parts.push(
      `<line x1="${wire.x1}" y1="${wire.y1}" x2="${wire.x2}" y2="${wire.y2}"` +
        ` class="wire wire-${wire.kind} wire-${wire.carrier.type}${energized ? ' energized' : ''}" />`,
    );
  }

  // Elements.
  for (const element of geometry.elements) {
    const energized =
      element.kind === 'contact'
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
    parts.push(
      `<g class="${classes}"${idAttr} data-rung="${element.rung}" data-branch="${element.branch}" data-index="${element.index}">` +
        `<rect x="${element.x}" y="${element.y}" width="${element.width}" height="${element.height}" rx="3" />` +
        `<text class="element-label" x="${element.x + element.width / 2}" y="${labelY}" text-anchor="middle">${escapeXml(element.label)}</text>` +
        sublabel +
        `</g>`,
    );
  }

  // Rung comments, right-aligned past the rail.
  for (let r = 0; r < program.rungs.length; r += 1) {
    const comment = program.rungs[r].comment;
    if (!comment) {
      continue;
    }
    const band = geometry.elements.filter((e) => e.rung === r);
    const minY = band.length > 0 ? Math.min(...band.map((e) => e.y)) : r * 40;
    parts.push(
      `<text class="rung-comment" x="${geometry.leftRailX + 4}" y="${Math.max(minY - 4, 12)}">${escapeXml(comment)}</text>`,
    );
  }

  parts.push('</svg>');
  return parts.join('\n');
}

/**
 * Wire coloring by carrier — physical conduction rules, no geometry
 * heuristics: the supply side (left rail, collector, feeds) is live whenever
 * flow exists; segments between contacts follow cumulative contact energy;
 * branch ends follow `branch_energized`; output feeds follow `rung_result`;
 * the return side follows `output_energized`.
 */
function wireEnergized(wire: WireSegment, flow: PowerFlow | undefined): boolean {
  if (!flow?.rungs) {
    return false;
  }
  const rungFlow = flow.rungs[wire.rung];
  if (!rungFlow) {
    return false;
  }
  switch (wire.carrier.type) {
    case 'source':
      return true;
    case 'contact':
      return rungFlow.contact_energized?.[wire.carrier.branch]?.[wire.carrier.after] === true;
    case 'branch':
      return rungFlow.branch_energized?.[wire.carrier.branch] === true;
    case 'rung':
      return rungFlow.rung_result === true;
    case 'output':
      return rungFlow.output_energized?.[wire.carrier.index] === true;
    case 'return':
      return (rungFlow.output_energized ?? []).some(Boolean);
    default:
      return false;
  }
}
