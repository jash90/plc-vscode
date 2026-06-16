# PLC VS Code workspace architecture

This document defines the initial repository boundaries for PLC VS Code.

## Product boundaries

| Boundary | Path | Responsibility |
|---|---|---|
| `api` | `crates/plc_api` | Backend-agnostic ports (`LanguageService`, `ExecutionEngine`) and shared DTOs. Zero `plc_*` deps. The stable seam third parties implement. |
| `compiler_core` | `crates/plc_compiler_core` | Default `LanguageService` implementation (analyzer/IDE backend) over syntax + semantics. |
| `lang` | `crates/plc_lang` | Language-plugin registry (`LanguageFrontend`) and canonical-IR conversion hub. Reference frontends: ST (feature `st`) and IL (feature `il`). |
| `syntax` | `crates/plc_syntax` | Lexer, error-tolerant parser, CST, and source ranges. |
| `semantic_analysis` | `crates/plc_semantics` | Symbol index, name resolution, IEC type model, and diagnostics. |
| `cli` | `crates/plc_cli` | Command-line parsing, diagnostics, execution, and compilation commands. |
| `lsp_server` | `crates/plc_lsp_server` | Language Server Protocol implementation. |
| `runtime` | `crates/plc_runtime` | PLC scan-cycle execution model and deterministic simulation. |
| `bytecode_vm` | `crates/plc_bytecode_vm` | Portable bytecode format and VM/interpreter. |
| `native_backend` | `crates/plc_native_backend` | LLVM/native code generation path. |
| `vscode_client` | `editors/vscode` | TypeScript VS Code extension client and packaging. |

## Dependency rule

`plc_api` is the dependency-free contract at the center (ports + DTOs). Frontends
depend on the ports, not on a concrete backend: `plc_lsp_server` holds an
`Arc<dyn LanguageService>` and `plc_cli`'s `run_with` takes `&dyn LanguageService`
+ `&mut dyn ExecutionEngine`. `plc_compiler_core` is the default analyzer backend
and re-exports the DTOs from `plc_api` for back-compat; it deliberately has **no**
runtime dependency. `plc_runtime` is the default execution backend
(`ScanRuntimeEngine`). Syntax and semantics remain the shared parsing/analysis
core that the default backend builds on.

## Pluggable backends (ports & adapters)

The workspace is split so a third party can plug in their own LSP backend or
compiler/runtime without forking the provided crates — they depend only on
`plc_api`:

- **Bring your own analyzer / LSP backend** — implement
  `plc_api::LanguageService` (13 methods: diagnostics, symbols, semantic tokens,
  completion, hover, signature help, definition/references, formatting, code
  actions) and pass it to the provided tower-lsp host via
  `PlcLanguageServer::with_service(client, Arc::new(MyBackend))`. The stock
  server is the same call with `Arc::new(CompilerCore)`.
- **Bring your own compiler / runtime** — implement
  `plc_api::ExecutionEngine` (load, set scan interval, run scans, set input,
  watch) and drive the CLI flow with `plc_cli::run_with(&service, &mut engine,
  &document, scans)`. The stock CLI uses `plc_runtime::ScanRuntimeEngine`; an
  LLVM-JIT or remote-PLC engine slots in with a one-line type change.

Both ports are object-safe (`dyn`-compatible). Proof tests:
`crates/plc_lsp_server/tests/pluggable_backend.rs`,
`crates/plc_cli/tests/pluggable_engine.rs`, `crates/plc_runtime/tests/engine.rs`.

## Adding a language & converting between languages

`plc_lang` makes the project ready for the other IEC 61131-3 languages (IL, LD,
FBD, SFC) and for transpilation, using `plc_hir` as a **canonical IR hub**:

- **Add a language** — implement `plc_lang::LanguageFrontend` (object-safe:
  `id`/`display_name`/`extensions`, `lower(source) -> IR`, and the defaulted
  `render`/`can_render`/`analyze`/`language_service`) and `register` it. The LSP
  becomes language-aware via `PlcLanguageServer::with_registry`, the CLI via
  `run_with_registry`; both select the frontend by file extension.
- **Convert** — `LanguageRegistry::convert(from, to, doc)` lowers with `from` and
  renders with `to` through the IR, so cost is N lowerers + M renderers (not N²
  pairwise). `plc convert <from-id> <to-id> <file>` exposes it on the CLI
  (`plc languages` lists ids). Unsupported targets, source errors, and unknown
  languages return a `ConversionError` (never wrong output); constructs the IR
  does not model are surfaced as **fidelity notes**.

Reference frontends: **ST** (lowers via `plc_hir::lower_source`, analyzes via
`CompilerCore`, renders IR→ST) and **IL** (renders IR→accumulator mnemonics
matching the bytecode golden, parses IL→IR as the inverse). Today's faithful
conversion subset is exactly what the IR models (POU declarations + assignment
bodies over `Int/Real/Bool/Str/Var` and `+`/`-`).

**Graphical languages** (LD/FBD/SFC) fit the same trait later: a graphical
frontend lowers a serialized diagram (e.g. PLCopen XML carried as document text)
into the executable IR and renders back; positional/graph metadata is added to
`plc_hir` *additively* (a `None`-defaulted overlay field) when those land, so the
textual MVP and existing tests are unaffected. Proof tests:
`crates/plc_lang/tests/conversion_st_il.rs`, `crates/plc_cli/tests/language_aware.rs`.

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
