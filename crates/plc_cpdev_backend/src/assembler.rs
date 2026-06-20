//! Two-pass encoder: [`asm::Program`] -> binary `.XCP` bytes.
//!
//! Pass 1 walks the items to bind every code [`Label`](asm::Item::Label) to its
//! byte offset (computing each instruction's encoded length from its operands).
//! Pass 2 emits the bytes, backpatching each [`Operand::Code`] reference to its
//! resolved label offset.
//!
//! Encoding rules (recovered from the vendored VM + the `WeJeStSt` fixture):
//! - opcode: the 16-bit `vmcode`, **big-endian** (`0x1C15` -> `1C 15`).
//! - address / code-label operand: 2 bytes little-endian ([`ADDR_WIDTH`]).
//! - immediates: little-endian, width per operand variant.

use std::collections::HashMap;

use crate::asm::{Item, Operand, Program};

/// Width in bytes of a data/code address operand. The vendored VM is built with
/// 16-bit addressing (`vm_cfg.h` leaves `VM_ADDRESSING_32` undefined), so this is
/// 2. Kept as a single constant so a future 32-bit build is a one-line change.
pub const ADDR_WIDTH: usize = 2;

/// Encoded byte length of a single operand.
fn operand_len(op: &Operand) -> usize {
    match op {
        Operand::Addr(_) | Operand::Code(_) => ADDR_WIDTH,
        Operand::ImmByte(_) => 1,
        Operand::ImmWord(_) => 2,
        Operand::ImmDword(_) => 4,
        Operand::ImmBytes(bytes) => bytes.len(),
    }
}

/// Encoded byte length of an instruction: opcode (2) + all operands.
fn instr_len(operands: &[Operand]) -> usize {
    2 + operands.iter().map(operand_len).sum::<usize>()
}

/// Assemble a symbolic program into CPDev `.XCP` bytecode.
///
/// Errors if a referenced code label is undefined or if the code segment grows
/// past the 16-bit address space.
pub fn assemble(program: &Program) -> Result<Vec<u8>, String> {
    let labels = bind_labels(program)?;

    let mut out = Vec::new();
    for item in &program.items {
        let Item::Instr(instr) = item else {
            continue;
        };
        out.extend_from_slice(&instr.vmcode.to_be_bytes());
        for op in &instr.operands {
            match op {
                Operand::Addr(addr) => out.extend_from_slice(&addr.to_le_bytes()[..ADDR_WIDTH]),
                Operand::ImmWord(word) => out.extend_from_slice(&word.to_le_bytes()),
                Operand::ImmDword(dword) => out.extend_from_slice(&dword.to_le_bytes()),
                Operand::ImmByte(byte) => out.push(*byte),
                Operand::ImmBytes(bytes) => out.extend_from_slice(bytes),
                Operand::Code(label) => {
                    let addr = labels
                        .get(label.as_str())
                        .ok_or_else(|| format!("undefined code label `{label}`"))?;
                    out.extend_from_slice(&addr.to_le_bytes()[..ADDR_WIDTH]);
                }
            }
        }
    }
    Ok(out)
}

/// The byte offset of every code label, by name. Useful for filling decorative
/// `.DCP` fields like a task's `BodyCodeAddres`.
pub fn label_offsets(program: &Program) -> Result<HashMap<String, u16>, String> {
    Ok(bind_labels(program)?
        .into_iter()
        .map(|(name, addr)| (name.to_owned(), addr))
        .collect())
}

/// Pass 1: bind each code label to its byte offset in the code segment.
fn bind_labels(program: &Program) -> Result<HashMap<&str, u16>, String> {
    let mut labels = HashMap::new();
    let mut offset: usize = 0;
    for item in &program.items {
        match item {
            Item::Label(name) => {
                let addr = u16::try_from(offset).map_err(|_| {
                    format!(
                        "code offset {offset} exceeds the 16-bit address space at label `{name}`"
                    )
                })?;
                labels.insert(name.as_str(), addr);
            }
            Item::Instr(instr) => offset += instr_len(&instr.operands),
            Item::Comment(_) => {}
        }
    }
    Ok(labels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::{Instr, Operand, Program};

    #[test]
    fn opcode_is_big_endian() {
        // 0x1C15 (MCD) must serialize high-byte-first so the VM's GetProgramWord
        // (LE) + `wCmd = word & 0xFF` reconstruct function group 0x1C.
        let mut p = Program::new();
        p.push(Instr::new(0x1C13, vec![])); // RETURN, no operands
        assert_eq!(assemble(&p).unwrap(), vec![0x1C, 0x13]);
    }

    #[test]
    fn operands_are_little_endian() {
        let mut p = Program::new();
        p.push(Instr::new(
            0x1C15,
            vec![
                Operand::Addr(0x0006),
                Operand::ImmByte(0x02),
                Operand::ImmBytes(vec![0xD0, 0x07]),
            ],
        ));
        // 1c 15 | 06 00 | 02 | d0 07
        assert_eq!(
            assemble(&p).unwrap(),
            vec![0x1C, 0x15, 0x06, 0x00, 0x02, 0xD0, 0x07]
        );
    }

    #[test]
    fn backpatches_forward_and_backward_labels() {
        let mut p = Program::new();
        p.label("top"); // offset 0
        p.push(Instr::new(0x1C00, vec![Operand::Code("end".into())])); // JMP end (4 bytes: 0..4)
        p.push(Instr::new(0x1C00, vec![Operand::Code("top".into())])); // JMP top (4 bytes: 4..8)
        p.label("end"); // offset 8
        let bytes = assemble(&p).unwrap();
        // JMP end -> 0x0008 ; JMP top -> 0x0000
        assert_eq!(bytes, vec![0x1C, 0x00, 0x08, 0x00, 0x1C, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn undefined_label_errors() {
        let mut p = Program::new();
        p.push(Instr::new(0x1C00, vec![Operand::Code("missing".into())]));
        assert!(assemble(&p).is_err());
    }
}
