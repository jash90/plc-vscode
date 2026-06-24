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

The LD model is a serde-serializable tree:

| Type | Description |
|---|---|
| `LdProgram` | Top-level: `{ name, rungs: Vec<Rung> }` |
| `Rung` | One horizontal line: `{ branches: Vec<SeriesBranch>, outputs: Vec<OutputElement> }` |
| `SeriesBranch` | AND chain: `{ elements: Vec<ContactElement> }` |
| `ContactElement` | `{ name, negated }` — NO (`| |`) or NC (`|/|`) |
| `OutputElement` | Coil or Block (tagged enum) |
| `CoilVariant` | Normal `( )`, Set `(S)`, Reset `(R)` |
| `BlockArg` | Named pin: `{ name, value }` |
| `PowerFlowResult` | Per-rung energized state for visualization |

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
elements are energized given variable states. Used by the CLI (`plc ld --watch`)
and the VS Code webview for live green/gray coloring.

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
