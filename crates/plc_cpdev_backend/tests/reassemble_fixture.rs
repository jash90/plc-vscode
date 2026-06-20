//! Phase-0 gate: prove the spec-driven assembler produces byte-exact CPDev
//! bytecode by transcribing the bundled `WeJeStSt` program (a 4-LED rotating
//! blinker, source of the `.dcp` MNEMONIC_MAP) into the symbolic [`asm`] IR and
//! asserting it re-assembles to the original `.xcp` byte-for-byte.
//!
//! This burns the highest-risk encoding details (opcode big-endianness,
//! little-endian addresses, the MCD immediate run, the variadic count nibble,
//! and two-pass code-label backpatching) with no C++ VM in the loop.

use plc_cpdev_backend::asm::{Instr, Operand, Program};
use plc_cpdev_backend::assemble;

/// The original program (vendored from upstream CPDev, binary).
const WEJESTST_XCP: &[u8] = include_bytes!("fixtures/WeJeStSt.xcp");

// --- opcodes used by WeJeStSt (cross-checked against vendored vmdef.h) ---
const MCD: u16 = 0x1C15; // move code immediate -> data
const CALB: u16 = 0x1C16; // call block
const TRML: u16 = 0x1C1D; // terminate cycle / set loop point
const RETURN: u16 = 0x1C13;
const JZ: u16 = 0x1C02; // jump if zero
const JMP: u16 = 0x1C00;
const MEMCP: u16 = 0x1C1F; // data -> data copy
const CUR_TIME: u16 = 0x1C17;
const NOP: u16 = 0x0000;
const NOT_BOOL: u16 = 0x0510;
const EQ_INT: u16 = 0x1202;
const GE_TIME: u16 = 0x110B;
const LT_INT: u16 = 0x1402;
const SUB_TIME: u16 = 0x020B;
const MUL_INT2: u16 = 0x0322; // MUL, 2 source operands, type INT

// --- data addresses (from the .dcp GLOBAL + DATA_MAP) ---
const OUT0: u16 = 0x00;
const OUT1: u16 = 0x01;
const OUT2: u16 = 0x02;
const OUT3: u16 = 0x03;
const ONOF: u16 = 0x04;
const PONOF: u16 = 0x05;
const LICZNIK: u16 = 0x06;
const STIME: u16 = 0x08;
const BCOUNT: u16 = 0x0C;
const IFCTL: u16 = 0x0D;
const ANDA003A: u16 = 0x0E;
const CST0044: u16 = 0x10; // INT 0
const CST0051: u16 = 0x12; // INT 8
const CST004A: u16 = 0x14; // TIME 2000ms
const CUR_TIME004B: u16 = 0x18;
const SUB004C: u16 = 0x1C;
const CST0054: u16 = 0x20; // INT 2
const CST0059: u16 = 0x22; // INT 1
const CST005E: u16 = 0x24; // INT 4

fn mcd(dst: u16, value: &[u8]) -> Instr {
    Instr::new(
        MCD,
        vec![
            Operand::Addr(dst),
            Operand::ImmByte(value.len() as u8),
            Operand::ImmBytes(value.to_vec()),
        ],
    )
}

fn jz(cond: u16, target: &str) -> Instr {
    Instr::new(JZ, vec![Operand::Addr(cond), Operand::Code(target.into())])
}

fn jmp(target: &str) -> Instr {
    Instr::new(JMP, vec![Operand::Code(target.into())])
}

fn calb(delta: u16, target: &str) -> Instr {
    Instr::new(
        CALB,
        vec![Operand::ImmWord(delta), Operand::Code(target.into())],
    )
}

fn bin3(vmcode: u16, dst: u16, a: u16, b: u16) -> Instr {
    Instr::new(
        vmcode,
        vec![Operand::Addr(dst), Operand::Addr(a), Operand::Addr(b)],
    )
}

fn not2(dst: u16, src: u16) -> Instr {
    Instr::new(NOT_BOOL, vec![Operand::Addr(dst), Operand::Addr(src)])
}

fn memcp(dst: u16, src: u16, count: u16) -> Instr {
    Instr::new(
        MEMCP,
        vec![
            Operand::Addr(dst),
            Operand::Addr(src),
            Operand::ImmWord(count),
        ],
    )
}

fn cur_time(dst: u16) -> Instr {
    Instr::new(CUR_TIME, vec![Operand::Addr(dst)])
}

fn nop() -> Instr {
    Instr::new(NOP, vec![])
}

fn ret() -> Instr {
    Instr::new(RETURN, vec![])
}

