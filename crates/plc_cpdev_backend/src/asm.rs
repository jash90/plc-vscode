//! Symbolic CPDev assembly IR: the intermediate the codegen produces and the
//! [`assembler`](crate::assembler) consumes. It is deliberately small — a flat
//! list of labelled instructions — because the CPDev VM is a flat-code,
//! flat-data machine with no nesting at the bytecode level.

/// A resolved data-memory address (byte offset into the VM's data segment).
pub type Addr = u16;

/// A single instruction operand. Each variant is self-describing about its byte
/// width, so the assembler can encode an instruction without consulting the spec
/// table (the spec table drives *codegen*, which constructs these operands).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operand {
    /// A data address (`:rdlabel`/`:gdlabel`/...). Encoded as 2 bytes little-endian.
    Addr(Addr),
    /// A reference to a code label (`:gclabel` jump/call/loop target). Resolved by
    /// the assembler's backpatch pass to the label's 2-byte little-endian offset.
    Code(String),
    /// 1-byte immediate (`imm.BYTE`).
    ImmByte(u8),
    /// 2-byte little-endian immediate (`imm.WORD`).
    ImmWord(u16),
    /// 4-byte little-endian immediate (`imm.DWORD`).
    ImmDword(u32),
    /// A raw little-endian immediate byte run (`imm.*`, sized by a sibling operand —
    /// e.g. the value bytes of an `MCD`). Emitted verbatim.
    ImmBytes(Vec<u8>),
}

/// A single VM instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instr {
    /// The 16-bit VM opcode (the spec's 4-hex `vmcode`, e.g. `0x1C15`). For a
    /// variadic family the variant byte already encodes `(count << 4) | type`.
    /// Emitted big-endian (high byte first).
    pub vmcode: u16,
    /// Operands in source order, encoded immediately after the opcode.
    pub operands: Vec<Operand>,
    /// Optional mnemonic for human-readable rendering only; never encoded.
    pub mnemonic: Option<String>,
}

impl Instr {
    /// Construct an instruction from an opcode and its operands.
    pub fn new(vmcode: u16, operands: Vec<Operand>) -> Self {
        Self {
            vmcode,
            operands,
            mnemonic: None,
        }
    }

    /// Attach a mnemonic (for `emit_asm`/debugging); does not affect encoding.
    #[must_use]
    pub fn with_mnemonic(mut self, mnemonic: impl Into<String>) -> Self {
        self.mnemonic = Some(mnemonic.into());
        self
    }
}

/// An item in the instruction stream: a label binding, an instruction, or a
/// comment (comments are for rendering only and are skipped by the assembler).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// Binds a code label to the current byte offset (a jump/call target).
    Label(String),
    Instr(Instr),
    Comment(String),
}

/// A complete assembly program: a flat, ordered list of items.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Program {
    pub items: Vec<Item>,
}

impl std::fmt::Display for Operand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Operand::Addr(a) => write!(f, "@{a:04X}"),
            Operand::Code(l) => write!(f, ":{l}"),
            Operand::ImmByte(b) => write!(f, "#{b:02X}"),
            Operand::ImmWord(w) => write!(f, "#{w:04X}"),
            Operand::ImmDword(d) => write!(f, "#{d:08X}"),
            Operand::ImmBytes(bytes) => {
                f.write_str("#")?;
                for byte in bytes {
                    write!(f, "{byte:02X}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::fmt::Display for Program {
    /// A human-readable rendering for debugging (`emit_asm`); not the wire format.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for item in &self.items {
            match item {
                Item::Label(name) => writeln!(f, "{name}:")?,
                Item::Comment(text) => writeln!(f, "  ; {text}")?,
                Item::Instr(instr) => {
                    let mnem = instr.mnemonic.as_deref().unwrap_or("?");
                    write!(f, "  {mnem} [{:04X}]", instr.vmcode)?;
                    for (i, op) in instr.operands.iter().enumerate() {
                        write!(f, "{}{op}", if i == 0 { " " } else { ", " })?;
                    }
                    writeln!(f)?;
                }
            }
        }
        Ok(())
    }
}

impl Program {
    /// An empty program.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a code label binding the given name to the current position.
    pub fn label(&mut self, name: impl Into<String>) -> &mut Self {
        self.items.push(Item::Label(name.into()));
        self
    }

    /// Append an instruction.
    pub fn push(&mut self, instr: Instr) -> &mut Self {
        self.items.push(Item::Instr(instr));
        self
    }

    /// Append a render-only comment.
    pub fn comment(&mut self, text: impl Into<String>) -> &mut Self {
        self.items.push(Item::Comment(text.into()));
        self
    }
}
