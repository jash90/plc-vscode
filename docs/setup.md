# Developer setup and build prerequisites

This guide documents the local tools required to work on the Rust compiler core, CLI, LSP server, and VS Code extension client.

## Required tools

### Rust

- Install Rust with `rustup`: <https://rustup.rs/>.
- Use the stable toolchain unless a task explicitly documents otherwise.
- Required commands:
  - `cargo test --workspace`
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets`

### Node.js and npm

- Install an active LTS release of Node.js.
- The VS Code extension uses `npm` and TypeScript.
- From `editors/vscode`, run:

```bash
npm install
npm test
```

### Visual Studio Code

- Install VS Code for extension development and manual smoke tests.
- The extension contributes the `structured-text` language for `.st`, `.iecst`, and `.plcst` files.
- In development mode the extension launches the Rust LSP server through Cargo by default.

### LLVM and native backend prerequisites

LLVM is not required for the current lightweight compiler, CLI, LSP, or VS Code extension tests. It becomes required when working on the native backend and inkwell-based tasks.

When native backend work starts:

- Install the LLVM version documented by the backend task.
- Ensure `llvm-config` for the selected version is on `PATH`.
- Keep LLVM-dependent CI jobs separate from lightweight Rust/TypeScript jobs.

## Repository setup

From the repository root:

```bash
cargo test --workspace
```

For the VS Code extension:

```bash
cd editors/vscode
npm install
npm test
```

## CLI development

The CLI package is `plc_cli`.

Run tests:

```bash
cargo test -p plc_cli
```

Run a Structured Text file in development mode:

```bash
cargo run --package plc_cli -- run path/to/file.st
```

## LSP server development

The LSP package is `plc_lsp_server`.

Run tests:

```bash
cargo test -p plc_lsp_server
```

Start the server in development mode:

```bash
cargo run --package plc_lsp_server --bin plc-lsp-server --
```

The VS Code extension uses this command by default through the `plcVscode.serverCommand` and `plcVscode.serverArgs` settings.

## VS Code extension development

From `editors/vscode`:

```bash
npm install
npm test
npm run compile
```

Useful settings while developing locally:

- `plcVscode.repositoryRoot`: repository root used for Cargo commands.
- `plcVscode.serverCommand`: command used to launch the LSP server.
- `plcVscode.serverArgs`: arguments used to launch the LSP server.
- `plcVscode.cliCommand`: command used by the extension to execute Structured Text files.
- `plcVscode.cliArgs`: arguments prepended before the current file path for run commands.

## Troubleshooting

### Cargo cannot find a package

Run commands from the repository root, or set `plcVscode.repositoryRoot` to the repository root when testing through the extension.

### VS Code extension starts but language features do not appear

Check the `PLC VS Code` output channel and confirm the LSP server command works from a terminal:

```bash
cargo run --package plc_lsp_server --bin plc-lsp-server --
```

### TypeScript compile failures

Run `npm install` in `editors/vscode` and ensure the Node.js version is an active LTS release.

### LLVM version mismatch

Symptoms can include `llvm-sys` build failures, missing `llvm-config`, or inkwell version errors.

Fixes:

1. Confirm the backend task documents the supported LLVM major version.
2. Ensure the matching `llvm-config` appears first on `PATH`.
3. Re-run the failing command with verbose output.
4. If the issue is platform-specific, record it in the LLVM compatibility tracking task before changing backend code.
