//! Compiles the vendored CPDev C++ VM + the `extern "C"` shim into a static
//! library that `src/lib.rs` links against. macOS/Linux use clang/gcc with the
//! POSIX platform port (`vm_linux.cpp`); Windows uses MSVC with the upstream
//! Windows port (`windows/vm_windows.cpp`). The shared VM core is identical.
//!
//! The vendored sources are unchanged (see `vendor/cpdev/UPSTREAM.md`); all
//! integration lives in `shim/`. Warnings are silenced because the vendored
//! code emits deprecation/encoding warnings we do not own — they must not fail
//! our build.

use std::path::PathBuf;

fn main() {
    let vendor = PathBuf::from("vendor/cpdev");
    // The vendored VM has a POSIX port (vm_linux) and an upstream Windows port
    // (windows/vm_windows). Pick the platform layer + its matching vm_variable
    // file-I/O by target OS; the rest of the VM core is platform-agnostic.
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        // Shared, platform-agnostic translation units + our shim.
        .file(vendor.join("vm/vm.cpp"))
        .file(vendor.join("vm/vmfunc/vm_data_access.cpp"))
        .file(vendor.join("tinyxml2/tinyxml2.cpp"))
        .file("shim/cpdev_shim.cpp");

    if windows {
        // Upstream Windows port (Platforms/Windows): _open/_read, Win32 timing.
        build
            .file(vendor.join("windows/vm_windows.cpp"))
            .file(vendor.join("windows/vm_variable.cpp"));
    } else {
        // POSIX/Linux port (Platforms/BeagleboneAI).
        build
            .file(vendor.join("vm_linux.cpp"))
            .file(vendor.join("vm_variable.cpp"));
    }

    build
        // Include dirs mirror the upstream makefile's -I set (plus the shim).
        .include(&vendor)
        .include(vendor.join("vm"))
        .include(vendor.join("vm/vmfunc"))
        .include(vendor.join("vm/vmlib"))
        .include(vendor.join("vm/vmspec"))
        .include(vendor.join("tinyxml2"))
        .include("shim")
        // Belt-and-braces: keep unaligned VM-memory access correct everywhere
        // even though the vendored `__arm__` guard never fires here.
        .define("BYTE_ACCESS", None)
        // Vendored code has non-UTF-8 (cp1250) comments; do not let that
        // warn-fail (clang/gcc flag; cc skips it on MSVC).
        .flag_if_supported("-Wno-invalid-source-encoding")
        .warnings(false)
        .compile("cpdev_vm");

    println!("cargo:rerun-if-changed=shim/cpdev_shim.cpp");
    println!("cargo:rerun-if-changed=shim/cpdev_shim.h");
    println!("cargo:rerun-if-changed=vendor/cpdev");
}
