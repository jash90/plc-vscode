# PLC VS Code roadmap phases and milestone gates

The roadmap follows the technical dependency order for the project: parser/LSP first, VM second, LLVM later, and IEC 61131-3 3rd edition object-oriented features last.

## Phase 0 — Foundation and scope control

Purpose: make the project buildable, understandable, and legally safe before expanding compiler implementation.

Entry criteria:

- Repository workspace exists.
- `plc_compiler_core` is the shared boundary for CLI and LSP consumers.
- Jira epics and tasks exist for the planned product areas.

Exit criteria:

- Reference-project licensing rules are documented.
- Developer setup and build prerequisites are documented.
- IEC conformance levels and MVP coverage matrix are documented.
- Workspace architecture contract tests pass.

## Phase 1 — Syntax and IDE diagnostics MVP

Purpose: parse Structured Text reliably enough for editor feedback, even on incomplete code.

Entry criteria:

- Phase 0 exit criteria are met.
- Syntax crate boundaries are defined.
- Test fixture conventions are available for parser/golden coverage.

Exit criteria:

- Lexer recognizes keywords, identifiers, literals, operators, comments, whitespace, and recoverable invalid tokens while retaining source ranges.
- Error-tolerant parser handles core `PROGRAM`, `FUNCTION`, `FUNCTION_BLOCK`, and `ACTION` syntax.
- MVP declarations and statements are parsed.
- CLI and LSP diagnostics are produced through `plc_compiler_core` rather than duplicated in consumers.
- Parser recovery fixtures cover malformed Structured Text samples.

## Phase 2 — Semantic analysis and LSP features

Purpose: add compiler-grade understanding of names, scopes, declarations, types, and editor navigation.

Entry criteria:

- Phase 1 parser outputs are stable enough for downstream consumers.
- Syntax diagnostics include source ranges.
- Incremental-query design is agreed for IDE responsiveness.

Exit criteria:

- Workspace symbol index supports deterministic cross-file lookup.
- IEC elementary and derived type model is available to diagnostics and hover.
- Name resolution, member access, invalid calls, unresolved names, and type mismatches produce diagnostics.
- Completion, hover, signature help, symbols, semantic tokens, go-to-definition, and find-references are backed by compiler-core data.
- LSP protocol integration tests cover the shipped language features.

## Phase 3 — Runtime, simulator, bytecode VM, and standard library

Purpose: execute validated Structured Text deterministically before native code generation.

Entry criteria:

- Phase 2 semantic analysis can produce typed compiler outputs required by execution.
- Runtime model is documented.
- Conformance suite layout exists for parser, semantic, and runtime examples.

Exit criteria:

- Scan-cycle model covers input scan, logic scan, output scan, retained state, and deterministic scheduling.
- Virtual time is separated from wall-clock time.
- Variable forcing and state inspection APIs support automated tests.
- Timer, counter, edge detector, conversion, math, string, and selection MVP functions/function blocks are covered by tests.
- Serializable bytecode direction and viewer contract are documented.

## Phase 4 — Native backend, packaging, and later IEC features

Purpose: add native compilation and release workflows after the interpreted/VM path is useful and tested.

Entry criteria:

- Phase 3 runtime and typed lowering inputs are stable.
- HIR/lowering responsibilities are documented.
- LLVM/toolchain compatibility policy is documented.

Exit criteria:

- LLVM IR backend prototype lowers simple functions and programs.
- Function-block state is represented in native output and tested.
- Object/static/shared/executable output modes are specified and implemented incrementally.
- CI separates lightweight Rust/TypeScript jobs from LLVM-dependent jobs.
- VS Code Marketplace packaging and release checklist are documented.
- Confluence roadmap links to Jira epics.

## Deferred scope

IEC 61131-3 3rd edition object-oriented features are deferred until after Phase 4 foundations are stable. This includes classes, interfaces, methods beyond MVP needs, inheritance-oriented workflows, and vendor-specific OOP extensions.
