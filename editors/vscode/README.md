# PLC VS Code client

TypeScript VS Code extension client for Structured Text / IEC 61131-3.

## Capabilities (MVP)

The extension talks to the Rust language server (`plc_lsp_server`) which is backed
by the shared `plc_compiler_core`. The current MVP provides:

- **Diagnostics** — syntax (lexer/parser) and semantic errors with stable codes
  (`PLC0001` unclosed block comment, `PLC0002` missing `END_PROGRAM`, `SEM0001`
  unresolved symbol, `SEM0002` assignment type mismatch).
- **Document outline** — hierarchical document symbols (POUs with nested variable
  declarations and IEC type detail).
- **Completion** — in-scope symbols plus Structured Text keywords.
- **Hover** — keyword help and symbol/type information.
- **Go-to-definition & find-references** — name-based navigation within a document.
- **Formatting** — whole-document and range formatting (keyword casing, block
  indentation, trailing-whitespace trim).
- **Quick fixes** — safe code actions such as adding a missing `END_PROGRAM`
  terminator.
- **Run** — execute a visible Structured Text file through the development runtime.

## Known limitations

- **Scope** — implementation targets the IEC 61131-3 baseline described in
  [`docs/conformance.md`](../../docs/conformance.md). The grammar and semantics are
  MVP-level; complex expressions, full call-signature checking, and exhaustive
  standard-library coverage are still in progress.
- **OOP deferred** — IEC object-oriented extensions (CLASS/METHOD/INTERFACE,
  inheritance) are intentionally out of scope for the MVP.
- **Vendor extensions** — non-standard vendor dialects are not supported.
- **Navigation** — definition/reference resolution is name-based within a single
  document; cross-file and member-scoped resolution is being expanded with the
  workspace symbol index.
- **Runtime/backend** — the deterministic runtime, bytecode VM, and native (LLVM)
  backend are under active development and not yet exposed as end-user features.

## Development run

From the repository root:

```bash
cargo build --workspace
cd editors/vscode
npm install
npm test
code --extensionDevelopmentPath="$PWD" /tmp/plc-vscode-smoke
```

Open a `.st` file to activate the extension and start the Rust LSP server via `cargo run --package plc_lsp_server --bin plc-lsp-server --`.
