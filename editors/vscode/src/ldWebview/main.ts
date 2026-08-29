/**
 * LD webview UI glue: renders SVG and forwards user edits to the extension
 * host as typed commands. The host owns the document and its undo history
 * (PLC-111); the webview never mutates state it cannot undo. DOM-heavy by
 * design — everything testable lives in the pure sibling modules. Bundled
 * by esbuild (excluded from tsc; no vscode import).
 */

import { parseHostMessage, WebviewToHost } from './protocol';
import { LdProgram, normalizeIds, parseProgram, serializeProgram } from './model';
import { LdCommand, commands } from './commands';
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

/** Palette of LD elements. */
const ELEMENT_PALETTE = [
  { type: 'no-contact', label: '| |', title: 'Normally-Open Contact' },
  { type: 'nc-contact', label: '|/|', title: 'Normally-Closed Contact' },
  { type: 'coil', label: '( )', title: 'Coil (Normal)' },
  { type: 'set-coil', label: '(S)', title: 'SET Coil' },
  { type: 'reset-coil', label: '(R)', title: 'RESET Coil' },
  { type: 'ton', label: 'TON', title: 'Timer On Delay' },
  { type: 'ctu', label: 'CTU', title: 'Count Up' },
];

/** Palette actions map onto domain commands (one undo step each). */
function paletteCommand(type: string): LdCommand | undefined {
  const rung = program.rungs.length === 0 ? 0 : program.rungs.length - 1;
  if (program.rungs.length === 0) {
    // Commands address existing rungs; create one first (two undo steps).
    return commands.addRung();
  }
  switch (type) {
    case 'no-contact':
    case 'nc-contact': {
      const branch = Math.max(program.rungs[rung].branches.length - 1, 0);
      return commands.addContact(rung, branch, 'NewVar', type === 'nc-contact');
    }
    case 'coil':
    case 'set-coil':
    case 'reset-coil':
      return commands.addCoil(
        rung,
        'OutVar',
        type === 'coil' ? 'normal' : type === 'set-coil' ? 'set' : 'reset',
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
}

function send(command: LdCommand): void {
  vscode.postMessage({ type: 'edit', command });
}

function sendReplace(next: LdProgram): void {
  vscode.postMessage({ type: 'modelChanged', program: next });
}

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
      beginRename(rung, branch, index, node as SVGElement);
    });
  });
}

/**
 * Inline rename: an overlay input over the element (PLC-111 replaces the
 * old prompt() dialogs). Enter commits a rename command; Escape cancels.
 * Positioning uses style attributes (allowed for styles by the CSP).
 */
function beginRename(rung: number, branch: number, index: number, node: SVGElement): void {
  const existing = document.getElementById('rename-input');
  if (existing) {
    existing.remove();
    return;
  }
  const contact =
    branch === -1 ? undefined : program.rungs[rung]?.branches[branch]?.elements[index];
  const output = branch === -1 ? program.rungs[rung]?.outputs[index] : undefined;
  if (contact) {
    // Contacts rename directly.
  } else if (output && output.kind === 'coil') {
    // Coils rename directly.
  } else {
    return; // blocks (and missing elements) rename in a later task
  }
  const currentName = contact ? contact.name : (output as { name: string }).name;
  const box = (node as unknown as SVGGraphicsElement).getBBox();
  const input = document.createElement('input');
  input.id = 'rename-input';
  input.className = 'rename-input';
  input.value = currentName;
  input.style.left = `${box.x}px`;
  input.style.top = `${box.y}px`;
  input.style.width = `${Math.max(box.width, 90)}px`;
  const container = byId('canvas-container');
  container.style.position = 'relative';
  container.appendChild(input);
  input.focus();
  input.select();

  const commit = (): void => {
    const name = input.value.trim();
    input.remove();
    if (name.length > 0 && name !== currentName) {
      send(commands.renameVariable(rung, branch, index, name));
    }
  };
  input.addEventListener('keydown', (event) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      commit();
    } else if (event.key === 'Escape') {
      input.remove();
    }
  });
  input.addEventListener('blur', commit);
}

function wire(): void {
  const palette = byId('palette');
  for (const item of ELEMENT_PALETTE) {
    const node = document.createElement('div');
    node.className = 'palette-item';
    node.title = item.title;
    node.textContent = item.label;
    node.addEventListener('click', () => {
      const command = paletteCommand(item.type);
      if (command) {
        send(command);
        // When the rung was just created, follow up with the actual element.
        if (command.type === 'addRung') {
          const followUp = paletteCommand(item.type);
          if (followUp) {
            send(followUp);
          }
        }
      }
    });
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
      const next = parseProgram((event.target as HTMLTextAreaElement).value);
      byId('ld-canvas').innerHTML = renderSvg(layout(next), next, powerFlow);
      program = next;
      updateStatus();
    } catch {
      // Ignore parse errors while typing.
    }
  });

  byId('ld-textarea').addEventListener('change', (event) => {
    // Committing the JSON edit pushes it through the document as one
    // undoable replacement.
    try {
      sendReplace(parseProgram((event.target as HTMLTextAreaElement).value));
    } catch {
      // Leave the document untouched on invalid JSON.
    }
  });

  // Undo/redo: VS Code handles them when focus is outside the webview;
  // forward them when it is inside.
  window.addEventListener('keydown', (event) => {
    const meta = event.metaKey || event.ctrlKey;
    if (!meta || event.key !== 'z') {
      return;
    }
    event.preventDefault();
    if (event.shiftKey) {
      vscode.postMessage({ type: 'redo' });
    } else {
      vscode.postMessage({ type: 'undo' });
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
