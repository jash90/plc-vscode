# IEC 61131-3 conformance levels and coverage matrix

PLC VS Code targets an explicit, testable subset of IEC 61131-3 so parser, semantic, runtime, and IDE work can progress without implying full standard coverage.

## Baseline

The MVP baseline is IEC 61131-3 2nd edition Structured Text.

Initial implementation focuses on:

- textual Structured Text source files,
- functions, function blocks, programs, and actions,
- declarations needed for common ST projects,
- deterministic diagnostics and execution for testable examples,
- standard functions/function blocks needed by MVP runtime scenarios.

## Deferred 3rd edition / OOP scope

IEC 61131-3 3rd edition object-oriented features are deferred. Deferred scope includes:

- classes,
- interfaces,
- inheritance-oriented workflows,
- advanced methods beyond MVP POU/function-block needs,
- vendor-specific OOP extensions.

These features may appear in design notes, but they should not block parser/LSP MVP, VM/runtime MVP, or the first native backend prototype.

## Conformance levels

| Level | Meaning | Typical evidence |
|---|---|---|
| Not started | No implementation or tests exist. | Empty matrix row or tracking issue only. |
| Parsed | Syntax is recognized and represented with source ranges. | Lexer/parser tests, CST snapshots, malformed fixture coverage. |
| Analyzed | Names/types are understood and diagnostics are emitted. | Semantic positive/negative tests, diagnostic snapshots. |
| Executable | Runtime/VM behavior is implemented deterministically. | Runtime fixtures with expected outputs/state. |
| IDE exposed | Feature is available through LSP/VS Code UX. | LSP protocol tests or extension smoke tests. |

Evidence paths are relative to the repository root. `crates/plc_llvm_backend` is excluded from the default workspace and built separately (see `docs/architecture/llvm-toolchain.md`).

## Coverage matrix

| Feature area | Parser | Semantic | Runtime | IDE | Evidence |
|---|---|---|---|---|---|
| Comments and whitespace trivia | Parsed | N/A | N/A | Semantic tokens | `crates/plc_syntax/tests/lexer.rs`, `cst.rs`; unclosed-comment diagnostic `PLC0001` |
| Identifiers, keywords, literals, operators | Parsed | N/A | N/A | Semantic tokens | `crates/plc_syntax/tests/lexer.rs`; `crates/plc_lsp_server/tests/semantic_tokens.rs` |
| `PROGRAM` POU | Parsed | Analyzed | Executable (scan cycle) | Symbols/diagnostics | `crates/plc_syntax/tests/parser.rs`, `crates/plc_semantics/tests/symbol_index.rs`, `crates/plc_runtime/tests/conformance_runtime.rs` |
| `FUNCTION` POU | Parsed | Analyzed (indexed) | Partial (stdlib calls) | Symbols/completion/signature help | `crates/plc_syntax/tests/parser.rs`, `crates/plc_semantics/tests/symbol_index.rs`, `crates/plc_lsp_server/tests/signature_help.rs` |
| `FUNCTION_BLOCK` POU | Parsed | Analyzed (indexed) | Executable (standard FBs) | Members/completion/signature help | `crates/plc_runtime/tests/timers.rs`, `counters.rs`; `crates/plc_lsp_server/tests/{signature_help,language_features}.rs` |
| `ACTION` | Parsed | Indexed | Not started | Symbols | `crates/plc_syntax/tests/parser.rs`, `crates/plc_semantics/tests/symbol_index.rs` |
| Variable declaration blocks (`VAR`, `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT`, `VAR_GLOBAL`, `VAR_TEMP`) | Parsed | Analyzed (symbol kinds) | Init state | Symbols/completion | `crates/plc_syntax/tests/parser.rs`, `crates/plc_semantics/tests/symbol_index.rs` |
| Constants (`CONSTANT`) | Parsed (modifier) | Partial | Partial | — | Folding deferred; `crates/plc_syntax/tests/parser.rs` |
| Assignment | Parsed | Analyzed (`SEM0001`, `SEM0002`) | Executable | Diagnostics | `crates/plc_semantics/tests/diagnostics.rs`, `crates/plc_runtime/tests/conformance_runtime.rs` |
| `IF` / `CASE` | Parsed | Partial | Not started (VM) | — | Runtime control flow deferred; `crates/plc_syntax/tests/parser.rs` |
| `FOR` / `WHILE` / `REPEAT` | Parsed | Partial | Not started (VM) | — | Loop execution deferred; `crates/plc_syntax/tests/parser.rs` |
| `RETURN` / `EXIT` / `CONTINUE` | Parsed | Partial | Not started (VM) | — | `crates/plc_syntax/tests/parser.rs` |
| Elementary types (`BOOL`, integers, `REAL`, `LREAL`, time/date, `STRING`, `WSTRING`) | Parsed | Analyzed (type model) | Executable (`BOOL`/`INT`/`REAL`/`STRING`) | Semantic tokens | `crates/plc_semantics/tests/type_model.rs`, `crates/plc_runtime/tests/conformance_runtime.rs` |
| Derived types (`ARRAY`, `STRUCT`, `ENUM`, alias, subrange) | Partial | Partial (type model aware) | Not started | — | Full parsing/runtime deferred; `crates/plc_semantics/tests/type_model.rs` |
| Name resolution and member access | N/A | Analyzed (`SEM0001`; FB instance/type resolution) | N/A | Definition/references, completion, signature help | `crates/plc_semantics/tests/diagnostics.rs`, `crates/plc_lsp_server/tests/{navigation,language_features,signature_help}.rs` |
| Type-checking diagnostics | N/A | Analyzed (assignment: unresolved `SEM0001`, mismatch `SEM0002`) | N/A | Diagnostics | Full call/argument checking deferred; `crates/plc_semantics/tests/diagnostics.rs`, `crates/plc_lsp_server/tests/diagnostics_mapping.rs` |
| Pure standard functions (conversion, math, string, selection) | Parsed as calls | Partial (completion/signature metadata) | Executable | Completion/signature help | `crates/plc_runtime/tests/stdlib.rs`, `crates/plc_lsp_server/tests/{language_features,signature_help}.rs` |
| Timers (`TON`, `TOF`, `TP`) | Parsed as FB calls | Indexed | Executable (virtual time) | Members | `crates/plc_runtime/tests/timers.rs`, `virtual_time.rs` |
| Counters and edge detectors (`CTU`, `CTD`, `CTUD`, `R_TRIG`, `F_TRIG`) | Parsed as FB calls | Indexed | Executable (scan-cycle state) | Members | `crates/plc_runtime/tests/counters.rs`, `scan_cycle.rs` |
| Serializable bytecode | N/A | HIR lowering input | Executable (assignment/arithmetic subset) | N/A | `crates/plc_hir/tests/lowering.rs`, `crates/plc_runtime/tests/bytecode.rs`, `bytecode_golden.rs` |
| LLVM/native backend | N/A | HIR lowering input | Prototype (IR/object for MVP subset) | N/A | `crates/plc_llvm_backend/tests/golden_ir.rs`, `output_modes.rs`; `docs/architecture/llvm-toolchain.md` (excluded from default workspace) |

