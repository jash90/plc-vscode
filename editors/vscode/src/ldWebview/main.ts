/**
 * LD webview UI glue: renders SVG and forwards user edits to the extension
 * host as typed commands. The host owns the document and its undo history
 * (PLC-111); the webview never mutates state it cannot undo. DOM-heavy by
 * design — everything testable lives in the pure sibling modules. Bundled
 * by esbuild (excluded from tsc; no vscode import).
 */

import { parseHostMessage, WebviewToHost } from './protocol';
import { LdProgram, normalizeIds, parseProgram, serializeProgram } from './model';
import { LdCommand, commands, paletteCommands } from './commands';
import { layout, hitTest } from './layout';
import { variables as completeVariables } from './completion';
import { PowerFlow, renderSvg } from './render';

declare function acquireVsCodeApi(): {
  postMessage(message: WebviewToHost): void;
};

const vscode = acquireVsCodeApi();

let program: LdProgram = { name: 'NewProgram', schema_version: 2, rungs: [] };
let powerFlow: PowerFlow | undefined;

/** Keyboard selection: (rung, branch, index); branch -1 = output. */
let selection: { rung: number; branch: number; index: number } | undefined;

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
  highlightSelection();
  updateStatus();
  syncTextarea();
}

function highlightSelection(): void {
  if (!selection) {
    return;
  }
  const node = document.querySelector(
    `#ld-canvas .element[data-rung="${selection.rung}"][data-branch="${selection.branch}"][data-index="${selection.index}"]`,
  );
  node?.classList.add('selected');
}

/** The element list in keyboard-navigation order (reading order). */
function elementOrder(): { rung: number; branch: number; index: number }[] {
  const order: { rung: number; branch: number; index: number }[] = [];
  program.rungs.forEach((rung, rungIndex) => {
    rung.branches.forEach((branch, branchIndex) => {
      branch.elements.forEach((_contact, index) => {
        order.push({ rung: rungIndex, branch: branchIndex, index });
      });
    });
    rung.outputs.forEach((_output, index) => {
      order.push({ rung: rungIndex, branch: -1, index });
    });
  });
  return order;
}

function moveSelection(delta: number): void {
  const order = elementOrder();
  if (order.length === 0) {
    return;
  }
  const current = selection
    ? order.findIndex(
        (e) =>
          e.rung === selection!.rung &&
          e.branch === selection!.branch &&
          e.index === selection!.index,
      )
    : -1;
  const next = current === -1 ? 0 : Math.min(Math.max(current + delta, 0), order.length - 1);
  selection = order[next];
  highlightSelection();
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
      selection = { rung, branch, index };
      highlightSelection();
    });
    node.addEventListener('dblclick', () => {
      const rung = Number(node.getAttribute('data-rung'));
      const branch = Number(node.getAttribute('data-branch'));
      const index = Number(node.getAttribute('data-index'));
      beginRename(rung, branch, index, node as SVGElement);
    });
    // Element drag: HTML5 DnD carrying the element address.
    const dragSource = node as unknown as SVGElement & { draggable?: boolean };
    dragSource.draggable = true;
    node.addEventListener('dragstart', (event) => {
      selection = {
        rung: Number(node.getAttribute('data-rung')),
        branch: Number(node.getAttribute('data-branch')),
        index: Number(node.getAttribute('data-index')),
      };
      (event as DragEvent).dataTransfer?.setData('application/x-ld-element', JSON.stringify(selection));
      (event as DragEvent).dataTransfer?.setData('text/plain', 'element');
    });
  });
}

/** Keyboard insert helpers addressed at the selection (or program end). */
function keyboardInsert(paletteType: string): void {
  for (const command of paletteCommands(program, paletteType)) {
    send(command);
  }
}

/** Apply a drop at a hit-tested location. */
function applyDrop(
  hit: ReturnType<typeof hitTest>,
  payloadType: string | undefined,
  elementSource: { rung: number; branch: number; index: number } | undefined,
): void {
  if (elementSource) {
    // Moving an existing element.
    if (hit.kind === 'series') {
      send(
        commands.moveElement(elementSource, {
          rung: hit.rung,
          branch: hit.branch,
          index: hit.index,
        }),
      );
    } else if (hit.kind === 'newRung') {
      send(commands.addRung());
      send(
        commands.moveElement(elementSource, {
          rung: program.rungs.length,
          branch: 0,
          index: 0,
        }),
      );
    }
    return;
  }
  if (!payloadType) {
    return;
  }
  // Palette drops map onto insertion commands.
  if (hit.kind === 'series') {
    send(
      commands.insertContact(
        hit.rung,
        hit.branch,
        hit.index,
        'NewVar',
        payloadType === 'nc-contact',
      ),
    );
  } else if (hit.kind === 'parallel') {
    send(commands.insertParallelBranch(hit.rung, 'NewVar'));
  } else if (hit.kind === 'newRung') {
    for (const command of paletteCommands(program, payloadType)) {
      send(command);
    }
  }
  // 'output' and 'element' targets fall back to append semantics via the
  // palette sequence (kept simple for MVP).
  if (hit.kind === 'output' || hit.kind === 'element') {
    for (const command of paletteCommands(program, payloadType)) {
      send(command);
    }
  }
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
  attachCompletion(input);
  input.focus();
  input.select();

  // One-shot guard: removing a focused input fires blur, which would
  // otherwise commit twice; Escape cancels without committing.
  let settled = false;
  const close = (commit: boolean): void => {
    if (settled) {
      return;
    }
    settled = true;
    const name = input.value.trim();
    input.remove();
    if (commit && name.length > 0 && name !== currentName) {
      send(commands.renameVariable(rung, branch, index, name));
    }
  };
  input.addEventListener('keydown', (event) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      close(true);
    } else if (event.key === 'Escape') {
      event.preventDefault();
      close(false);
    }
  });
  input.addEventListener('blur', () => close(true));
}

