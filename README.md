# PLC VS Code

Structured Text / IEC 61131-3 VS Code extension and Rust toolchain.

## Architecture

The repository is organized around a shared Rust compiler core consumed by:

- CLI tooling,
- Language Server Protocol server,
- deterministic runtime/simulator,
- bytecode VM,
- native backend,
- TypeScript VS Code client.

See [`docs/architecture/workspace.md`](docs/architecture/workspace.md) for the initial workspace contract.

## Development

Run the architecture contract tests:

```bash
cargo test -p plc_workspace
```
