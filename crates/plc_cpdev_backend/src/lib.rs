//! CPDev `.XCP` bytecode compiler backend.
//!
//! This crate is the *emit* counterpart to [`plc_cpdev_vm`] (which *runs* `.XCP`
//! bytecode through the vendored C++ CPDev VM). It lowers a PLC program to CPDev
//! VM bytecode in two stages, mirroring CPDev's own compiler/assembler split:
//!
//! ```text
//! PLC IR  --codegen-->  CPDev symbolic assembly ([`asm::Program`])
//!         --assemble--> binary `.XCP` + sidecar `.DCP`
//! ```
//!
//! The [`asm`] layer is a small symbolic instruction IR; [`assembler::assemble`]
//! encodes it to the exact byte layout the VM loader expects. The binary format
//! was recovered from the vendored VM and the bundled `WeJeStSt` fixture:
//!
//! - An instruction is a 2-byte opcode (the 4-hex `vmcode`, emitted **big-endian**:
//!   `0x1C15` -> bytes `1C 15`) followed by its operands.
//! - Data/code addresses are 2-byte little-endian (16-bit addressing).
//! - Immediates are little-endian (`imm.BYTE`=1, `imm.WORD`=2, `imm.DWORD`=4, or a
//!   raw byte run sized by a sibling operand, as `MCD` uses).
//!
//! [`plc_cpdev_vm`]: ../plc_cpdev_vm/index.html
#![forbid(unsafe_code)]

pub mod asm;
pub mod assembler;
pub mod codegen;
pub mod dcp;
pub mod layout;
pub mod spec;
pub mod std_fb;
pub mod types;

pub use assembler::assemble;
pub use codegen::{Compiled, compile_source};
pub use spec::SpecTable;
pub use types::CpType;

/// The artifacts of compiling a program for the CPDev VM.
pub struct Artifacts {
    /// Binary `.XCP` bytecode.
    pub xcp: Vec<u8>,
    /// `.DCP` variable-map sidecar (XML).
    pub dcp: String,
    /// Human-readable assembly listing (for debugging / `--emit asm`).
    pub asm: String,
}

/// Compile Structured Text source to CPDev `.XCP` + `.DCP` (+ a debug listing).
pub fn compile(source: &str) -> Result<Artifacts, String> {
    let spec = SpecTable::load();
    let compiled = compile_source(source, &spec)?;
    finish(compiled)
}

/// Assemble a lowered program into the final artifacts.
fn finish(compiled: Compiled) -> Result<Artifacts, String> {
    let asm = compiled.program.to_string();
    let labels = assembler::label_offsets(&compiled.program)?;
    let body_code_addr = labels.get("TSKLOOP").copied().unwrap_or(0);
    let xcp = assemble(&compiled.program)?;
    let code_size = u16::try_from(xcp.len()).unwrap_or(u16::MAX);
    let dcp = dcp::render(&compiled, body_code_addr, code_size);
    Ok(Artifacts { xcp, dcp, asm })
}

/// Compile and return just the `.XCP` bytecode.
pub fn emit_xcp(source: &str) -> Result<Vec<u8>, String> {
    Ok(compile(source)?.xcp)
}

/// Compile and return just the `.DCP` variable map.
pub fn emit_dcp(source: &str) -> Result<String, String> {
    Ok(compile(source)?.dcp)
}

/// Compile and return just the debug assembly listing.
pub fn emit_asm(source: &str) -> Result<String, String> {
    Ok(compile(source)?.asm)
}
