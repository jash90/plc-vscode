# Bytecode format and viewer contract

This document defines the MVP serializable bytecode format produced by the
`plc_runtime` crate and the contract consumed by the VS Code bytecode viewer.

## Instruction set (MVP)

The MVP target is a small **stack machine**. The instruction set covers the
current expression surface and is intentionally minimal so it can grow
alongside the runtime and the native backend:

| Instruction      | Effect                                             |
| ---------------- | -------------------------------------------------- |
| `PushInt(i64)`   | Push an integer literal onto the stack.            |
| `PushBool(bool)` | Push a boolean literal.                            |
| `PushReal(f64)`  | Push a real literal.                               |
| `PushStr(String)`| Push a string literal.                             |
| `LoadVar(name)`  | Push the current value of a variable.              |
| `StoreVar(name)` | Pop the stack into a variable.                     |
| `Add` / `Sub`    | Pop two operands, push the arithmetic result.      |

The set is defined as `plc_runtime::Instruction` and grouped into a
`BytecodeModule { name, instructions }`.

## Serialization format

Modules serialize to **JSON** via `serde` (`BytecodeModule::to_json` /
`from_json`). JSON is chosen for the MVP because it is human-inspectable,
round-trips losslessly (covered by tests), and is trivial to transport to the
editor. A compact binary format may be added later without changing the
in-memory instruction model.

## VS Code bytecode viewer contract

The viewer contract is the **indexed mnemonic listing** returned by
`BytecodeModule::disassemble()`:

```
0000  LOAD_VAR Count
0001  PUSH_INT 1
0002  ADD
0003  STORE_VAR Count
```

Each line is `"<zero-padded index>  <MNEMONIC> [operand]"`. The VS Code client
renders this listing read-only; mnemonics are stable identifiers the viewer can
rely on for syntax highlighting and navigation. The editor obtains a module
either as JSON (and disassembles client-side) or as the pre-rendered listing
from the language server.
