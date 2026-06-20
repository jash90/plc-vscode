# Vendored CPDev Virtual Machine

These C/C++ sources are vendored **unmodified** from the CPDev Control Program
Developer Virtual Machine, POSIX/Linux port (`Platforms/BeagleboneAI`).

- Upstream: https://github.com/CPDev-ControlProgramDeveloper/VirtualMachine
- Commit: `b8ccd91d6a47431c462b0c2935d1538dfc38a8e4`
- Vendored subtree: `Platforms/BeagleboneAI/{vm/,tinyxml2/,vm_linux.{cpp,h},vm_variable.{cpp,h}}`
- Excluded: `test.cpp` (demo `main()` writing to Linux sysfs LEDs — not a library unit).

Vendored with the upstream owner's permission. Do **not** edit these files; the
only integration code lives in `../../shim/` (a thin `extern "C"` wrapper) and
`../../src/` (the Rust `XcpEngine`). To update, re-copy the subtree at a new
upstream commit and record it here. `tinyxml2/LICENSE.txt` is the upstream
tinyxml2 license and is retained as required.

The compiled translation units are exactly:

- `vm/vm.cpp`
- `vm/vmfunc/vm_data_access.cpp`
- `vm_linux.cpp`
- `vm_variable.cpp`
- `tinyxml2/tinyxml2.cpp`

Everything else under `vm/` (`vmlib/`, `vmspec/`, `vmfunc/vm_stack.h`, the config
and register headers) is header-only and included textually.