## IDE features (LSP)

| Feature | Level | Evidence |
|---|---|---|
| Diagnostics | IDE exposed | `crates/plc_lsp_server/tests/diagnostics_mapping.rs`, `change_recovery.rs` |
| Completion (symbols, keywords, standard functions, FB members) | IDE exposed | `crates/plc_lsp_server/tests/language_features.rs`, `crates/plc_compiler_core/tests/api_contract.rs` |
| Hover | IDE exposed | `crates/plc_lsp_server/tests/language_features.rs` |
| Definition / references | IDE exposed | `crates/plc_lsp_server/tests/navigation.rs` |
| Document symbols | IDE exposed | `crates/plc_lsp_server/tests/document_symbols.rs` |
| Formatting / code actions | IDE exposed | `crates/plc_lsp_server/tests/editing.rs` |
| Signature help | IDE exposed | `crates/plc_lsp_server/tests/signature_help.rs`, `crates/plc_compiler_core/tests/api_contract.rs` |
| Workspace symbols | IDE exposed | `crates/plc_lsp_server/tests/workspace_symbols.rs`, `crates/plc_compiler_core/tests/api_contract.rs` |
| Semantic tokens | IDE exposed | `crates/plc_lsp_server/tests/semantic_tokens.rs`, `crates/plc_compiler_core/tests/api_contract.rs` |

## Explicitly deferred and partial scopes

These remain out of MVP acceptance and must stay called out until implemented:

- **Runtime control flow** — `IF`/`CASE`/`FOR`/`WHILE`/`REPEAT` and `RETURN`/`EXIT`/`CONTINUE` are parsed but not yet executed by the VM (which currently covers assignment, arithmetic, and stateful function blocks).
- **Full signature / call-argument checking** — signature help and call signature data exist, but semantic validation of argument count/types is not implemented (only assignment-level `SEM0001`/`SEM0002`).
- **Complex expressions** — beyond the assignment/arithmetic subset lowered to bytecode.
- **Derived types** — `ARRAY`/`STRUCT`/`ENUM`/alias/subrange are recognized by the type model but lack full parsing and runtime representation.
- **Constant folding**.
- **3rd edition OOP features** — classes, interfaces, inheritance, advanced methods.
- **Vendor-specific extensions**.

## Updating the matrix

When a task changes support level:

1. Update the relevant row in this document.
2. Add or update fixtures/tests that prove the new level, and cite them in the Evidence column.
3. Keep parser, semantic, runtime, and IDE evidence separate so partial support is visible.
4. Move any newly implemented item out of "Explicitly deferred and partial scopes".
