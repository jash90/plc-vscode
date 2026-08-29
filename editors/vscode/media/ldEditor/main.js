"use strict";
(() => {
  // src/ldWebview/protocol.ts
  function asRecord(value) {
    if (typeof value === "object" && value !== null && !Array.isArray(value)) {
      return value;
    }
    return null;
  }
  function parseHostMessage(value) {
    const record = asRecord(value);
    switch (record?.type) {
      case "load":
        return { type: "load", text: String(record.text) };
      case "state":
        return { type: "state", program: record.program };
      case "powerFlow":
        return { type: "powerFlow", json: String(record.json) };
      case "error":
        return { type: "error", message: String(record.message) };
      default:
        throw new Error(`unknown host message: ${JSON.stringify(value)}`);
    }
  }

  // src/ldWebview/model.ts
  var CURRENT_SCHEMA_VERSION = 2;
  function parseProgram(text) {
    const raw = JSON.parse(text);
    const program2 = {
      name: typeof raw.name === "string" ? raw.name : "NewProgram",
      schema_version: typeof raw.schema_version === "number" ? raw.schema_version : CURRENT_SCHEMA_VERSION,
      rungs: Array.isArray(raw.rungs) ? raw.rungs : []
    };
    for (const rung of program2.rungs) {
      rung.branches = Array.isArray(rung.branches) ? rung.branches : [];
      rung.outputs = Array.isArray(rung.outputs) ? rung.outputs : [];
      for (const branch of rung.branches) {
        branch.elements = Array.isArray(branch.elements) ? branch.elements : [];
      }
    }
    return program2;
  }
  var IdAllocator = class {
    constructor(prefix, next) {
      this.prefix = prefix;
      this.next = next;
    }
    prefix;
    used = /* @__PURE__ */ new Set();
    next;
    ensure(slot) {
      const keep = typeof slot.id === "string" && slot.id.length > this.prefix.length && slot.id.startsWith(this.prefix) && !this.used.has(slot.id) ? (this.used.add(slot.id), true) : false;
      if (!keep) {
        slot.id = this.allocate();
      }
    }
    allocate() {
      for (; ; ) {
        const candidate = `${this.prefix}${this.next}`;
        this.next += 1;
        if (!this.used.has(candidate)) {
          this.used.add(candidate);
          return candidate;
        }
      }
    }
  };
  function normalizeIds(program2) {
    const rungs = new IdAllocator("r", seed(program2, "r"));
    const elements = new IdAllocator("e", seed(program2, "e"));
    for (const rung of program2.rungs) {
      rungs.ensure(rung);
      for (const branch of rung.branches) {
        for (const contact of branch.elements) {
          elements.ensure(contact);
        }
      }
      for (const output of rung.outputs) {
        elements.ensure(output);
      }
    }
  }
  function seed(program2, prefix) {
    let next = 0;
    const consider = (id) => {
      if (typeof id === "string" && id.startsWith(prefix)) {
        const suffix = Number(id.slice(prefix.length));
        if (Number.isInteger(suffix) && suffix >= next && suffix < Number.MAX_SAFE_INTEGER) {
          next = suffix + 1;
        }
      }
    };
    for (const rung of program2.rungs) {
      consider(rung.id);
      for (const branch of rung.branches) {
        for (const contact of branch.elements) {
          consider(contact.id);
        }
      }
      for (const output of rung.outputs) {
        consider(output.id);
      }
    }
    return next;
  }
  function serializeProgram(program2) {
    return `${JSON.stringify(program2, null, 2)}
`;
  }

  // src/ldWebview/layout.ts
  var CELL = {
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
    outerPad: 12
  };
  function contactLabel(contact) {
    return `${contact.negated ? "|/|" : "| |"} ${contact.name}`;
  }
  function coilLabel(coil) {
    const symbol = coil.variant === "set" ? "(S)" : coil.variant === "reset" ? "(R)" : "( )";
    return `${symbol} ${coil.name}`;
  }
  function outputSize(output, cell) {
    return output.kind === "block" ? { width: cell.blockW, height: cell.blockH } : { width: cell.coilW, height: cell.coilH };
  }
  function layout(program2, cell = CELL) {
    const leftRailX = cell.outerPad;
    const maxContacts = Math.max(0, ...program2.rungs.map((r) => Math.max(0, ...r.branches.map((b) => b.elements.length)), 0));
    const contactX = (column) => leftRailX + cell.railPadX + column * (cell.contactW + cell.gapX);
    const outputsX = contactX(maxContacts) + (maxContacts > 0 ? cell.gapX : cell.gapX);
    const maxOutputW = Math.max(
      cell.coilW,
      ...program2.rungs.flatMap((r) => r.outputs.map((o) => outputSize(o, cell).width))
    );
    const rightRailX = outputsX + maxOutputW + cell.railPadX;
    const elements = [];
    const wires = [];
    let y = cell.outerPad;
    for (let rungIndex = 0; rungIndex < program2.rungs.length; rungIndex += 1) {
      const rung = program2.rungs[rungIndex];
      const branchCount = Math.max(rung.branches.length, 1);
      const rowH = cell.contactH;
      const branchTop = (branch) => y + branch * (rowH + cell.branchGapY);
      const branchMid = (branch) => branchTop(branch) + rowH / 2;
      const outputsTotalH = rung.outputs.reduce(
        (sum, output, index) => sum + outputSize(output, cell).height + (index > 0 ? cell.branchGapY : 0),
        0
      );
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
      wires.push({ x1: leftRailX, y1: bandTop, x2: leftRailX, y2: bandTop + bandHeight, kind: "rail" });
      wires.push({ x1: rightRailX, y1: bandTop, x2: rightRailX, y2: bandTop + bandHeight, kind: "rail" });
      const firstBranchMid = branchMid(0);
      const lastBranchMid = branchMid(branchCount - 1);
      const collectorX = leftRailX + cell.railPadX / 2;
      const teeX = outputsX - cell.gapX / 2;
      wires.push({ x1: leftRailX, y1: firstBranchMid, x2: collectorX, y2: firstBranchMid, kind: "series" });
      if (rung.branches.length > 1) {
        wires.push({
          x1: collectorX,
          y1: firstBranchMid,
          x2: collectorX,
          y2: lastBranchMid,
          kind: "collector"
        });
      }
      for (let b = 0; b < rung.branches.length; b += 1) {
        wires.push({ x1: collectorX, y1: branchMid(b), x2: contactX(0), y2: branchMid(b), kind: "series" });
      }
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
              kind: "series"
            });
          }
          elements.push({
            id: contact.id,
            rung: rungIndex,
            branch: b,
            index: i,
            kind: "contact",
            x: contactX(i),
            y: branchTop(b),
            width: cell.contactW,
            height: rowH,
            label: contactLabel(contact)
          });
        }
        const branchEndX = branch.elements.length > 0 ? contactX(branch.elements.length - 1) + cell.contactW : contactX(0);
        wires.push({ x1: branchEndX, y1: branchMid(b), x2: teeX, y2: branchMid(b), kind: "series" });
      }
      if (rung.branches.length > 1) {
        wires.push({ x1: teeX, y1: firstBranchMid, x2: teeX, y2: lastBranchMid, kind: "tee" });
      }
      for (let o = 0; o < rung.outputs.length; o += 1) {
        const output = rung.outputs[o];
        const size = outputSize(output, cell);
        elements.push({
          id: output.id,
          rung: rungIndex,
          branch: -1,
          index: o,
          kind: output.kind === "block" ? "block" : "coil",
          x: outputsX,
          y: outputY(o),
          width: size.width,
          height: size.height,
          label: output.kind === "block" ? output.fb_type : coilLabel(output),
          sublabel: output.kind === "block" ? output.instance : void 0
        });
        wires.push({ x1: teeX, y1: firstBranchMid, x2: outputsX, y2: outputMid(o), kind: "tee" });
        wires.push({
          x1: outputsX + size.width,
          y1: outputMid(o),
          x2: rightRailX,
          y2: outputMid(o),
          kind: "series"
        });
      }
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
      wires
    };
  }

  // src/ldWebview/render.ts
  function escapeXml(text) {
    return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
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
  function renderSvg(geometry, program2, flow) {
    const parts = [];
    const width = Math.ceil(geometry.width);
    const height = Math.ceil(geometry.height);
    parts.push(
      `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}" class="ld-diagram">`
    );
    for (const wire2 of geometry.wires) {
      const energized = wireEnergized(wire2, geometry, flow);
      parts.push(
        `<line x1="${wire2.x1}" y1="${wire2.y1}" x2="${wire2.x2}" y2="${wire2.y2}" class="wire wire-${wire2.kind}${energized ? " energized" : ""}" />`
      );
    }
    for (const element of geometry.elements) {
      const energized = element.kind === "contact" ? contactEnergized(flow, element) : outputEnergized(flow, element);
      const classes = `element element-${element.kind}${energized ? " energized" : " not-energized"}`;
      const idAttr = element.id ? ` data-id="${escapeXml(element.id)}"` : "";
      const labelY = element.sublabel ? element.y + element.height / 2 - 2 : element.y + element.height / 2 + 4;
      const sublabel = element.sublabel ? `<text class="element-sublabel" x="${element.x + element.width / 2}" y="${element.y + element.height / 2 + 13}" text-anchor="middle">${escapeXml(element.sublabel)}</text>` : "";
      parts.push(
        `<g class="${classes}"${idAttr} data-rung="${element.rung}" data-branch="${element.branch}" data-index="${element.index}"><rect x="${element.x}" y="${element.y}" width="${element.width}" height="${element.height}" rx="3" /><text class="element-label" x="${element.x + element.width / 2}" y="${labelY}" text-anchor="middle">${escapeXml(element.label)}</text>` + sublabel + `</g>`
      );
    }
    for (let r = 0; r < program2.rungs.length; r += 1) {
      const comment = program2.rungs[r].comment;
      if (!comment) {
        continue;
      }
      const band = geometry.elements.filter((e) => e.rung === r);
      const minY = band.length > 0 ? Math.min(...band.map((e) => e.y)) : r * 40;
      parts.push(
        `<text class="rung-comment" x="${geometry.leftRailX + 4}" y="${Math.max(minY - 4, 12)}">${escapeXml(comment)}</text>`
      );
    }
    parts.push("</svg>");
    return parts.join("\n");
  }
  function wireEnergized(wire2, geometry, flow) {
    if (wire2.kind === "rail") {
      const midY = (wire2.y1 + wire2.y2) / 2;
      const band = geometry.elements.filter(
        (e) => e.y <= midY && e.y + e.height >= wire2.y1
      );
      return band.some((element) => isElementEnergized(element, flow));
    }
    if (wire2.kind === "series") {
      const left = geometry.elements.filter((e) => e.kind === "contact").filter((e) => Math.abs(e.y + e.height / 2 - wire2.y1) < 0.5 && e.x + e.width <= wire2.x1 + 1).sort((a, b) => b.x - a.x)[0];
      return left ? isElementEnergized(left, flow) : false;
    }
    const branches = geometry.elements.filter((e) => e.kind === "contact" && e.index === 0).filter((e) => e.y + e.height / 2 >= Math.min(wire2.y1, wire2.y2) - 0.5).filter((e) => e.y + e.height / 2 <= Math.max(wire2.y1, wire2.y2) + 0.5);
    return branches.some((element) => isElementEnergized(element, flow));
  }
  function isElementEnergized(element, flow) {
    return element.kind === "contact" ? contactEnergized(flow, element) : outputEnergized(flow, element);
  }

  // src/ldWebview/main.ts
  var vscode = acquireVsCodeApi();
  var program = { name: "NewProgram", schema_version: 2, rungs: [] };
  var powerFlow;
  function byId(id) {
    const element = document.getElementById(id);
    if (!element) {
      throw new Error(`missing #${id}`);
    }
    return element;
  }
  var ELEMENT_PALETTE = [
    { type: "no-contact", label: "| |", title: "Normally-Open Contact" },
    { type: "nc-contact", label: "|/|", title: "Normally-Closed Contact" },
    { type: "coil", label: "( )", title: "Coil (Normal)" },
    { type: "set-coil", label: "(S)", title: "SET Coil" },
    { type: "reset-coil", label: "(R)", title: "RESET Coil" },
    { type: "ton", label: "TON", title: "Timer On Delay" },
    { type: "ctu", label: "CTU", title: "Count Up" }
  ];
  function render() {
    normalizeIds(program);
    byId("ld-canvas").innerHTML = renderSvg(layout(program), program, powerFlow);
    bindElementClicks();
    updateStatus();
    syncTextarea();
  }
  function updateStatus() {
    const status = byId("status-bar");
    if (powerFlow?.rungs) {
      const energized = powerFlow.rungs.filter((r) => r.rung_result).length;
      status.textContent = `${program.rungs.length} rungs, ${energized} energized.`;
    } else {
      status.textContent = `${program.rungs.length} rungs. Save to evaluate power-flow.`;
    }
  }
  function syncTextarea() {
    const textarea = byId("ld-textarea");
    if (document.activeElement !== textarea) {
      textarea.value = serializeProgram(program).trimEnd();
    }
  }
  function bindElementClicks() {
    document.querySelectorAll("#ld-canvas .element").forEach((node) => {
      node.addEventListener("click", () => {
        const rung = Number(node.getAttribute("data-rung"));
        const branch = Number(node.getAttribute("data-branch"));
        const index = Number(node.getAttribute("data-index"));
        renameElement(rung, branch, index);
      });
    });
  }
  function renameElement(rung, branch, index) {
    const output = program.rungs[rung]?.outputs[index];
    if (output && branch === -1) {
      if (output.kind === "coil") {
        const name = window.prompt("Variable name:", output.name);
        if (name !== null && name.trim().length > 0) {
          output.name = name.trim();
          modelChanged();
        }
      }
      return;
    }
    const contact = program.rungs[rung]?.branches[branch]?.elements[index];
    if (contact) {
      const name = window.prompt("Variable name:", contact.name);
      if (name !== null && name.trim().length > 0) {
        contact.name = name.trim();
        modelChanged();
      }
    }
  }
  function modelChanged() {
    render();
    vscode.postMessage({ type: "modelChanged", program });
  }
  function addElement(type) {
    if (program.rungs.length === 0) {
      program.rungs.push({ branches: [{ elements: [] }], outputs: [] });
    }
    const last = program.rungs[program.rungs.length - 1];
    switch (type) {
      case "no-contact":
      case "nc-contact":
        if (last.branches.length === 0) {
          last.branches.push({ elements: [] });
        }
        last.branches[0].elements.push({ name: "NewVar", negated: type === "nc-contact" });
        break;
      case "coil":
      case "set-coil":
      case "reset-coil":
        last.outputs.push({
          kind: "coil",
          name: "OutVar",
          variant: type === "coil" ? "normal" : type === "set-coil" ? "set" : "reset"
        });
        break;
      case "ton":
      case "ctu": {
        const isTon = type === "ton";
        last.outputs.push({
          kind: "block",
          fb_type: isTon ? "TON" : "CTU",
          instance: isTon ? "TON_inst" : "CTU_inst",
          inputs: isTon ? [{ name: "IN", value: "NewVar" }, { name: "PT", value: "T#1s" }] : [{ name: "CU", value: "NewVar" }, { name: "PV", value: "10" }],
          outputs: [{ name: "Q", value: "Done" }]
        });
        break;
      }
      default:
        return;
    }
    modelChanged();
  }
  function wire() {
    const palette = byId("palette");
    for (const item of ELEMENT_PALETTE) {
      const node = document.createElement("div");
      node.className = "palette-item";
      node.title = item.title;
      node.textContent = item.label;
      node.addEventListener("click", () => addElement(item.type));
      palette.appendChild(node);
    }
    byId("btn-save").addEventListener("click", () => {
      const textarea = byId("ld-textarea");
      const text = textarea.style.display !== "none" ? textarea.value : serializeProgram(program);
      vscode.postMessage({ type: "save", text });
    });
    byId("btn-run").addEventListener("click", () => {
      vscode.postMessage({ type: "run" });
    });
    byId("btn-toggle-json").addEventListener("click", () => {
      const textarea = byId("ld-textarea");
      textarea.style.display = textarea.style.display === "none" ? "block" : "none";
      if (textarea.style.display !== "none") {
        textarea.value = serializeProgram(program);
      }
    });
    byId("ld-textarea").addEventListener("input", (event) => {
      try {
        program = parseProgram(event.target.value);
        byId("ld-canvas").innerHTML = renderSvg(layout(program), program, powerFlow);
        updateStatus();
      } catch {
      }
    });
    window.addEventListener("message", (event) => {
      const message = parseHostMessage(event.data);
      switch (message.type) {
        case "load":
          try {
            program = parseProgram(message.text);
          } catch {
            program = { name: "NewProgram", schema_version: 2, rungs: [] };
          }
          powerFlow = void 0;
          render();
          break;
        case "state":
          program = message.program;
          render();
          break;
        case "powerFlow":
          try {
            powerFlow = JSON.parse(message.json);
          } catch {
            byId("status-bar").textContent = "Power-flow parse error.";
            return;
          }
          byId("ld-canvas").innerHTML = renderSvg(layout(program), program, powerFlow);
          updateStatus();
          break;
        case "error":
          byId("status-bar").textContent = message.message;
          break;
      }
    });
    vscode.postMessage({ type: "ready" });
  }
  wire();
})();
