//! `.DCP` sidecar writer.
//!
//! The `.DCP` is the variable map the VM loads alongside the `.XCP`. The C++
//! loader (`vm_variable.cpp`) and the Rust [`plc_cpdev_vm`] engine both read only
//! `CPDEV/TARGET/GLOBAL/VAR` (`LName`, `Addr` hex, `Size`, `Type`); the rest is
//! decorative tool metadata. We emit a `TASKS`/`MEMORY_MAP` block too for
//! tool-compatibility, but only the GLOBAL VARs gate loading + watching.
//!
//! [`plc_cpdev_vm`]: ../../plc_cpdev_vm/index.html

use std::fmt::Write;

use crate::codegen::Compiled;

/// The code byte offset where the cyclic task body begins (the `TSKLOOP` label).
/// Decorative for our VM round-trip, but emitted for tool-compatibility.
pub fn render(compiled: &Compiled, body_code_addr: u16, code_size: u16) -> String {
    let prog = &compiled.prog_name;
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"yes\"?>\n");
    out.push_str("<CPDEV version=\"1.0\">\n  <TARGET>\n    <GLOBAL>\n");
    for global in &compiled.globals {
        let _ = writeln!(
            out,
            "      <VAR LName=\"{name}\" PName=\"{prog}.{name}\" Addr=\"{addr:04X}\" AdrType=\"gdlabel\" Size=\"{size}\" Type=\"{ty}\" />",
            name = global.name,
            addr = global.addr,
            size = global.ty.size(),
            ty = global.ty.iec_name(),
        );
    }
    out.push_str("    </GLOBAL>\n    <TASKS>\n");
    let _ = writeln!(
        out,
        "      <TASK LName=\"Task0001\" PName=\"{prog}.Task0001\" LoopType=\"0\" Cycle=\"10\" CycleUnit=\"ms\" BodyCodeAddres=\"{body_code_addr:04X}\" TaskFlags=\"00000000\" />",
    );
    out.push_str("    </TASKS>\n");
    let _ = writeln!(
        out,
        "    <MEMORY_MAP Type=\"code\"><file LoadAddr=\"0\" Size=\"{code_size}\">{prog}.xcp</file></MEMORY_MAP>",
    );
    let _ = writeln!(
        out,
        "    <MEMORY_MAP Type=\"data\"><file LoadAddr=\"0\" Size=\"{}\">#dummy</file></MEMORY_MAP>",
        compiled.data_size,
    );
    out.push_str("  </TARGET>\n</CPDEV>\n");
    out
}
