/**
 * LD webview UI glue: renders SVG and forwards user edits to the extension
 * host as typed commands. The host owns the document and its undo history
 * (PLC-111); the webview never mutates state it cannot undo. DOM-heavy by
 * design — everything testable lives in the pure sibling modules. Bundled
 * by esbuild (excluded from tsc; no vscode import).
 */

import { parseHostMessage, WebviewToHost } from './protocol';
import { LdProgram, allVariables, normalizeIds, parseProgram, serializeProgram } from './model';
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

/** Latest simulation snapshot (PLC-114). */
let simState: { scan: number; timeMs: number; watch: string[]; forced: string[] } | undefined;

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

/** Keep the selection only while it still points at a real element. */
function revalidateSelection(): void {
  if (!selection) {
    return;
  }
  const exists = elementOrder().some(
    (e) =>
      e.rung === selection!.rung &&
      e.branch === selection!.branch &&
      e.index === selection!.index,
  );
  if (!exists) {
    selection = undefined;
  }
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
    // Element drag via pointer events — SVG elements cannot initiate
    // HTML5 drag-and-drop in Chromium, so we drive it ourselves.
    node.addEventListener('pointerdown', (event) => {
      if ((event as PointerEvent).button !== 0) {
        return;
      }
      beginPointerDrag(node as SVGElement, event as PointerEvent);
    });
  });
}

/**
 * Pointer-based element dragging: pointerdown selects, a >4px move arms the
 * drag (visual cue via CSS), pointerup drop-targets through hitTest.
 */
