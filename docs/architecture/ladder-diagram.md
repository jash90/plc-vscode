# Ladder Diagram (LD) Module

## Overview

The LD module adds IEC 61131-3 Ladder Diagram support to PLC VS Code. It provides a
graphical editor, LD→ST compilation through the canonical HIR, execution via the
existing runtime, and live power-flow visualization.

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│  VS Code Custom Editor (Webview / Canvas)                 │
│  • Drag/drop contacts, coils, timer/counter blocks        │
│  • Series (AND) / parallel (OR) connections               │
│  • Live power-flow coloring (green = energized)           │
│  • Reads/writes .ld JSON files                            │
└───────────────┬──────────────────────────────────────────┘
                │ JSON model (LdProgram)
                ▼
┌──────────────────────────────────────────────────────────┐
│  plc_ld (Rust crate)                                      │
│  • LdProgram model (serde JSON serialization)             │
│  • lower_ld_program(): LdProgram → HirModule              │
│  • evaluate_power_flow(): variable state → PowerFlowResult│
└───────────────┬──────────────────────────────────────────┘
                │ HirModule
                ▼
┌──────────────────────────────────────────────────────────┐
│  plc_lang (Language Registry / IR Hub)                    │
│  • LdFrontend: lower() → HirModule (delegates to plc_ld)  │
│  • StFrontend: render() → ST source text                  │
│  • registry.convert("ld", "st", &doc) = the pipeline      │
└───────────────┬──────────────────────────────────────────┘
                │ ST source text
                ▼
