//! Compiles the vendored CPDev C++ VM + the `extern "C"` shim into a static
//! library that `src/lib.rs` links against. macOS/clang + libc++.
//!
//! The vendored sources are unchanged (see `vendor/cpdev/UPSTREAM.md`); all
//! integration lives in `shim/`. Warnings are silenced because the vendored
//! code emits deprecation/encoding warnings we do not own — they must not fail
//! our build.

use std::path::PathBuf;

fn main() {
    let vendor = PathBuf::from("vendor/cpdev");

    cc::Build::new()
        .cpp(true)
        .std("c++17")
        // The five compiled translation units (BeagleboneAI POSIX port).
        .file(vendor.join("vm/vm.cpp"))
        .file(vendor.join("vm/vmfunc/vm_data_access.cpp"))
        .file(vendor.join("vm_linux.cpp"))
        .file(vendor.join("vm_variable.cpp"))
        .file(vendor.join("tinyxml2/tinyxml2.cpp"))
        // Our shim.
        .file("shim/cpdev_shim.cpp")
        // Include dirs mirror the upstream makefile's -I set (plus the shim).
        .include(&vendor)
        .include(vendor.join("vm"))
        .include(vendor.join("vm/vmfunc"))
        .include(vendor.join("vm/vmlib"))
        .include(vendor.join("vm/vmspec"))
        .include(vendor.join("tinyxml2"))
        .include("shim")
        // Belt-and-braces: keep unaligned VM-memory access correct on macOS
        // arm64/x86_64 even though the `__arm__` guard never fires here.
        .define("BYTE_ACCESS", None)
        // Vendored code has non-UTF-8 (cp1250) comments; do not let that warn-fail.
        .flag_if_supported("-Wno-invalid-source-encoding")
        .warnings(false)
        .compile("cpdev_vm");

    println!("cargo:rerun-if-changed=shim/cpdev_shim.cpp");
    println!("cargo:rerun-if-changed=shim/cpdev_shim.h");
    println!("cargo:rerun-if-changed=vendor/cpdev");
}
