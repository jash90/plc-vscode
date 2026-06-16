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
| `IF` / `CASE` | Parsed | Partial | Executable (tree-walking interpreter) | — | `crates/plc_runtime/src/interp.rs`, `crates/plc_runtime/tests/{interpreter,prg_test_st}.rs` |
| `FOR` / `WHILE` / `REPEAT` | Parsed | Partial | Executable (tree-walking interpreter) | — | `crates/plc_runtime/src/interp.rs`, `crates/plc_runtime/tests/{interpreter,prg_test_st}.rs` |
| `RETURN` / `EXIT` / `CONTINUE` | Parsed | Partial | Executable (loop/block control flow) | — | `crates/plc_runtime/src/interp.rs`, `crates/plc_runtime/tests/interpreter.rs` |
| Elementary types (`BOOL`, integers, `REAL`, `LREAL`, time/date, `STRING`, `WSTRING`, bit-strings) | Parsed | Analyzed (type model) | Executable (`BOOL`/`INT`/`REAL`/`STRING`/`WORD`/`TIME`) | Semantic tokens | `crates/plc_semantics/tests/type_model.rs`, `crates/plc_runtime/tests/{literals,conformance_runtime}.rs` |
| Operators (arithmetic, comparison, `AND`/`OR`/`XOR`/`NOT`/`MOD`, bit-shifts) | Parsed (operator-keywords classified) | Partial | Executable (precedence-correct evaluator) | Semantic tokens | `crates/plc_runtime/src/interp.rs`, `crates/plc_runtime/tests/interpreter.rs` |
| Literals (radix `16#`/`2#`/`8#`, typed, compound duration `T#1h30m`) | Parsed | N/A | Executable (decoded to values) | Number tokens | `crates/plc_runtime/tests/literals.rs` |
| Derived types (`ARRAY`, `STRUCT`, `ENUM`, alias, subrange) | Partial | Partial (type model aware) | Not started | — | Full parsing/runtime deferred; `crates/plc_semantics/tests/type_model.rs` |
| Name resolution and member access | N/A | Analyzed (`SEM0001`; FB instance/type resolution) | N/A | Definition/references, completion, signature help | `crates/plc_semantics/tests/diagnostics.rs`, `crates/plc_lsp_server/tests/{navigation,language_features,signature_help}.rs` |
| Type-checking diagnostics | N/A | Analyzed (assignment: unresolved `SEM0001`, mismatch `SEM0002`) | N/A | Diagnostics | Full call/argument checking deferred; `crates/plc_semantics/tests/diagnostics.rs`, `crates/plc_lsp_server/tests/diagnostics_mapping.rs` |
| Pure standard functions (math `ABS`/`SQRT`/`EXPT`, selection `MIN`/`MAX`/`LIMIT`/`SEL`, bit-shifts `SHL`/`SHR`, string `LEN`/`CONCAT`/`LEFT`/`RIGHT`/`MID`, conversions and the `*_TO_STRING` family incl. `TO_STRING`) | Parsed as calls | Partial (completion/signature metadata) | Executable | Completion/signature help | `crates/plc_runtime/tests/stdlib.rs`, `crates/plc_lsp_server/tests/{language_features,signature_help}.rs` |
| Timers (`TON`, `TOF`, `TP`) | Parsed as FB calls | Indexed | Executable (virtual time, wired into program FB-call execution + member reads) | Members | `crates/plc_runtime/tests/timers.rs`, `virtual_time.rs`, `prg_test_st.rs` |
| Counters and edge detectors (`CTU`, `CTD`, `CTUD`, `R_TRIG`, `F_TRIG`) | Parsed as FB calls | Indexed | Executable (scan-cycle state, wired into program FB-call execution + member reads; `RESET`/`R` and `LOAD`/`LD` aliases) | Members | `crates/plc_runtime/tests/counters.rs`, `scan_cycle.rs`, `prg_test_st.rs` |
| Serializable bytecode | N/A | HIR lowering input | Executable (assignment/arithmetic subset) | N/A | `crates/plc_hir/tests/lowering.rs`, `crates/plc_runtime/tests/bytecode.rs`, `bytecode_golden.rs` |
| LLVM/native backend | N/A | HIR lowering input | Prototype (IR/object for MVP subset) | N/A | `crates/plc_llvm_backend/tests/golden_ir.rs`, `output_modes.rs`; `docs/architecture/llvm-toolchain.md` (excluded from default workspace) |

## Runtime execution and output formatting

`plc run <file.st> [scans]` analyzes the program (diagnostics gate), then executes
it through the deterministic scan-cycle runtime and prints an online "watch"
snapshot of every declared scalar variable. Scalar/string formatting matches
CODESYS / TwinCAT conventions so results can be compared directly against those
tools:

- `REAL_TO_STRING` keeps the decimal point and a trailing zero for whole numbers
  (`REAL_TO_STRING(1024.0)` → `"1024.0"`, `12.0` → `"12.0"`), and trims to at most
  six fractional digits otherwise (`3.5`).
- `WORD_TO_STRING` / `*_TO_STRING` render bit-strings as **decimal**.
- `TIME_TO_STRING` emits canonical compound `T#` literals (`T#0ms`, `T#2s`, `T#1h30m`).

`crates/plc_runtime/tests/prg_test_st.rs` pins these against a full exerciser
program. Note that an exerciser's hand-written `// expected` comments may omit the
trailing `.0` on integral REALs; the runtime follows the real-compiler output.

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

- **Runtime control flow / full expressions in the bytecode VM and native backend** — the deterministic **tree-walking interpreter** (`crates/plc_runtime/src/interp.rs`) now executes `IF`/`CASE`/`FOR`/`WHILE`/`REPEAT`, `RETURN`/`EXIT`/`CONTINUE`, full precedence-correct expressions (arithmetic, comparison, `AND`/`OR`/`XOR`/`NOT`/`MOD`, bit-shifts), standard-function calls, and standard function-block calls with member reads. The HIR-lowered **bytecode VM** and **native backend** still cover only the assignment/arithmetic subset; bringing them to interpreter parity is the remaining work.
- **Full signature / call-argument checking** — signature help and call signature data exist, but semantic validation of argument count/types is not implemented (only assignment-level `SEM0001`/`SEM0002`).
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
