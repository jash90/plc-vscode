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
      if (typeof id === "string" && id.length > prefix.length && id.startsWith(prefix) && /^\+?\d+$/.test(id.slice(prefix.length))) {
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

  // src/ldWebview/commands.ts
  var commands = {
    addRung() {
      return { type: "addRung", label: "Add rung", rung: -1, branch: -1, index: -1 };
    },
    deleteRung(rung) {
      return { type: "deleteRung", label: `Delete rung ${rung + 1}`, rung, branch: -1, index: -1 };
    },
    setRungComment(rung, comment) {
      return { type: "setRungComment", label: "Comment rung", rung, branch: -1, index: -1, comment };
    },
    addContact(rung, branch, name, negated) {
      return { type: "addContact", label: `Add contact ${name}`, rung, branch, index: -1, name, negated };
    },
    /** Insert a contact at a specific position in a branch (drop target). */
    insertContact(rung, branch, index, name, negated) {
      return { type: "insertContact", label: `Insert contact ${name}`, rung, branch, index, name, negated };
    },
    /** Move an element to a series position (reorder/branch change). */
    moveElement(from, to) {
      return {
        type: "moveElement",
        label: "Move element",
        rung: from.rung,
        branch: from.branch,
        index: from.index,
        toRung: to.rung,
        toBranch: to.branch,
        toIndex: to.index
      };
    },
    insertParallelBranch(rung, contactName) {
      return {
        type: "insertParallelBranch",
        label: "Add parallel branch",
        rung,
        branch: -1,
        index: -1,
        name: contactName
      };
    },
    addCoil(rung, name, variant) {
      return { type: "addCoil", label: `Add coil ${name}`, rung, branch: -1, index: -1, name, variant };
    },
    addBlock(rung, output) {
      return { type: "addBlock", label: `Add ${output.fb_type}`, rung, branch: -1, index: -1, output };
    },
    deleteElement(rung, branch, index) {
      return { type: "deleteElement", label: "Delete element", rung, branch, index };
    },
    toggleNegate(rung, branch, index) {
      return { type: "toggleNegate", label: "Toggle contact type", rung, branch, index };
    },
    renameVariable(rung, branch, index, name) {
      return { type: "renameVariable", label: `Rename to ${name}`, rung, branch, index, name };
    },
    setCoilVariant(rung, outputIndex, variant) {
      return {
        type: "setCoilVariant",
        label: `Coil \u2192 (${variant})`,
        rung,
        branch: -1,
        index: outputIndex,
        variant
      };
    },
    /** Wholesale model replacement (the JSON textarea path), one undo step. */
    replaceProgram(program2) {
      return {
        type: "replaceProgram",
        label: "Edit JSON",
        rung: -1,
        branch: -1,
        index: -1,
        program: program2
      };
    }
  };
  function paletteCommands(program2, paletteType) {
    const elementCommand = (rung) => {
      switch (paletteType) {
        case "no-contact":
        case "nc-contact": {
          const branches = program2.rungs[rung]?.branches.length ?? 0;
          return commands.addContact(rung, Math.max(branches - 1, 0), "NewVar", paletteType === "nc-contact");
        }
        case "coil":
        case "set-coil":
        case "reset-coil":
          return commands.addCoil(
            rung,
            "OutVar",
            paletteType === "coil" ? "normal" : paletteType === "set-coil" ? "set" : "reset"
          );
        case "ton":
          return commands.addBlock(rung, {
            kind: "block",
            fb_type: "TON",
            instance: "TON_inst",
            inputs: [
              { name: "IN", value: "NewVar" },
              { name: "PT", value: "T#1s" }
            ],
            outputs: [{ name: "Q", value: "Done" }]
          });
        case "ctu":
          return commands.addBlock(rung, {
            kind: "block",
            fb_type: "CTU",
            instance: "CTU_inst",
            inputs: [
              { name: "CU", value: "NewVar" },
              { name: "PV", value: "10" }
            ],
            outputs: [{ name: "Q", value: "Done" }]
          });
        default:
          return void 0;
      }
    };
    if (program2.rungs.length === 0) {
      const element2 = elementCommand(0);
      return element2 ? [commands.addRung(), element2] : [];
    }
    const element = elementCommand(program2.rungs.length - 1);
    return element ? [element] : [];
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
    const outputsX = contactX(maxContacts) + cell.gapX;
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
      wires.push({
        x1: leftRailX,
        y1: bandTop,
        x2: leftRailX,
        y2: bandTop + bandHeight,
        kind: "rail",
        carrier: { type: "source" },
        rung: rungIndex
      });
      wires.push({
        x1: rightRailX,
        y1: bandTop,
        x2: rightRailX,
        y2: bandTop + bandHeight,
        kind: "rail",
        carrier: { type: "return" },
        rung: rungIndex
      });
      const firstBranchMid = branchMid(0);
      const lastBranchMid = branchMid(branchCount - 1);
      const collectorX = leftRailX + cell.railPadX / 2;
      const teeX = outputsX - cell.gapX / 2;
      wires.push({
        x1: leftRailX,
        y1: firstBranchMid,
        x2: collectorX,
        y2: firstBranchMid,
        kind: "series",
        carrier: { type: "source" },
        rung: rungIndex
      });
      if (rung.branches.length > 1) {
        wires.push({
          x1: collectorX,
          y1: firstBranchMid,
          x2: collectorX,
          y2: lastBranchMid,
          kind: "collector",
          carrier: { type: "source" },
          rung: rungIndex
        });
      }
      for (let b = 0; b < rung.branches.length; b += 1) {
        wires.push({
          x1: collectorX,
          y1: branchMid(b),
          x2: contactX(0),
          y2: branchMid(b),
          kind: "series",
          carrier: { type: "source" },
          rung: rungIndex
        });
      }
      if (rung.branches.length === 0 && rung.outputs.length > 0) {
        wires.push({
          x1: leftRailX,
          y1: branchMid(0),
          x2: teeX,
          y2: branchMid(0),
          kind: "series",
          carrier: { type: "source" },
          rung: rungIndex
        });
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
              kind: "series",
              carrier: { type: "contact", branch: b, after: i - 1 },
              rung: rungIndex
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
        wires.push({
          x1: branchEndX,
          y1: branchMid(b),
          x2: teeX,
          y2: branchMid(b),
          kind: "series",
          carrier: { type: "branch", branch: b },
          rung: rungIndex
        });
      }
      if (rung.branches.length > 1) {
        wires.push({
          x1: teeX,
          y1: firstBranchMid,
          x2: teeX,
          y2: lastBranchMid,
          kind: "tee",
          carrier: { type: "rung" },
          rung: rungIndex
        });
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
        if (Math.abs(outputMid(o) - firstBranchMid) > 0.5) {
          wires.push({
            x1: teeX,
            y1: firstBranchMid,
            x2: teeX,
            y2: outputMid(o),
            kind: "tee",
            carrier: { type: "rung" },
            rung: rungIndex
          });
        }
        wires.push({
          x1: teeX,
          y1: outputMid(o),
          x2: outputsX,
          y2: outputMid(o),
          kind: "tee",
          carrier: { type: "rung" },
          rung: rungIndex
        });
        wires.push({
          x1: outputsX + size.width,
          y1: outputMid(o),
          x2: rightRailX,
          y2: outputMid(o),
          kind: "series",
          carrier: { type: "output", index: o },
          rung: rungIndex
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
      wires
    };
  }
  function hitTest(geometry, x, y) {
    for (const element of geometry.elements) {
      if (x >= element.x && x <= element.x + element.width && y >= element.y && y <= element.y + element.height) {
        return {
          kind: "element",
          id: element.id ?? `${element.rung}:${element.branch}:${element.index}`,
          rung: element.rung,
          branch: element.branch,
          index: element.index
        };
      }
    }
    if (y > geometry.height) {
      return { kind: "newRung" };
    }
    const rungs = [...new Set(geometry.elements.map((e) => e.rung))].sort((a, b) => a - b);
    if (rungs.length === 0) {
      return { kind: "newRung" };
    }
    let rung = rungs[rungs.length - 1];
    for (const candidate of rungs) {
      const band2 = geometry.elements.filter((e) => e.rung === candidate);
      const top = Math.min(...band2.map((e) => e.y));
      const bottom = Math.max(...band2.map((e) => e.y + e.height));
      if (y >= top - 14 && y <= bottom + 24) {
        rung = candidate;
        break;
      }
    }
    const band = geometry.elements.filter((e) => e.rung === rung);
    const contacts = band.filter((e) => e.kind === "contact");
    const outputs = band.filter((e) => e.kind !== "contact");
    const bandTop = Math.min(...band.map((e) => e.y));
    const bandBottom = Math.max(...band.map((e) => e.y + e.height));
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
        return { kind: "output", rung, index };
      }
    }
    const branchRows = [...new Set(contacts.map((e) => e.branch))].sort((a, b) => a - b);
    const rowsBottom = contacts.length ? Math.max(...contacts.map((e) => e.y + e.height)) : bandTop;
    if (y > rowsBottom && y <= bandBottom + 24) {
      const inOutputColumn = outputs.length > 0 ? x >= Math.min(...outputs.map((e) => e.x)) - 8 : false;
      if (inOutputColumn) {
        return { kind: "output", rung, index: outputs.length };
      }
      return { kind: "parallel", rung };
    }
    if (contacts.length > 0 && branchRows.length > 0) {
      let branch = branchRows[0];
      for (const candidate of branchRows) {
        const row2 = contacts.filter((e) => e.branch === candidate);
        const top = Math.min(...row2.map((e) => e.y));
        const bottom = Math.max(...row2.map((e) => e.y + e.height));
        if (y >= top - 6 && y <= bottom + 6) {
          branch = candidate;
          break;
        }
      }
      const row = contacts.filter((e) => e.branch === branch).sort((a, b) => a.index - b.index);
      let index = row.length;
      for (let i = 0; i < row.length; i += 1) {
        if (x < row[i].x + row[i].width / 2) {
          index = i;
          break;
        }
      }
      return { kind: "series", rung, branch, index };
    }
    return { kind: "parallel", rung };
  }

  // src/ldWebview/completion.ts
  function variables(program2, prefix) {
    const needle = prefix.trim().toLowerCase();
    if (needle.length === 0) {
      return [];
    }
    const seen = /* @__PURE__ */ new Set();
    const result = [];
    for (const name of collectVariables(program2)) {
      const lower = name.toLowerCase();
      if (lower.startsWith(needle) && lower !== needle && !seen.has(name)) {
        seen.add(name);
        result.push(name);
      }
    }
    return result;
  }
  function collectVariables(program2) {
    const names = [];
    for (const rung of program2.rungs) {
      for (const branch of rung.branches) {
        for (const contact of branch.elements) {
          names.push(contact.name);
        }
      }
      for (const output of rung.outputs) {
        if (output.kind === "coil") {
          names.push(output.name);
        } else {
          for (const arg of [...output.inputs, ...output.outputs]) {
            names.push(arg.value);
          }
        }
      }
    }
    return names;
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
      const energized = wireEnergized(wire2, flow);
      parts.push(
        `<line x1="${wire2.x1}" y1="${wire2.y1}" x2="${wire2.x2}" y2="${wire2.y2}" class="wire wire-${wire2.kind} wire-${wire2.carrier.type}${energized ? " energized" : ""}" />`
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
  function wireEnergized(wire2, flow) {
    if (!flow?.rungs) {
      return false;
    }
    const rungFlow = flow.rungs[wire2.rung];
    if (!rungFlow) {
      return false;
    }
    switch (wire2.carrier.type) {
      case "source":
        return true;
      case "contact":
        return rungFlow.contact_energized?.[wire2.carrier.branch]?.[wire2.carrier.after] === true;
      case "branch":
        return rungFlow.branch_energized?.[wire2.carrier.branch] === true;
      case "rung":
        return rungFlow.rung_result === true;
      case "output":
        return rungFlow.output_energized?.[wire2.carrier.index] === true;
      case "return":
        return (rungFlow.output_energized ?? []).some(Boolean);
      default:
        return false;
    }
  }

  // src/ldWebview/main.ts
  var vscode = acquireVsCodeApi();
  var program = { name: "NewProgram", schema_version: 2, rungs: [] };
  var powerFlow;
  var selection;
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
  function send(command) {
    vscode.postMessage({ type: "edit", command });
  }
  function sendReplace(next) {
    vscode.postMessage({ type: "modelChanged", program: next });
  }
  function render() {
    normalizeIds(program);
    byId("ld-canvas").innerHTML = renderSvg(layout(program), program, powerFlow);
    bindElementClicks();
    highlightSelection();
    updateStatus();
    syncTextarea();
  }
  function highlightSelection() {
    if (!selection) {
      return;
    }
    const node = document.querySelector(
      `#ld-canvas .element[data-rung="${selection.rung}"][data-branch="${selection.branch}"][data-index="${selection.index}"]`
    );
    node?.classList.add("selected");
  }
  function elementOrder() {
    const order = [];
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
  function moveSelection(delta) {
    const order = elementOrder();
    if (order.length === 0) {
      return;
    }
    const current = selection ? order.findIndex(
      (e) => e.rung === selection.rung && e.branch === selection.branch && e.index === selection.index
    ) : -1;
    const next = current === -1 ? 0 : Math.min(Math.max(current + delta, 0), order.length - 1);
    selection = order[next];
    highlightSelection();
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
        selection = { rung, branch, index };
        highlightSelection();
      });
      node.addEventListener("dblclick", () => {
        const rung = Number(node.getAttribute("data-rung"));
        const branch = Number(node.getAttribute("data-branch"));
        const index = Number(node.getAttribute("data-index"));
        beginRename(rung, branch, index, node);
      });
      const dragSource = node;
      dragSource.draggable = true;
      node.addEventListener("dragstart", (event) => {
        selection = {
          rung: Number(node.getAttribute("data-rung")),
          branch: Number(node.getAttribute("data-branch")),
          index: Number(node.getAttribute("data-index"))
        };
        event.dataTransfer?.setData("application/x-ld-element", JSON.stringify(selection));
        event.dataTransfer?.setData("text/plain", "element");
      });
    });
  }
  function keyboardInsert(paletteType) {
    for (const command of paletteCommands(program, paletteType)) {
      send(command);
    }
  }
  function applyDrop(hit, payloadType, elementSource) {
    if (elementSource) {
      if (hit.kind === "series") {
        send(
          commands.moveElement(elementSource, {
            rung: hit.rung,
            branch: hit.branch,
            index: hit.index
          })
        );
      } else if (hit.kind === "newRung") {
        send(commands.addRung());
        send(
          commands.moveElement(elementSource, {
            rung: program.rungs.length,
            branch: 0,
            index: 0
          })
        );
      }
      return;
    }
    if (!payloadType) {
      return;
    }
    if (hit.kind === "series") {
      send(
        commands.insertContact(
          hit.rung,
          hit.branch,
          hit.index,
          "NewVar",
          payloadType === "nc-contact"
        )
      );
    } else if (hit.kind === "parallel") {
      send(commands.insertParallelBranch(hit.rung, "NewVar"));
    } else if (hit.kind === "newRung") {
      for (const command of paletteCommands(program, payloadType)) {
        send(command);
      }
    }
    if (hit.kind === "output" || hit.kind === "element") {
      for (const command of paletteCommands(program, payloadType)) {
        send(command);
      }
    }
  }
  function beginRename(rung, branch, index, node) {
    const existing = document.getElementById("rename-input");
    if (existing) {
      existing.remove();
      return;
    }
    const contact = branch === -1 ? void 0 : program.rungs[rung]?.branches[branch]?.elements[index];
    const output = branch === -1 ? program.rungs[rung]?.outputs[index] : void 0;
    if (contact) {
    } else if (output && output.kind === "coil") {
    } else {
      return;
    }
    const currentName = contact ? contact.name : output.name;
    const box = node.getBBox();
    const input = document.createElement("input");
    input.id = "rename-input";
    input.className = "rename-input";
    input.value = currentName;
    input.style.left = `${box.x}px`;
    input.style.top = `${box.y}px`;
    input.style.width = `${Math.max(box.width, 90)}px`;
    const container = byId("canvas-container");
    container.style.position = "relative";
    container.appendChild(input);
    attachCompletion(input);
    input.focus();
    input.select();
    let settled = false;
    const close = (commit) => {
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
    input.addEventListener("keydown", (event) => {
      if (event.key === "Enter") {
        event.preventDefault();
        close(true);
      } else if (event.key === "Escape") {
        event.preventDefault();
        close(false);
      }
    });
    input.addEventListener("blur", () => close(true));
  }
  function wire() {
    const palette = byId("palette");
    for (const item of ELEMENT_PALETTE) {
      const node = document.createElement("div");
      node.className = "palette-item";
      node.title = item.title;
      node.textContent = item.label;
      node.addEventListener("click", () => {
        for (const command of paletteCommands(program, item.type)) {
          send(command);
        }
      });
      node.draggable = true;
      node.addEventListener("dragstart", (event) => {
        event.dataTransfer?.setData("application/x-ld-palette", item.type);
        event.dataTransfer?.setData("text/plain", item.type);
      });
      palette.appendChild(node);
    }
    const canvasContainer = byId("canvas-container");
    canvasContainer.addEventListener("dragover", (event) => {
      event.preventDefault();
    });
    canvasContainer.addEventListener("drop", (event) => {
      event.preventDefault();
      const drag = event;
      const data = drag.dataTransfer;
      if (!data) {
        return;
      }
      const elementJson = data.getData("application/x-ld-element");
      const elementSource = elementJson ? JSON.parse(elementJson) : void 0;
      const payloadType = data.getData("application/x-ld-palette") || (elementSource ? void 0 : void 0);
      const canvas = byId("ld-canvas");
      const rect = canvas.getBoundingClientRect();
      const geometry = layout(program);
      const hit = hitTest(geometry, drag.clientX - rect.left, drag.clientY - rect.top);
      applyDrop(hit, payloadType || void 0, elementSource);
    });
    byId("btn-save").addEventListener("click", () => {
      vscode.postMessage({ type: "save" });
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
        const next = parseProgram(event.target.value);
        byId("ld-canvas").innerHTML = renderSvg(layout(next), next, powerFlow);
        program = next;
        updateStatus();
      } catch {
      }
    });
    byId("ld-textarea").addEventListener("change", (event) => {
      try {
        sendReplace(parseProgram(event.target.value));
      } catch {
      }
    });
    window.addEventListener("keydown", (event) => {
      const target = event.target;
      if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA")) {
        return;
      }
      const meta = event.metaKey || event.ctrlKey;
      if (meta && event.key.toLowerCase() === "z") {
        event.preventDefault();
        vscode.postMessage({ type: event.shiftKey ? "redo" : "undo" });
        return;
      }
      if (meta) {
        return;
      }
      switch (event.key) {
        case "ArrowDown":
        case "ArrowRight":
          event.preventDefault();
          moveSelection(1);
          return;
        case "ArrowUp":
        case "ArrowLeft":
          event.preventDefault();
          moveSelection(-1);
          return;
        case "1":
          keyboardInsert("no-contact");
          return;
        case "2":
          keyboardInsert("nc-contact");
          return;
        case "c":
          keyboardInsert("coil");
          return;
        case "s":
          keyboardInsert("set-coil");
          return;
        case "r":
          keyboardInsert("reset-coil");
          return;
        case "t":
          keyboardInsert("ton");
          return;
        case "b":
          if (selection) {
            send(commands.insertParallelBranch(selection.rung, "NewVar"));
          }
          return;
        case "Enter":
          if (selection) {
            const node = document.querySelector(
              `#ld-canvas .element[data-rung="${selection.rung}"][data-branch="${selection.branch}"][data-index="${selection.index}"]`
            );
            if (node) {
              beginRename(selection.rung, selection.branch, selection.index, node);
            }
          }
          return;
        case "Delete":
        case "Backspace":
          if (selection) {
            event.preventDefault();
            send(commands.deleteElement(selection.rung, selection.branch, selection.index));
            selection = void 0;
          }
          return;
        default:
          return;
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
  function attachCompletion(input) {
    let list = document.getElementById("completion-list");
    if (list) {
      list.remove();
    }
    list = document.createElement("div");
    list.id = "completion-list";
    list.className = "completion-list";
    input.after(list);
    const refresh = () => {
      const hits = variables(program, input.value);
      list.innerHTML = "";
      for (const hit of hits.slice(0, 8)) {
        const item = document.createElement("div");
        item.className = "completion-item";
        item.textContent = hit;
        item.addEventListener("mousedown", (event) => {
          event.preventDefault();
          input.value = hit;
          list.innerHTML = "";
          input.focus();
        });
        list.appendChild(item);
      }
    };
    input.addEventListener("input", refresh);
    input.addEventListener("blur", () => {
      window.setTimeout(() => list.remove(), 150);
    });
    refresh();
  }
})();
