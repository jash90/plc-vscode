# PLC VS Code workspace architecture

This document defines the initial repository boundaries for PLC VS Code.

## Product boundaries

| Boundary | Path | Responsibility |
|---|---|---|
| `compiler_core` | `crates/plc_compiler_core` | Shared compiler API for CLI, LSP, runtime, and backends. |
| `syntax` | `crates/plc_syntax` | Lexer, error-tolerant parser, CST, and source ranges. |
| `semantic_analysis` | `crates/plc_semantics` | Symbol index, name resolution, IEC type model, and diagnostics. |
| `cli` | `crates/plc_cli` | Command-line parsing, diagnostics, execution, and compilation commands. |
| `lsp_server` | `crates/plc_lsp_server` | Language Server Protocol implementation. |
| `runtime` | `crates/plc_runtime` | PLC scan-cycle execution model and deterministic simulation. |
| `bytecode_vm` | `crates/plc_bytecode_vm` | Portable bytecode format and VM/interpreter. |
| `native_backend` | `crates/plc_native_backend` | LLVM/native code generation path. |
| `vscode_client` | `editors/vscode` | TypeScript VS Code extension client and packaging. |

## Dependency rule

The compiler core is the shared boundary. CLI, LSP, runtime, bytecode VM, and native backend consume it instead of duplicating parser or semantic logic.

## Delivery order

1. Foundation and guardrails: licensing policy, roadmap gates, developer setup, and IEC conformance scope.
2. Syntax layer: lexer, parser, CST, diagnostics.
3. Semantic layer: symbols, name resolution, type checking.
4. IDE layer: LSP diagnostics, completion, hover, navigation.
5. Runtime layer: scan-cycle interpreter/bytecode VM.
6. Native backend: LLVM IR and native artifacts.
7. Client packaging: VS Code distribution and bundled server binaries.

Supporting documents:

- [`../licensing.md`](../licensing.md) records allowed use of IronPLC and RuSTy references.
- [`../roadmap.md`](../roadmap.md) defines Phase 0 through Phase 4 milestone gates.
- [`../setup.md`](../setup.md) lists local build prerequisites for contributors.
- [`../conformance.md`](../conformance.md) maps IEC 61131-3 feature coverage across parser, semantics, and runtime.
