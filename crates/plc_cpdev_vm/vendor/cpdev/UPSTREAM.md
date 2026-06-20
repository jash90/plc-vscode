# Vendored CPDev Virtual Machine

These C/C++ sources are vendored **unmodified** from the CPDev Control Program
Developer Virtual Machine. The POSIX/Linux platform port comes from
`Platforms/BeagleboneAI`; the Windows (MSVC) platform port comes from
`Platforms/Windows`. The shared VM core (`vm/`, `tinyxml2/`, `vm_variable.h`) is
byte-identical between the two upstream platform folders at the pinned commit.

- Upstream: https://github.com/CPDev-ControlProgramDeveloper/VirtualMachine
- Commit: `b8ccd91d6a47431c462b0c2935d1538dfc38a8e4`
- Vendored subtree (POSIX): `Platforms/BeagleboneAI/{vm/,tinyxml2/,vm_linux.{cpp,h},vm_variable.{cpp,h}}`
- Vendored subtree (Windows): `Platforms/Windows/{vm_windows.{cpp,h},vm_variable.cpp}` → kept under
  `windows/` here. Only the platform layer + the `vm_variable.cpp` file-I/O differ from POSIX
  (`<io.h>`/`_open`/`_read` + `QueryPerformanceCounter` vs `<unistd.h>`/`open`/`read` +
  `clock_gettime`); the shared `vm/`, `tinyxml2/`, and `vm_variable.h` are reused.
- Excluded: `test.cpp` (demo `main()`), the Visual Studio project, and the duplicate
  `Platforms/Windows/{vm/,tinyxml2/,vm_variable.h}` (identical to the shared copies).

Vendored with the upstream owner's permission. Do **not** edit these files; the
only integration code lives in `../../shim/` (a thin `extern "C"` wrapper) and
`../../src/` (the Rust `XcpEngine`). To update, re-copy the subtree at a new
upstream commit and record it here. `tinyxml2/LICENSE.txt` is the upstream
tinyxml2 license and is retained as required.

The compiled translation units are (selected per target OS in `../../build.rs`):

- `vm/vm.cpp` (shared)
- `vm/vmfunc/vm_data_access.cpp` (shared)
- `tinyxml2/tinyxml2.cpp` (shared)
- POSIX: `vm_linux.cpp` + `vm_variable.cpp`
- Windows: `windows/vm_windows.cpp` + `windows/vm_variable.cpp`

Everything else under `vm/` (`vmlib/`, `vmspec/`, `vmfunc/vm_stack.h`, the config
and register headers) is header-only and included textually. `vm_linux.h` and
`windows/vm_windows.h` are byte-identical (both declare `class VMLinux`), so the
shim includes `vm_linux.h` on every platform.
