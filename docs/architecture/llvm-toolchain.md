# LLVM toolchain compatibility

This document tracks the LLVM/`inkwell` toolchain constraints for the native
backend (PLC-36+) and the known failure modes so build environments and CI fail
clearly on an incompatible toolchain.

## Supported version matrix

The native backend uses [`inkwell`](https://crates.io/crates/inkwell), which
binds to a specific major LLVM version selected by a Cargo feature. The pinned
target for this repository is:

| Component   | Version            | Notes                                            |
| ----------- | ------------------ | ------------------------------------------------ |
| LLVM        | 18.x (18.1.8)      | Installed via `brew install llvm@18` (macOS).    |
| `inkwell`   | 0.5, feature `llvm18-0` | Must match the installed LLVM major version. |
| `llvm-sys`  | 180.x              | Pulled transitively by `inkwell`.                |

> Note: the default `brew install llvm` may install a newer major (e.g. 22) that
> `inkwell` does not yet support; pin `llvm@18` explicitly.

`inkwell`'s feature flag and the installed LLVM **major** version must agree. A
mismatch (e.g. `llvm18-1` against an installed LLVM 17) fails at build time in
`llvm-sys`'s build script, not at runtime.

## Environment setup

`llvm-sys` locates LLVM through `llvm-config`. On macOS/Homebrew the keg is not
linked into `PATH` by default, so the native-backend build expects:

```bash
brew install llvm@18 zstd
export LLVM_SYS_180_PREFIX="$(brew --prefix llvm@18)"
# LLVM 18 links against zstd; expose it (and the LLVM keg) to the linker:
export LIBRARY_PATH="$(brew --prefix zstd)/lib:$(brew --prefix llvm@18)/lib:$LIBRARY_PATH"
"$LLVM_SYS_180_PREFIX/bin/llvm-config" --version   # must print an 18.x version

# Build/test the isolated backend crate:
cargo test --manifest-path crates/plc_llvm_backend/Cargo.toml
```

The backend crate is excluded from the default workspace (`exclude` in the root
`Cargo.toml`), so `cargo test --workspace` stays LLVM-free; only the explicit
command above builds it.

## Known failure modes

- **`llvm-config` not found** — `llvm-sys` build script errors with "could not
  find llvm-config". Fix: install LLVM and/or set `LLVM_SYS_180_PREFIX`.
- **Major-version mismatch** — `llvm-sys` reports the detected version does not
  satisfy the crate's required range. Fix: align the `inkwell` feature with the
  installed LLVM major version.
- **Linker errors for `libLLVM`** — usually a static/shared mismatch; prefer the
  Homebrew keg's `lib` directory and a consistent `LLVM_SYS_*_PREFIX`.

## Output modes and cross-compilation

The backend (`plc_llvm_backend::compile`) supports these output modes:

- `LlvmIr` / `Assembly` — textual output.
- `Object` — host machine-code object bytes.
- `StaticLibrary` / `SharedLibrary` / `Executable` — emit object code as the
  compile step (shared output is position-independent); producing the final
  archive / shared object / executable is a subsequent **link** step that takes
  these object bytes as linker input.

Native emission targets the **host triple** by default
(`TargetMachine::get_default_triple()` + host CPU/features). Cross-compilation
requires:

- the target triple (e.g. `aarch64-unknown-linux-gnu`) and matching CPU/feature
  strings passed to the target machine,
- the corresponding LLVM target initialized (the prototype initializes the
  native target only; cross targets need `Target::initialize_all` or the
  specific target),
- a cross linker/sysroot for the link step of the linkable modes.

## CI isolation

The native-backend build and its golden IR tests are **gated behind an isolated
CI job** (see PLC-47) that installs the pinned LLVM and exports
`LLVM_SYS_180_PREFIX`. Keeping LLVM out of the default workspace job ensures the
rest of the suite stays fast and that an LLVM-toolchain problem produces a clear,
isolated failure rather than breaking unrelated checks.
