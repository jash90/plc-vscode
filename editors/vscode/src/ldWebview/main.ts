/**
 * LD webview UI glue: owns the in-memory program, renders SVG, and talks to
 * the extension host exclusively through the typed protocol. DOM-heavy by
 * design — everything testable lives in the pure sibling modules. Bundled by
 * esbuild (excluded from tsc; no vscode import).
 */

import { parseHostMessage, WebviewToHost } from './protocol';
import {
  LdProgram,
  normalizeIds,
  parseProgram,
  serializeProgram,
} from './model';
import { layout } from './layout';
import { PowerFlow, renderSvg } from './render';

declare function acquireVsCodeApi(): {
  postMessage(message: WebviewToHost): void;
};

const vscode = acquireVsCodeApi();

let program: LdProgram = { name: 'NewProgram', schema_version: 2, rungs: [] };
let powerFlow: PowerFlow | undefined;

function byId<T extends HTMLElement>(id: string): T {
  const element = document.getElementById(id);
  if (!element) {
    throw new Error(`missing #${id}`);
  }
  return element as T;
}

/** Palette of LD elements (kept identical to the PLC-105 editor). */
const ELEMENT_PALETTE = [
  { type: 'no-contact', label: '| |', title: 'Normally-Open Contact' },
  { type: 'nc-contact', label: '|/|', title: 'Normally-Closed Contact' },
  { type: 'coil', label: '( )', title: 'Coil (Normal)' },
  { type: 'set-coil', label: '(S)', title: 'SET Coil' },
  { type: 'reset-coil', label: '(R)', title: 'RESET Coil' },
  { type: 'ton', label: 'TON', title: 'Timer On Delay' },
  { type: 'ctu', label: 'CTU', title: 'Count Up' },
];

function render(): void {
  normalizeIds(program);
  byId('ld-canvas').innerHTML = renderSvg(layout(program), program, powerFlow);
  bindElementClicks();
  updateStatus();
  syncTextarea();
}

function updateStatus(): void {
  const status = byId('status-bar');
  if (powerFlow?.rungs) {
    const energized = powerFlow.rungs.filter((r) => r.rung_result).length;
    status.textContent = `${program.rungs.length} rungs, ${energized} energized.`;
  } else {
    status.textContent = `${program.rungs.length} rungs. Save to evaluate power-flow.`;
  }
}

function syncTextarea(): void {
  const textarea = byId<HTMLTextAreaElement>('ld-textarea');
  if (document.activeElement !== textarea) {
    textarea.value = serializeProgram(program).trimEnd();
  }
}

function bindElementClicks(): void {
  document.querySelectorAll('#ld-canvas .element').forEach((node) => {
    node.addEventListener('click', () => {
      const rung = Number(node.getAttribute('data-rung'));
      const branch = Number(node.getAttribute('data-branch'));
      const index = Number(node.getAttribute('data-index'));
      renameElement(rung, branch, index);
    });
  });
}

function renameElement(rung: number, branch: number, index: number): void {
  // Feature parity with the PLC-105 editor: rename via prompt(). The
  // document-lifecycle task (PLC-111) replaces this with inline editing.
  const output = program.rungs[rung]?.outputs[index];
  if (output && branch === -1) {
    if (output.kind === 'coil') {
      const name = window.prompt('Variable name:', output.name);
      if (name !== null && name.trim().length > 0) {
        output.name = name.trim();
        modelChanged();
      }
    }
    return;
  }
  const contact = program.rungs[rung]?.branches[branch]?.elements[index];
  if (contact) {
    const name = window.prompt('Variable name:', contact.name);
    if (name !== null && name.trim().length > 0) {
      contact.name = name.trim();
      modelChanged();
    }
  }
}

function modelChanged(): void {
  render();
  vscode.postMessage({ type: 'modelChanged', program });
}

function addElement(type: string): void {
  if (program.rungs.length === 0) {
    program.rungs.push({ branches: [{ elements: [] }], outputs: [] });
  }
  const last = program.rungs[program.rungs.length - 1];

  switch (type) {
    case 'no-contact':
    case 'nc-contact':
      if (last.branches.length === 0) {
        last.branches.push({ elements: [] });
      }
      last.branches[0].elements.push({ name: 'NewVar', negated: type === 'nc-contact' });
      break;
    case 'coil':
    case 'set-coil':
    case 'reset-coil':
      last.outputs.push({
        kind: 'coil',
        name: 'OutVar',
        variant: type === 'coil' ? 'normal' : type === 'set-coil' ? 'set' : 'reset',
      });
      break;
    case 'ton':
    case 'ctu': {
      const isTon = type === 'ton';
      last.outputs.push({
        kind: 'block',
        fb_type: isTon ? 'TON' : 'CTU',
        instance: isTon ? 'TON_inst' : 'CTU_inst',
        inputs: isTon
          ? [{ name: 'IN', value: 'NewVar' }, { name: 'PT', value: 'T#1s' }]
          : [{ name: 'CU', value: 'NewVar' }, { name: 'PV', value: '10' }],
        outputs: [{ name: 'Q', value: 'Done' }],
      });
      break;
    }
    default:
      return;
  }
  modelChanged();
}

function wire(): void {
  const palette = byId('palette');
  for (const item of ELEMENT_PALETTE) {
    const node = document.createElement('div');
    node.className = 'palette-item';
    node.title = item.title;
    node.textContent = item.label;
    node.addEventListener('click', () => addElement(item.type));
    palette.appendChild(node);
  }

  byId('btn-save').addEventListener('click', () => {
    const textarea = byId<HTMLTextAreaElement>('ld-textarea');
    const text =
      textarea.style.display !== 'none'
        ? textarea.value
        : serializeProgram(program);
    vscode.postMessage({ type: 'save', text });
  });

  byId('btn-run').addEventListener('click', () => {
    vscode.postMessage({ type: 'run' });
  });

  byId('btn-toggle-json').addEventListener('click', () => {
    const textarea = byId<HTMLTextAreaElement>('ld-textarea');
    textarea.style.display = textarea.style.display === 'none' ? 'block' : 'none';
    if (textarea.style.display !== 'none') {
      textarea.value = serializeProgram(program);
    }
  });

  byId('ld-textarea').addEventListener('input', (event) => {
    try {
      program = parseProgram((event.target as HTMLTextAreaElement).value);
      byId('ld-canvas').innerHTML = renderSvg(layout(program), program, powerFlow);
      updateStatus();
    } catch {
      // Ignore parse errors while typing.
    }
  });

  window.addEventListener('message', (event: MessageEvent) => {
    const message = parseHostMessage(event.data);
    switch (message.type) {
      case 'load':
        try {
          program = parseProgram(message.text);
        } catch {
          program = { name: 'NewProgram', schema_version: 2, rungs: [] };
        }
        powerFlow = undefined;
        render();
        break;
      case 'state':
        program = message.program;
        render();
        break;
      case 'powerFlow':
        try {
          powerFlow = JSON.parse(message.json) as PowerFlow;
        } catch {
          byId('status-bar').textContent = 'Power-flow parse error.';
          return;
        }
        byId('ld-canvas').innerHTML = renderSvg(layout(program), program, powerFlow);
        updateStatus();
        break;
      case 'error':
        byId('status-bar').textContent = message.message;
        break;
    }
  });

  vscode.postMessage({ type: 'ready' });
}

wire();
