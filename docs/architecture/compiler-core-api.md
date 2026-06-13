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

## Current placeholder checks

The first contract implementation detects:

- unclosed Structured Text block comments,
- `PROGRAM` declarations without `END_PROGRAM`.

These checks are intentionally small and will be replaced/expanded by lexer and parser tasks.
