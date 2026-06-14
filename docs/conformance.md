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

## Coverage matrix

| Feature area | Parser support | Semantic support | Runtime support | Notes |
|---|---|---|---|---|
| Comments and whitespace trivia | Planned | N/A | N/A | Preserve ranges for IDE formatting/refactoring. |
| Identifiers, keywords, literals, operators | Planned | Planned | Planned | Lexer is the first required layer. |
| `PROGRAM` POU | Partial placeholder exists | Not started | Placeholder CLI execution exists | Replace placeholder checks with syntax/parser outputs. |
| `FUNCTION` POU | Planned | Planned | Planned | Needed for calls and standard functions. |
| `FUNCTION_BLOCK` POU | Planned | Planned | Planned | Needed for timers, counters, and retained state. |
| `ACTION` | Planned | Planned | Not started | Parse in syntax MVP; runtime behavior can follow later. |
| Variable declaration blocks (`VAR`, `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT`, `VAR_GLOBAL`, `VAR_TEMP`) | Planned | Planned | Planned | Required for symbol index and state model. |
| Constants | Planned | Planned | Planned | Required for type checking and folding later. |
| Assignment | Planned | Planned | Planned | Basic executable statement. |
| `IF` / `CASE` | Planned | Planned | Planned | Control-flow MVP. |
| `FOR` / `WHILE` / `REPEAT` | Planned | Planned | Planned | Loop MVP; runtime tests must avoid nondeterminism. |
| `RETURN` / `EXIT` / `CONTINUE` | Planned | Planned | Planned | Control-flow validation required. |
| Workspace symbols | N/A | Planned | N/A | Feeds navigation, completion, and semantic tokens. |
| Name resolution and member access | N/A | Planned | N/A | Required before navigation and type diagnostics. |
| Elementary types (`BOOL`, integers, `REAL`, `LREAL`, time/date, `STRING`, `WSTRING`) | Planned | Planned | Planned | 2nd edition baseline. |
| Derived types (`ARRAY`, `STRUCT`, `ENUM`, alias, subrange) | Planned | Planned | Planned | Runtime representation can be incremental. |
| Type checking diagnostics | N/A | Planned | N/A | Invalid assignments, calls, and unresolved symbols. |
| Pure standard functions (conversion, math, string, selection) | Parsed as calls | Planned | Planned | Type checking should precede runtime behavior. |
| Timers (`TON`, `TOF`, `TP`) | Parsed as FB calls | Planned | Planned | Requires deterministic virtual time. |
| Counters and edge detectors (`CTU`, `CTD`, `CTUD`, `R_TRIG`, `F_TRIG`) | Parsed as FB calls | Planned | Planned | Requires retained scan-cycle state. |
| Serializable bytecode | N/A | Planned lowering input | Planned | VM contract follows typed compiler outputs. |
| LLVM/native backend | N/A | Planned lowering input | N/A | Later phase after VM/runtime path is useful. |
| 3rd edition OOP features | Deferred | Deferred | Deferred | Do not include in MVP acceptance. |

## Updating the matrix

When a task changes support level:

1. Update the relevant row in this document.
2. Add or update fixtures/tests that prove the new level.
3. Keep parser, semantic, runtime, and LSP evidence separate so partial support is visible.
