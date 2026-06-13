# PLC VS Code client

TypeScript VS Code extension client for Structured Text / IEC 61131-3.

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
