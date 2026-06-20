//! Pure-Rust codegen checks (no VM): lower ST -> CPDev artifacts and inspect the
//! structure. Executable behaviour is verified by the VM round-trip tests in
//! `plc_cpdev_vm`.

use plc_cpdev_backend::compile;

#[test]
fn lowers_counter_program() {
    let src = "PROGRAM main\nVAR n : INT; END_VAR\nn := n + 1;\nEND_PROGRAM";
    let art = compile(src).expect("compile counter");
    println!("--- ASM ---\n{}\n--- DCP ---\n{}", art.asm, art.dcp);
    assert!(!art.xcp.is_empty());
    assert!(art.dcp.contains("LName=\"n\""));
    assert!(art.dcp.contains("Type=\"INT\""));
}

#[test]
fn lowers_if_with_comparison_and_boolean() {
    let src = "PROGRAM main\n\
        VAR x : INT; flag : BOOL; END_VAR\n\
        IF x > 10 AND NOT flag THEN\n\
            flag := TRUE;\n\
        END_IF;\n\
        END_PROGRAM";
    let art = compile(src).expect("compile if");
    // Should assemble without undefined labels and declare both globals.
    assert!(!art.xcp.is_empty());
    assert!(art.dcp.contains("LName=\"flag\""));
}

#[test]
fn lowers_user_function_block() {
    let src = "FUNCTION_BLOCK Accum\n\
        VAR_INPUT step : INT; END_VAR\n\
        VAR_OUTPUT acc : INT; END_VAR\n\
        acc := acc + step;\n\
        END_FUNCTION_BLOCK\n\
        PROGRAM main\n\
        VAR a : Accum; o : INT; END_VAR\n\
        a(step := 1);\n\
        o := a.acc;\n\
        END_PROGRAM";
    let art = compile(src).expect("compile fb");
    assert!(art.asm.contains("FB_accum_CODE"), "asm: {}", art.asm);
    assert!(art.dcp.contains("LName=\"o\""));
}

#[test]
fn cyclic_function_blocks_are_rejected() {
    // Two FBs instantiating each other would need an infinite data frame.
    let src = "FUNCTION_BLOCK A\nVAR b : B; END_VAR\nEND_FUNCTION_BLOCK\n\
        FUNCTION_BLOCK B\nVAR a : A; END_VAR\nEND_FUNCTION_BLOCK\n\
        PROGRAM main\nVAR x : A; END_VAR\nEND_PROGRAM";
    match compile(src) {
        Ok(_) => panic!("cyclic FB instantiation must be rejected"),
        Err(err) => assert!(err.contains("cyclic"), "error was: {err}"),
    }
}

#[test]
fn self_instantiating_function_block_is_rejected() {
    let src = "FUNCTION_BLOCK A\nVAR self : A; END_VAR\nEND_FUNCTION_BLOCK\n\
        PROGRAM main\nVAR x : A; END_VAR\nEND_PROGRAM";
    match compile(src) {
        Ok(_) => panic!("self-instantiating FB must be rejected"),
        Err(err) => assert!(err.contains("itself"), "error was: {err}"),
    }
}

#[test]
fn string_assignment_compiles() {
    // STRING is laid out inline ([length][chars_size][padding:2][chars]); literal
    // assignment becomes a single MCD of that image.
    let src = "PROGRAM main\nVAR s : STRING[16]; END_VAR\ns := 'hi';\nEND_PROGRAM";
    let art = compile(src).expect("compile string program");
    assert!(art.dcp.contains("Type=\"STRING\""), "dcp: {}", art.dcp);
    // STRING[16] slot = 4 header + 16 capacity = 20 bytes.
    assert!(art.dcp.contains("Size=\"20\""), "dcp: {}", art.dcp);
}

#[test]
fn string_comparison_and_functions_compile() {
    let src = "PROGRAM main\n\
        VAR s : STRING[16]; b : BOOL; n : INT; head : STRING[16]; both : STRING[16]; END_VAR\n\
        b := s = 'x';\n\
        n := LEN(s);\n\
        head := LEFT(s, 2);\n\
        both := CONCAT(head, s);\n\
        END_PROGRAM";
    compile(src).expect("string comparison + LEN/LEFT/CONCAT should compile");
}

#[test]
fn string_arithmetic_is_rejected() {
    // `+` on strings is not IEC (use CONCAT); reject cleanly.
    let src = "PROGRAM main\nVAR a : STRING; b : STRING; END_VAR\na := a + b;\nEND_PROGRAM";
    match compile(src) {
        Ok(_) => panic!("string `+` must be rejected"),
        Err(err) => assert!(err.contains("string operators"), "error was: {err}"),
    }
}

#[test]
fn lowers_while_loop() {
    let src = "PROGRAM main\n\
        VAR i : INT; sum : INT; END_VAR\n\
        WHILE i < 3 DO\n\
            sum := sum + i;\n\
            i := i + 1;\n\
        END_WHILE;\n\
        END_PROGRAM";
    let art = compile(src).expect("compile while");
    assert!(!art.xcp.is_empty());
}