function wire(): void {
  const palette = byId('palette');
  for (const item of ELEMENT_PALETTE) {
    const node = document.createElement('div');
    node.className = 'palette-item';
    node.title = item.title;
    node.textContent = item.label;
    node.addEventListener('click', () => {
      // The whole sequence is computed up front against the current program
      // (pure paletteCommands) — a not-yet-synced local model cannot
      // corrupt the addressing of the follow-up command.
      for (const command of paletteCommands(program, item.type)) {
        send(command);
      }
    });
    // Drag from the palette onto the diagram.
    node.draggable = true;
    node.addEventListener('dragstart', (event) => {
      (event as DragEvent).dataTransfer?.setData('application/x-ld-palette', item.type);
      (event as DragEvent).dataTransfer?.setData('text/plain', item.type);
    });
    palette.appendChild(node);
  }

  // Drop targets: the diagram area.
  const canvasContainer = byId('canvas-container');
  canvasContainer.addEventListener('dragover', (event) => {
    event.preventDefault();
  });
  canvasContainer.addEventListener('drop', (event) => {
    event.preventDefault();
    const drag = event as DragEvent;
    const data = drag.dataTransfer;
    if (!data) {
      return;
    }
    const elementJson = data.getData('application/x-ld-element');
    const elementSource = elementJson
      ? (JSON.parse(elementJson) as { rung: number; branch: number; index: number })
      : undefined;
    const payloadType =
      data.getData('application/x-ld-palette') || (elementSource ? undefined : undefined);
    const canvas = byId('ld-canvas');
    const rect = canvas.getBoundingClientRect();
    const geometry = layout(program);
    const hit = hitTest(geometry, drag.clientX - rect.left, drag.clientY - rect.top);
    applyDrop(hit, payloadType || undefined, elementSource);
  });

  byId('btn-save').addEventListener('click', () => {
    // Clicking the button blurs the JSON textarea first, committing its
    // content via the change event; the host then routes through VS Code's
    // save flow, which serializes the document's canonical model.
    vscode.postMessage({ type: 'save' });
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
  // forward them when it is inside — but never hijack text editing (the
  // rename input and JSON textarea keep their native undo).
  window.addEventListener('keydown', (event) => {
    const target = event.target as HTMLElement | null;
    if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA')) {
      return;
    }
    const meta = event.metaKey || event.ctrlKey;
    if (meta && event.key.toLowerCase() === 'z') {
      event.preventDefault();
      vscode.postMessage({ type: event.shiftKey ? 'redo' : 'undo' });
      return;
    }
    if (meta) {
      return;
    }
    // Keyboard editing map (PLC-112). Ship only these keys.
    switch (event.key) {
      case 'ArrowDown':
      case 'ArrowRight':
        event.preventDefault();
        moveSelection(1);
        return;
      case 'ArrowUp':
      case 'ArrowLeft':
        event.preventDefault();
        moveSelection(-1);
        return;
      case '1':
        keyboardInsert('no-contact');
        return;
      case '2':
        keyboardInsert('nc-contact');
        return;
      case 'c':
        keyboardInsert('coil');
        return;
      case 's':
        keyboardInsert('set-coil');
        return;
      case 'r':
        keyboardInsert('reset-coil');
        return;
      case 't':
        keyboardInsert('ton');
        return;
      case 'b':
        if (selection) {
          send(commands.insertParallelBranch(selection.rung, 'NewVar'));
        }
        return;
      case 'Enter':
        if (selection) {
          const node = document.querySelector(
            `#ld-canvas .element[data-rung="${selection.rung}"][data-branch="${selection.branch}"][data-index="${selection.index}"]`,
          );
          if (node) {
            beginRename(selection.rung, selection.branch, selection.index, node as SVGElement);
          }
        }
        return;
      case 'Delete':
      case 'Backspace':
        if (selection) {
          event.preventDefault();
          send(commands.deleteElement(selection.rung, selection.branch, selection.index));
          selection = undefined;
        }
        return;
      default:
        return;
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

/** Completion dropdown for the rename input (PLC-112). */
function attachCompletion(input: HTMLInputElement): void {
  let list = document.getElementById('completion-list');
  if (list) {
    list.remove();
  }
  list = document.createElement('div');
  list.id = 'completion-list';
  list.className = 'completion-list';
  input.after(list);

  const refresh = (): void => {
    const hits = completeVariables(program, input.value);
    list!.innerHTML = '';
    for (const hit of hits.slice(0, 8)) {
      const item = document.createElement('div');
      item.className = 'completion-item';
      item.textContent = hit;
      item.addEventListener('mousedown', (event) => {
        event.preventDefault();
        input.value = hit;
        list!.innerHTML = '';
        input.focus();
      });
      list!.appendChild(item);
    }
  };

  input.addEventListener('input', refresh);
  input.addEventListener('blur', () => {
    // Remove slightly after blur so mousedown clicks land first.
    window.setTimeout(() => list!.remove(), 150);
  });
  refresh();
}