/// Transcribe WeJeStSt into the symbolic IR (jump targets as code labels so the
/// assembler's backpatch — not hard-coded offsets — resolves them).
fn wejestst_program() -> Program {
    let mut p = Program::new();

    // Global zero-init prologue (one MCD per global BOOL).
    p.push(mcd(OUT0, &[0]));
    p.push(mcd(OUT1, &[0]));
    p.push(mcd(OUT2, &[0]));
    p.push(mcd(OUT3, &[0]));
    p.push(mcd(ONOF, &[0]));

    // Task scaffold: run INIT once, then loop CODE, terminating each cycle at the
    // loop label (TRML pins the PC there so INIT is not re-run).
    p.label("TSKSTR");
    p.push(calb(0, "INIT"));
    p.label("TSKLOOP");
    p.push(calb(0, "CODE"));
    p.push(Instr::new(TRML, vec![Operand::Code("TSKLOOP".into())]));

    // Program INIT: materialize every local + constant, then return.
    p.label("INIT");
    p.push(mcd(LICZNIK, &[0x00, 0x00])); // INT 0
    p.push(mcd(STIME, &[0, 0, 0, 0])); // TIME 0
    p.push(mcd(PONOF, &[0x01])); // BOOL TRUE
    p.push(mcd(BCOUNT, &[0x01])); // BOOL TRUE
    p.push(mcd(CST0044, &[0x00, 0x00])); // INT 0
    p.push(mcd(CST004A, &[0xD0, 0x07, 0x00, 0x00])); // TIME 2000ms (t#2s)
    p.push(mcd(CST0051, &[0x08, 0x00])); // INT 8
    p.push(mcd(CST0054, &[0x02, 0x00])); // INT 2
    p.push(mcd(CST0059, &[0x01, 0x00])); // INT 1
    p.push(mcd(CST005E, &[0x04, 0x00])); // INT 4
    p.push(ret());

    // Program CODE: the cyclic body.
    p.label("CODE");
    // IF ONOF AND NOT pONOF THEN bCOUNT := NOT bCOUNT; END_IF;
    p.push(jz(ONOF, "AND0039"));
    p.push(not2(ANDA003A, PONOF));
    p.push(jz(ANDA003A, "AND0039"));
    p.push(mcd(IFCTL, &[0x01]));
    p.push(jmp("EAND003D"));
    p.label("AND0039");
    p.push(mcd(IFCTL, &[0x00]));
    p.label("EAND003D");
    p.push(jz(IFCTL, "B0038"));
    p.push(not2(BCOUNT, BCOUNT));
    p.label("B0038");
    p.push(nop());
    // pONOF := ONOF;
    p.push(memcp(PONOF, ONOF, 1));
    // IF bCOUNT THEN ...
    p.push(jz(BCOUNT, "B0042"));
    // IF Licznik = 0 THEN sTime := CUR_TIME(); Licznik := 1; END_IF;
    p.push(bin3(EQ_INT, IFCTL, LICZNIK, CST0044));
    p.push(jz(IFCTL, "B0043"));
    p.push(cur_time(STIME));
    p.push(mcd(LICZNIK, &[0x01, 0x00]));
    p.label("B0043");
    // IF CUR_TIME() - sTime >= t#2s THEN ...
    p.push(cur_time(CUR_TIME004B));
    p.push(bin3(SUB_TIME, SUB004C, CUR_TIME004B, STIME));
    p.push(bin3(GE_TIME, IFCTL, SUB004C, CST004A));
    p.push(jz(IFCTL, "B0049"));
    p.push(cur_time(STIME));
    // IF Licznik < 8 THEN Licznik := Licznik * 2; ELSE Licznik := 1; END_IF;
    p.push(bin3(LT_INT, IFCTL, LICZNIK, CST0051));
    p.push(jz(IFCTL, "B0050"));
    p.push(bin3(MUL_INT2, LICZNIK, LICZNIK, CST0054));
    p.push(jmp("E0056"));
    p.label("B0050");
    p.push(mcd(LICZNIK, &[0x01, 0x00]));
    p.label("E0056");
    p.label("B0049");
    p.label("B0042");
    p.push(nop());
    // OUT0..OUT3 := Licznik = {1,2,4,8};
    p.push(bin3(EQ_INT, OUT0, LICZNIK, CST0059));
    p.push(bin3(EQ_INT, OUT1, LICZNIK, CST0054));
    p.push(bin3(EQ_INT, OUT2, LICZNIK, CST005E));
    p.push(bin3(EQ_INT, OUT3, LICZNIK, CST0051));
    p.push(ret());

    p
}

/// Compare with a helpful first-difference report (a raw slice `assert_eq!` of
/// 306 bytes is unreadable on failure).
fn assert_bytes_eq(got: &[u8], want: &[u8]) {
    if got == want {
        return;
    }
    let n = got.len().min(want.len());
    let first = (0..n).find(|&i| got[i] != want[i]).unwrap_or(n);
    let end_g = (first + 8).min(got.len());
    let end_w = (first + 8).min(want.len());
    panic!(
        "byte mismatch: got {} bytes, want {} bytes; first diff at 0x{first:02x}:\n  got  {:02x?}\n  want {:02x?}",
        got.len(),
        want.len(),
        &got[first..end_g],
        &want[first..end_w],
    );
}

#[test]
fn reassembles_wejestst_byte_exact() {
    let bytes = assemble(&wejestst_program()).expect("assemble WeJeStSt");
    assert_eq!(bytes.len(), WEJESTST_XCP.len(), "code segment length");
    assert_bytes_eq(&bytes, WEJESTST_XCP);
}
