# Residual SEM0001 resolution — design

**Date:** 2026-06-15
**Status:** Approved (brainstorming) — ready for implementation planning
**Parent ticket:** PLC-79 (member/index access + named-args parser fixes already merged)

## Background

Two merged parser fixes under PLC-79 cut corpus `SEM0001` (unresolved symbol) from
**9017 → 2446 (−73%)** by stopping named call arguments (`f(IN := x)`) and member/index access
targets (`obj.field := x`) from being mis-parsed as simple assignments.

The remaining ~2446 `SEM0001` are **not** parser recovery problems. Inspection of
`crates/plc_semantics/src/lib.rs` (`analyze_workspace`) and `types.rs` (`SymbolIndex`) shows the
residual splits into three distinct causes. This spec covers all three; each ships as its own
ticket so the work stays inside the project's 1-task → 1-branch → TDD → MR workflow.

### How resolution works today

`analyze_workspace(files)` builds one cross-file `SymbolIndex` from **all** supplied files, then
for each assignment statement resolves the target with:

```
find_in_container(currentPOU, target)  ||  find_top_level(target)
```

- `find_in_container` matches symbols whose `container` equals the current POU name.
- `find_top_level` matches symbols with `container: None` — which today is **only POU names**.
- A symbol's `container` is the POU it was declared in (`None` for the POU name itself).

### Root causes of the residual ~2446

1. **Single-file measurement artifact.** `analyze_workspace` is already cross-file, but the corpus
   harness runs `plc run <file>` **one file at a time**. Every reference to a library `FUNCTION`,
   shared `TYPE`, or global declared in another file is reported as `SEM0001` in the *measurement*,
   even though the product LSP (`SemanticQueryDatabase`) analyzes the whole open workspace and would
   resolve it. Part of the 2446 is therefore not a product gap.
2. **ACTION/METHOD body scope (the largest real gap).** The parser models an `ACTION` as a flat
   top-level POU (`container = action name`, no link to its parent FB/PROGRAM). Statements in the
   action body reference variables declared in the **parent** POU; resolution never looks there, so
   every such reference is `SEM0001`. This is the `vTestCase1_act`-style remainder.
3. **`VAR_GLOBAL` / GVL unreachable.** Global declarations are indexed with
   `container = enclosing POU`, and `find_top_level` only matches POU names (`container: None`), so
   cross-POU global references never resolve. Standalone GVL files (not POUs) are not indexed at all.

OOP member access is already neutralized by the merged parser fix (those statements are no longer
assignments), so it is out of scope here.

## Strategy & ordering (measurement-first)

Three tickets off this one spec, in order, because each de-risks the next:

1. **Piece C — multi-file measurement** (no product code). Measure the corpus the way the product
   works (whole workspace, not one file) **before** building resolution features, to size the real
   gap. Likely the single biggest drop.
2. **Piece A — ACTION/METHOD scope** (parser + semantics). The largest *real* gap.
3. **Piece B — VAR_GLOBAL/GVL** (semantics-only). Smallest; cleans up the tail.

Re-baseline the corpus after each ticket so each has a measured before/after.

## Piece C — multi-file measurement

**Goal:** measure SEM0001 as the product would see it, not as isolated single-file runs.

- Add a CLI mode `plc check <path>`: given a directory, collect its `.st` files, call the existing
  `analyze_workspace(&files)` **once** over the whole set, and aggregate diagnostics — mirroring the
  LSP's `SemanticQueryDatabase`.
- The corpus harness groups files by their top-level vendored source-repo directory (each of the
  ~10 repos = one workspace) and runs `plc check` per group.
- `run <file>` (single-file execution) is unchanged. No semantics changes. Pure function:
  in = file set, out = aggregated diagnostics.

**Interface:** `plc check <dir>` exits non-zero if any diagnostics are produced (same convention as
`run`), and prints `CODE: message` lines grouped by file.

## Piece A — ACTION/METHOD scope resolution

**Parser** (`crates/plc_syntax/src/parser.rs`):

- Add `parent: Option<String>` to the parsed unit.
- Set it two ways:
  - (a) a nested `ACTION Name … END_ACTION` / `METHOD Name … END_METHOD` inside an FB/PROGRAM gets
    the enclosing POU name as `parent`;
  - (b) a qualified header `ACTION FB.Act` / `METHOD FB.M` parses the dotted prefix as `parent`
    (CODESYS/TwinCAT textual exports use both forms).

**Semantics** (`crates/plc_semantics/src/lib.rs`, `types.rs`):

- Resolution for a statement in an action/method chains:
  **action-local VAR → parent POU's vars → globals → top-level POU names.**
- Implemented by walking the declared parent chain when the current container is an action/method.
- **Bounded risk:** widen the lookup only to the *declared* parent, never "any POU", so a typo'd
  variable is not silently resolved against an unrelated same-named symbol elsewhere.

## Piece B — VAR_GLOBAL/GVL indexing

**Indexing** (`index_file_symbols`):

- Declarations from a `VarBlockKind::Global` block are inserted as globals: `container: None`,
  new `SymbolKind::Global`, so they are reachable workspace-wide.
- Standalone GVL files (no POU wrapper) get a lightweight "global block" unit so their declarations
  are indexed too.

**Resolution:**

- Add a `globals` fallback step in the chain (after container/parent, before giving up). Distinct
  from `find_top_level` so globals and POU names stay separable.

## Testing

Inline-ST regression tests mirroring existing `crates/plc_semantics` and `crates/plc_syntax` test
style (no `/tmp` or external fixtures):

- **Piece A:** action body resolves a parent-POU variable; qualified `FB.Act` header resolves;
  **negatives** — a genuinely unknown name still flags `SEM0001`; an unrelated same-named variable
  in a different POU does **not** falsely resolve.
- **Piece B:** a cross-POU `VAR_GLOBAL` reference resolves; a standalone GVL global resolves; a
  non-global same-named local in another POU is not treated as global.
- **Piece C:** unit test that `analyze_workspace` over a two-file set resolves a cross-file
  reference that fails when each file is analyzed alone.

Each ticket re-baselines the corpus (`SEM0001` before/after) and passes the standard gate:
`cargo test --workspace`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets
-- -D warnings`.

## Out of scope

- Full 3rd-edition OOP type modeling (METHOD return types, THIS/SUPER, inheritance) beyond name
  resolution for method bodies.
- Type-checking improvements (`SEM0002`); this spec only addresses unresolved-symbol false
  positives.
- Editing the vendored corpus under `tests/st` — adapt the toolchain to the files, never the files.