┌──────────────────────────────────────────────────────────┐
│  plc_runtime (existing scan-cycle engine)                 │
│  • Runtime::from_source(st_text)                          │
│  • run_scans(N) → watch() = variable values               │
│  • Timers (TON/TOF/TP), Counters (CTU/CTD/CTUD), Edges    │
└──────────────────────────────────────────────────────────┘
```

## Crate: `plc_ld`

### Model (`model.rs`)

The LD model is a serde-serializable tree. Wire format **v2** (PLC-107) adds
`schema_version`, optional rung `comment`s, and optional stable element `id`s —
all additive, so v1 files parse unchanged:

| Type | Description |
|---|---|
| `LdProgram` | Top-level: `{ name, schema_version, rungs: Vec<Rung> }` |
| `Rung` | One horizontal line: `{ id?, comment?, branches, outputs }` |
| `SeriesBranch` | AND chain: `{ elements: Vec<ContactElement> }` |
| `ContactElement` | `{ id?, name, negated }` — NO (`\| \|`) or NC (`\|/\|`) |
| `OutputElement` | Coil or Block (tagged enum), both with `id?` |
| `CoilVariant` | Normal `( )`, Set `(S)`, Reset `(R)` |
| `BlockArg` | Named pin: `{ name, value }` |
| `PowerFlowResult` | Per-rung energized state for visualization |

#### Element ids (`ids.rs`)

`normalize_ids(&mut LdProgram)` assigns `r{n}` to rungs and `e{n}` to
contacts/outputs in document order, preserving valid existing ids, resolving
duplicate claims (first occurrence wins), and seeding fresh counters past the
highest numeric suffix in use. Editors normalize on load (v1 upgrade) and
before save. Ids are ignored by lowering — they exist for diagnostics, undo,
and interchange. See `tests/ld/motor_control_v2.ld` for a canonical v2 file.

### Lowering (`lower.rs`)

`lower_ld_program(&LdProgram) → HirModule` maps LD constructs to HIR:

| LD | HIR |
|---|---|
| Series contacts | `Binary { And, ... }` |
| Parallel branches | `Binary { Or, ... }` |
| NC contact | `Unary { Not, Var }` |
| Normal coil | `HirStmt::Assign` |
| SET coil | `HirStmt::Set` |
| RESET coil | `HirStmt::Reset` |
| FB block (TON, CTU) | `HirStmt::FbCall` |

The IN/CU pin of a timer/counter block receives the **rung logic expression**
(contacts + AND/OR), not the literal variable name.

### Power-flow (`power_flow.rs`)

`evaluate_power_flow(&LdProgram, &VarState) → PowerFlowResult` evaluates which
elements are energized given variable states. `RungPowerFlow.contact_energized`
reports cumulative energy after each contact (`[branch][contact]`), enabling
per-contact wire coloring rather than whole-branch tinting. Used by the CLI
(`plc ld --watch`) and the VS Code webview for live green/gray coloring.

## HIR Extension

The canonical IR (`plc_hir`) was extended with:

- `BinaryOp`: `And`, `Or`, `Xor`, `Eq`, `Ne`, `Lt`, `Le`, `Gt`, `Ge`, `Mul`, `Div`, `Mod`
- `UnaryOp`: `Not`, `Neg`
- `HirExpr::Unary { op, expr }`
- `HirExpr::Call { name, args }`
- `HirStmt`: `Assign`, `Set`, `Reset`, `FbCall`
- `HirProgram.statements`: extended statement list (alongside the original `body`)

The ST renderer (`plc_lang/src/st.rs`) handles all new operators and `HirStmt`
variants, so LD→ST conversion works through the IR hub without any LD-specific
rendering code.

## CLI

- `plc ld <file.ld>` — compile LD→ST, execute via runtime, show watch table
- `plc ld <file.ld> --watch` — emit power-flow JSON for webview consumption

## VS Code Editor

A custom editor provider (`plc-vscode.ldEditor`) opens `.ld` files in a webview
with:
- Canvas/SVG rendering of rungs with contacts, coils, and blocks
- Palette of elements (NO/NC contact, normal/SET/RESET coil, TON/CTU)
- Click-to-rename for variable names
- JSON toggle for direct model editing
- Save triggers power-flow evaluation → green/gray coloring

## Files

| File | Description |
|---|---|
| `crates/plc_ld/src/model.rs` | LD model + serde |
| `crates/plc_ld/src/lower.rs` | LD → HIR lowering |
| `crates/plc_ld/src/power_flow.rs` | Power-flow evaluation |
| `crates/plc_lang/src/ld.rs` | `LdFrontend` (LanguageFrontend impl) |
| `crates/plc_lang/src/st.rs` | ST renderer (extended for new operators) |
| `crates/plc_cli/src/main.rs` | `plc ld` subcommand |
| `editors/vscode/src/ldEditor.ts` | Custom editor + webview |
| `tests/ld/motor_control.ld` | Test fixture |

### Validation (`validate.rs`, PLC-108)

`validate(&LdProgram) → Vec<LdDiagnostic>` is a pure rule set shared by the
editor, CLI, and LSP diagnostics. Diagnostics key on the PLC-107 element ids.
`LdFrontend::lower()` attaches **errors only** (warnings must not block
conversion through the SourceHasErrors path):

| Code | Severity | Meaning |
|---|---|---|
| `LD0001` | error | Empty rung, or empty branch (lowers to a silent always-true) |
| `LD0002` | error | Duplicate FB instance name (cross-rung) |
| `LD0003` | error | FB type outside `STANDARD_FB_TYPES` (runtime dispatches exact names) |
| `LD0004` | warning | Rung without outputs |
| `LD0005` | warning | Pin unknown for the FB type (see `fb_pins`) |
| `LD0006` | error | Empty variable/instance/pin value |
| `LD0007` | error | FB instance name collides with a variable name (duplicate VARs in ST) |

### Interchange, simulation, and testing (PLC-113…118)

- `plc ld --serve` (crates/plc_cli/src/ld_serve.rs): synchronous
  line-JSON simulation protocol over stdio; the CLIENT drives pacing
  (`tick` = one scan), so the server is fully deterministic. The editor
  paces ticks host-side and live-reloads the in-memory model — simulating
  without saving.
- PLCopen XML (crates/plc_plcopen): model-level interchange
  (`to_plcopen`/`from_plcopen`), NOT through the HIR hub — the graphical
  model (ids, comments, rungs) has no HIR representation. `plc convert ld
  plcopen` / `plcopen ld`; extension export/import commands.
- `syntaxes/ladder-diagram.schema.json`: JSON Schema for raw `.ld`
  editing (jsonValidation), golden-tested against the fixture corpus.
- E2E (`editors/vscode/test/e2e`): @vscode/test-electron suite asserting
  through the `plc-vscode.ld.*` test hooks — never the webview DOM.
