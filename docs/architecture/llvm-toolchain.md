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
| LLVM        | 18.x               | Installed via `brew install llvm` (macOS).       |
| `inkwell`   | feature `llvm18-1` | Must match the installed LLVM major version.     |
| `llvm-sys`  | matches LLVM major | Pulled transitively by `inkwell`.                |

`inkwell`'s feature flag and the installed LLVM **major** version must agree. A
mismatch (e.g. `llvm18-1` against an installed LLVM 17) fails at build time in
`llvm-sys`'s build script, not at runtime.

## Environment setup

`llvm-sys` locates LLVM through `llvm-config`. On macOS/Homebrew the keg is not
linked into `PATH` by default, so the native-backend build expects:

```bash
brew install llvm
export LLVM_SYS_180_PREFIX="$(brew --prefix llvm)"
# or ensure `$(brew --prefix llvm)/bin` is on PATH so llvm-config resolves
llvm-config --version   # must print an 18.x version
```

## Known failure modes

- **`llvm-config` not found** — `llvm-sys` build script errors with "could not
  find llvm-config". Fix: install LLVM and/or set `LLVM_SYS_180_PREFIX`.
- **Major-version mismatch** — `llvm-sys` reports the detected version does not
  satisfy the crate's required range. Fix: align the `inkwell` feature with the
  installed LLVM major version.
- **Linker errors for `libLLVM`** — usually a static/shared mismatch; prefer the
  Homebrew keg's `lib` directory and a consistent `LLVM_SYS_*_PREFIX`.

## CI isolation

The native-backend build and its golden IR tests are **gated behind an isolated
CI job** (see PLC-47) that installs the pinned LLVM and exports
`LLVM_SYS_180_PREFIX`. Keeping LLVM out of the default workspace job ensures the
rest of the suite stays fast and that an LLVM-toolchain problem produces a clear,
isolated failure rather than breaking unrelated checks.