function beginPointerDrag(node: SVGElement, event: PointerEvent): void {
  const source = {
    rung: Number(node.getAttribute('data-rung')),
    branch: Number(node.getAttribute('data-branch')),
    index: Number(node.getAttribute('data-index')),
  };
  const startX = event.clientX;
  const startY = event.clientY;
  let armed = false;

  const move = (moveEvent: PointerEvent): void => {
    if (!armed && Math.hypot(moveEvent.clientX - startX, moveEvent.clientY - startY) > 4) {
      armed = true;
      node.classList.add('dragging');
    }
    if (armed) {
      moveEvent.preventDefault();
    }
  };
  const up = (upEvent: PointerEvent): void => {
    window.removeEventListener('pointermove', move);
    window.removeEventListener('pointerup', up);
    node.classList.remove('dragging');
    if (!armed) {
      return; // plain click — selection already handled
    }
    const canvas = byId('ld-canvas');
    const rect = canvas.getBoundingClientRect();
    const geometry = layout(program);
    const hit = hitTest(geometry, upEvent.clientX - rect.left, upEvent.clientY - rect.top);
    applyDrop(hit, undefined, source);
  };
  window.addEventListener('pointermove', move);
  window.addEventListener('pointerup', up);
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
    // Moving an existing element. A drop resolving to the element's own
    // position is a no-op — never push a phantom undo entry for it.
    const samePosition =
      (hit.kind === 'series' &&
        elementSource.rung === hit.rung &&
        elementSource.branch === hit.branch &&
        elementSource.index === hit.index) ||
      (hit.kind === 'output' &&
        elementSource.branch === -1 &&
        elementSource.rung === hit.rung &&
        elementSource.index === hit.index);
    if (samePosition) {
      return;
    }
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
  // Contact payloads insert contacts; coils/blocks land in the outputs of
  // the hit rung — regardless of the exact zone, so the payload type and
  // the drop rung are always honored.
  const isContact = payloadType === 'no-contact' || payloadType === 'nc-contact';
  if (isContact) {
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
    } else {
      // On/around an element or the output column: insert into the hit
      // rung's first branch.
      send(commands.insertContact(hit.rung, 0, 0, 'NewVar', payloadType === 'nc-contact'));
    }
    return;
  }
  // Coil / block payloads go to the outputs of the hit rung.
  if (hit.kind === 'newRung') {
    for (const command of paletteCommands(program, payloadType)) {
      send(command);
    }
    return;
  }
  const rung = hit.kind === 'element' || hit.kind === 'output' || hit.kind === 'parallel'
    ? hit.rung
    : program.rungs.length - 1;
  if (payloadType === 'coil' || payloadType === 'set-coil' || payloadType === 'reset-coil') {
    send(
      commands.addCoil(
        rung,
        'OutVar',
        payloadType === 'coil' ? 'normal' : payloadType === 'set-coil' ? 'set' : 'reset',
      ),
    );
  } else {
    for (const command of paletteCommands({ ...program, rungs: program.rungs.slice(0, rung + 1) }, payloadType)) {
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
    const payloadType = data.getData('application/x-ld-palette') || undefined;
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

  byId('btn-sim-run').addEventListener('click', () => {
    vscode.postMessage({ type: 'simStart' });
  });
  byId('btn-sim-pause').addEventListener('click', () => {
    vscode.postMessage({ type: 'simStop' });
  });
  byId('btn-sim-step').addEventListener('click', () => {
    vscode.postMessage({ type: 'simStep' });
  });
  byId('btn-sim-reset').addEventListener('click', () => {
    vscode.postMessage({ type: 'simReset' });
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
        selection = undefined;
        render();
        break;
      case 'state':
        program = message.program;
        revalidateSelection();
        render();
        break;
      case 'simState':
        simState = {
          scan: message.scan,
          timeMs: message.timeMs,
          watch: message.watch,
          forced: message.forced,
        };
        renderSimPanel();
        updateStatus();
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

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

/** Boolean watch values by variable name (TRUE/FALSE on the wire). */
function watchBooleans(): Map<string, boolean> {
  const map = new Map<string, boolean>();
  if (!simState) {
    return map;
  }
  for (const line of simState.watch) {
    const separator = line.indexOf('=');
    if (separator === -1) {
      continue;
    }
    const name = line.slice(0, separator).trim();
    const value = line.slice(separator + 1).trim().toUpperCase();
    if (value === 'TRUE' || value === 'FALSE') {
      map.set(name, value === 'TRUE');
    }
  }
  return map;
}

/**
 * The simulation panel. The inputs section is built once per MODEL (its
 * variable set only changes on edits); the watch table and header update
 * on every state event. Checkboxes derive their checked state from the
 * watch values, so the controls never contradict the runtime.
 */
function renderSimPanel(): void {
  const panel = document.getElementById('sim-panel');
  if (!panel || !simState) {
    return;
  }
  const forcedSet = new Set(simState.forced.map((name) => name.toLowerCase()));
  const watchValues = watchBooleans();
  const inputSignature = JSON.stringify(allVariables(program));
  if (panel.getAttribute('data-inputs') !== inputSignature) {
    panel.setAttribute('data-inputs', inputSignature);
    const inputs = allVariables(program).slice(0, 12);
    panel.innerHTML = `
      <div class="sim-header"></div>
      <div class="sim-inputs">
        ${inputs
          .map(
            (name) =>
              `<label class="sim-input"><input type="checkbox" data-var="${escapeHtml(name)}" /> ${escapeHtml(name)}</label>`,
          )
          .join('')}
      </div>
      <table class="sim-watch">
        <thead><tr><th>variable</th><th>value</th><th></th></tr></thead>
        <tbody></tbody>
      </table>`;
    panel.querySelectorAll<HTMLInputElement>('input[type="checkbox"]').forEach((box) => {
      box.addEventListener('change', () => {
        vscode.postMessage({
          type: 'simInput',
          name: box.getAttribute('data-var') ?? '',
          value: box.checked,
        });
      });
    });
  }

  const header = panel.querySelector<HTMLElement>('.sim-header');
  if (header) {
    header.textContent = `scan ${simState.scan} · t=${simState.timeMs}ms`;
  }

  // Checkboxes mirror the runtime state (not the click) — no contradictions.
  panel.querySelectorAll<HTMLInputElement>('input[type="checkbox"]').forEach((box) => {
    const name = box.getAttribute('data-var') ?? '';
    const value = watchValues.get(name);
    if (value !== undefined) {
      box.checked = value;
    }
  });

  const tbody = panel.querySelector('.sim-watch tbody');
  if (tbody) {
    const rows = simState.watch
      .map((line) => {
        const separator = line.indexOf('=');
        const name = separator === -1 ? line : line.slice(0, separator).trim();
        const value = separator === -1 ? '' : line.slice(separator + 1).trim();
        const forced = forcedSet.has(name.toLowerCase());
        const forcedMark = forced ? '⚠' : '';
        const rowClass = forced ? ' class="forced"' : '';
        return `<tr${rowClass}><td>${escapeHtml(name)}</td><td>${escapeHtml(value)}</td><td>${forcedMark}</td></tr>`;
      })
      .join('');
    tbody.innerHTML = rows;
  }
}
