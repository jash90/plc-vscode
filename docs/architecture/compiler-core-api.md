# Compiler-core API contract

`plc_compiler_core` is the shared compiler boundary consumed by CLI, LSP, runtime, bytecode VM, and native backend tasks.

## Stable inputs

- `SourceDocument`: URI, version, and full text snapshot.

## Stable outputs

- `Analysis`: document URI/version plus diagnostics.
- `Diagnostic`: severity, range, code, and message.

## Consumer rules

- CLI and LSP must call compiler-core instead of duplicating parser or semantic checks.
- Runtime and backend tasks will consume typed compiler outputs added in later tasks.
- Diagnostics are represented independently from LSP so they can be rendered by multiple consumers.

## Current syntax- and semantic-backed checks

`plc_compiler_core` delegates syntax analysis to `plc_syntax` and semantic analysis to `plc_semantics`. It currently surfaces:

- unclosed Structured Text block comments,
- recoverable invalid token diagnostics,
- `PROGRAM` declarations without `END_PROGRAM`,
- unresolved assignment targets,
- basic assignment type mismatches for known elementary types.

The syntax crate owns lexer/parser diagnostics so CLI and LSP consumers do not duplicate syntax logic. It also exposes a rowan-backed CST layer that preserves trivia and token text ranges for future IDE formatting, semantic tokens, and refactoring features.

The semantic crate owns the first workspace symbol index, baseline IEC type model, and memoized query facade for parse/index/type-check operations. The query facade distinguishes standard-library data from user code and avoids recomputation for whitespace-only edits in the current syntax model. Compiler-core maps semantic diagnostics into the same stable `Diagnostic` shape used by CLI and LSP consumers.

## Current IDE-facing outputs

Compiler-core exposes hierarchical document symbols, completion candidates, and hover payloads for IDE consumers:

- top-level POUs are document symbols,
- variable declarations are nested under their containing POU,
- variable details include the known IEC type display name,
- completions include indexed symbols and core Structured Text keywords,
- hover shows keyword help or symbol/type information.

The LSP server maps these compiler-core outputs to `textDocument/documentSymbol`, completion, and hover responses and advertises the corresponding server capabilities.
