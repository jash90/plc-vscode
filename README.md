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

Planning and contributor references:

- [`docs/licensing.md`](docs/licensing.md) — reference-project licensing rules for IronPLC and RuSTy.
- [`docs/roadmap.md`](docs/roadmap.md) — Phase 0 through Phase 4 delivery gates.
- [`docs/setup.md`](docs/setup.md) — local Rust, Node, VS Code, and LLVM setup notes.
- [`docs/conformance.md`](docs/conformance.md) — IEC 61131-3 baseline, deferred OOP scope, and coverage matrix.

## Development

Run the architecture contract tests:

```bash
cargo test -p plc_workspace
```
