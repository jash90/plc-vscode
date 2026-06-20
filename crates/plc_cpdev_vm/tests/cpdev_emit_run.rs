//! End-to-end round-trip: compile ST -> CPDev `.XCP`/`.DCP` with the backend,
//! then load + run it through the vendored VM and assert the watch snapshot.
//! This closes the loop the byte-exact assembler test cannot: it proves the
//! emitted bytecode actually executes to the expected state.

use plc_api::{ExecutionEngine, SourceDocument};
use plc_cpdev_vm::XcpEngine;

/// Compile `src`, write the `.xcp`/`.dcp` pair to a temp dir, load via the engine
/// and run `scans` cycles, returning the watch snapshot.
fn emit_run_watch(src: &str, name: &str, scans: u64) -> Vec<String> {
    let artifacts = plc_cpdev_backend::compile(src).expect("backend compile");

    let dir = tempfile::tempdir().unwrap();
    let xcp_path = dir.path().join(format!("{name}.xcp"));
    std::fs::write(&xcp_path, &artifacts.xcp).unwrap();
    std::fs::write(dir.path().join(format!("{name}.dcp")), &artifacts.dcp).unwrap();
    let uri = format!("file://{}", xcp_path.display());

    let mut engine = XcpEngine::default();
    engine
        .load_artifact(&artifacts.xcp, &uri)
        .expect("load emitted artifact");
    engine.run_scans(scans);
    engine.watch()
}

#[test]
fn counter_increments_once_per_scan() {
    let src = "PROGRAM main\nVAR n : INT; END_VAR\nn := n + 1;\nEND_PROGRAM";
    assert_eq!(emit_run_watch(src, "main", 7), vec!["n = 7".to_owned()]);
}

#[test]
fn add_and_sub_over_globals() {
    let src = "PROGRAM main\nVAR a : INT; b : INT; END_VAR\na := a + 2;\nb := a - 1;\nEND_PROGRAM";
    let watch = emit_run_watch(src, "main", 5);
    assert!(watch.contains(&"a = 10".to_owned()), "watch: {watch:?}");
    assert!(watch.contains(&"b = 9".to_owned()), "watch: {watch:?}");
}

#[test]
fn comparison_and_boolean_drive_an_output() {
    // out := (x = 3) ; with x seeded to 3 via repeated increment, then a guard.
    let src = "PROGRAM main\n\
        VAR x : INT; hit : BOOL; END_VAR\n\
        x := x + 1;\n\
        hit := x = 3;\n\
        END_PROGRAM";
    // After 3 scans x = 3, so hit becomes TRUE on scan 3.
    let watch = emit_run_watch(src, "main", 3);
    assert!(watch.contains(&"x = 3".to_owned()), "watch: {watch:?}");
    assert!(watch.contains(&"hit = TRUE".to_owned()), "watch: {watch:?}");
    // On scan 4, x = 4, hit goes back to FALSE.
    let watch4 = emit_run_watch(src, "main", 4);
    assert!(
        watch4.contains(&"hit = FALSE".to_owned()),
        "watch: {watch4:?}"
    );
}

#[test]
fn if_elsif_else_selects_branch() {
    // Classify x into buckets; with x incremented each scan, check the boundary.
    let src = "PROGRAM main\n\
        VAR x : INT; band : INT; END_VAR\n\
        x := x + 1;\n\
        IF x < 3 THEN\n\
            band := 1;\n\
        ELSIF x < 6 THEN\n\
            band := 2;\n\
        ELSE\n\
            band := 3;\n\
        END_IF;\n\
        END_PROGRAM";
    assert!(emit_run_watch(src, "main", 2).contains(&"band = 1".to_owned())); // x=2
    assert!(emit_run_watch(src, "main", 5).contains(&"band = 2".to_owned())); // x=5
    assert!(emit_run_watch(src, "main", 9).contains(&"band = 3".to_owned())); // x=9
}

#[test]
fn while_loop_sums_within_one_scan() {
    // sum := 0+1+2 = 3, i ends at 3, computed in a single scan's WHILE loop.
    let src = "PROGRAM main\n\
        VAR i : INT; sum : INT; END_VAR\n\
        i := 0;\n\
        sum := 0;\n\
        WHILE i < 3 DO\n\
            sum := sum + i;\n\
            i := i + 1;\n\
        END_WHILE;\n\
        END_PROGRAM";
    let watch = emit_run_watch(src, "main", 1);
    assert!(watch.contains(&"sum = 3".to_owned()), "watch: {watch:?}");
    assert!(watch.contains(&"i = 3".to_owned()), "watch: {watch:?}");
}

#[test]
fn for_loop_accumulates() {
    // sum := 1+2+3+4 = 10 in one scan via a FOR loop.
    let src = "PROGRAM main\n\
        VAR k : INT; sum : INT; END_VAR\n\
        sum := 0;\n\
        FOR k := 1 TO 4 DO\n\
            sum := sum + k;\n\
        END_FOR;\n\
        END_PROGRAM";
    let watch = emit_run_watch(src, "main", 1);
    assert!(watch.contains(&"sum = 10".to_owned()), "watch: {watch:?}");
}

#[test]
fn case_selects_by_value_and_range() {
    let src = "PROGRAM main\n\
        VAR sel : INT; out : INT; END_VAR\n\
        sel := sel + 1;\n\
        CASE sel OF\n\
            1: out := 10;\n\
            2: out := 20;\n\
            3, 4: out := 30;\n\
        ELSE\n\
            out := 99;\n\
        END_CASE;\n\
        END_PROGRAM";
    assert!(emit_run_watch(src, "c", 1).contains(&"out = 10".to_owned()));
    assert!(emit_run_watch(src, "c", 2).contains(&"out = 20".to_owned()));
    assert!(emit_run_watch(src, "c", 4).contains(&"out = 30".to_owned())); // 4 is in 3,4
    assert!(emit_run_watch(src, "c", 5).contains(&"out = 99".to_owned())); // else
}

#[test]
fn dint_uses_full_width() {
    // 100000 overflows INT (16-bit); DINT must use a 4-byte slot and ADD.
    let src = "PROGRAM main\nVAR d : DINT; END_VAR\nd := d + 100000;\nEND_PROGRAM";
    assert_eq!(emit_run_watch(src, "d", 3), vec!["d = 300000".to_owned()]);
}

#[test]
fn real_arithmetic_and_watch_formatting() {
    let src = "PROGRAM main\nVAR r : REAL; END_VAR\nr := r + 0.5;\nEND_PROGRAM";
    // 0.5 * 4 = 2.0, rendered CODESYS-style with a trailing .0.
    assert_eq!(emit_run_watch(src, "r", 4), vec!["r = 2.0".to_owned()]);
}

#[test]
fn wejestst_blinker_compiles_and_runs() {
    // The original WeJeStSt program (a 4-LED rotating blinker), reconstructed
    // from the fixture's `.dcp` source comments and compiled by OUR backend.
    // Exercises: nested IF/ELSE, AND/NOT, `=`/`<`/`>=`, MUL, TIME subtraction,
    // the CUR_TIME() intrinsic, and a t#2s literal — i.e. real-program coverage.
    let src = "PROGRAM main\n\
        VAR\n\
            OUT0 : BOOL;\n\
            OUT1 : BOOL;\n\
            OUT2 : BOOL;\n\
            OUT3 : BOOL;\n\
            ONOF : BOOL;\n\
            Licznik : INT;\n\
            sTime : TIME;\n\
            pONOF : BOOL := TRUE;\n\
            bCOUNT : BOOL := TRUE;\n\
        END_VAR\n\
        IF ONOF AND NOT pONOF THEN\n\
            bCOUNT := NOT bCOUNT;\n\
        END_IF;\n\
        pONOF := ONOF;\n\
        IF bCOUNT THEN\n\
            IF Licznik = 0 THEN\n\
                sTime := CUR_TIME();\n\
                Licznik := 1;\n\
            END_IF;\n\
            IF CUR_TIME() - sTime >= t#2s THEN\n\
                sTime := CUR_TIME();\n\
                IF Licznik < 8 THEN\n\
                    Licznik := Licznik * 2;\n\
                ELSE\n\
                    Licznik := 1;\n\
                END_IF;\n\
            END_IF;\n\
        END_IF;\n\
        OUT0 := Licznik = 1;\n\
        OUT1 := Licznik = 2;\n\
        OUT2 := Licznik = 4;\n\
        OUT3 := Licznik = 8;\n\
        END_PROGRAM";
    // Over 25 fast scans far less than t#2s elapses, so Licznik stays 1 and only
    // OUT0 is driven high — matching the behaviour of the vendored .xcp golden.
    let watch = emit_run_watch(src, "wejestst", 25);
    assert!(
        watch.contains(&"OUT0 = TRUE".to_owned()),
        "watch: {watch:?}"
    );
    assert!(
        watch.contains(&"OUT1 = FALSE".to_owned()),
        "watch: {watch:?}"
    );
    assert!(
        watch.contains(&"OUT2 = FALSE".to_owned()),
        "watch: {watch:?}"
    );
    assert!(
        watch.contains(&"OUT3 = FALSE".to_owned()),
        "watch: {watch:?}"
    );
}

#[test]
fn function_block_instances_keep_independent_state() {
    // A user-defined up-counter FB, instantiated twice. Each call adds its STEP
    // input to the instance's ACC output. Distinct instances must keep separate
    // accumulators — the proof that per-instance data sub-frames work.
    let src = "FUNCTION_BLOCK Accum\n\
        VAR_INPUT step : INT; END_VAR\n\
        VAR_OUTPUT acc : INT; END_VAR\n\
        acc := acc + step;\n\
        END_FUNCTION_BLOCK\n\
        PROGRAM main\n\
        VAR\n\
            a : Accum;\n\
            b : Accum;\n\
            outA : INT;\n\
            outB : INT;\n\
        END_VAR\n\
        a(step := 2);\n\
        b(step := 5);\n\
        outA := a.acc;\n\
        outB := b.acc;\n\
        END_PROGRAM";
    // After 4 scans: a.acc = 2*4 = 8, b.acc = 5*4 = 20.
    let watch = emit_run_watch(src, "fbtest", 4);
    assert!(watch.contains(&"outA = 8".to_owned()), "watch: {watch:?}");
    assert!(watch.contains(&"outB = 20".to_owned()), "watch: {watch:?}");
}

#[test]
fn standard_counter_and_edge_detector() {
    // A self-toggling clock feeds an R_TRIG; its pulses drive a CTU. Fully
    // deterministic: rising edges occur on scans 1,3,5,7,..., so after 7 scans
    // the up-counter has seen 4 of them. Exercises the synthesized R_TRIG + CTU
    // standard FBs, FB output feeding another FB's input, and member reads.
    let src = "PROGRAM main\n\
        VAR\n\
            clk : BOOL;\n\
            r : R_TRIG;\n\
            c : CTU;\n\
            edges : INT;\n\
        END_VAR\n\
        clk := NOT clk;\n\
        r(CLK := clk);\n\
        c(CU := r.Q, R := FALSE, PV := 100);\n\
        edges := c.CV;\n\
        END_PROGRAM";
    assert!(emit_run_watch(src, "ctu", 7).contains(&"edges = 4".to_owned()));
}

#[test]
fn standard_up_down_counter_reaches_preset() {
    // CTUD counting up on a toggling clock; QU latches once CV >= PV (=2).
    let src = "PROGRAM main\n\
        VAR\n\
            clk : BOOL;\n\
            cud : CTUD;\n\
            v : INT;\n\
            up : BOOL;\n\
        END_VAR\n\
        clk := NOT clk;\n\
        cud(CU := clk, CD := FALSE, R := FALSE, LD := FALSE, PV := 2);\n\
        v := cud.CV;\n\
        up := cud.QU;\n\
        END_PROGRAM";
    // Edges on scans 1,3 -> CV=2 by scan 3, QU true.
    let watch = emit_run_watch(src, "cud", 3);
    assert!(watch.contains(&"v = 2".to_owned()), "watch: {watch:?}");
    assert!(watch.contains(&"up = TRUE".to_owned()), "watch: {watch:?}");
}

#[test]
fn standard_timers_run() {
    // TOF asserts Q immediately while IN is held true (deterministic), and TON
    // stays low because far less than its preset elapses across fast scans.
    let src = "PROGRAM main\n\
        VAR\n\
            en : BOOL;\n\
            ton : TON;\n\
            tof : TOF;\n\
            tonQ : BOOL;\n\
            tofQ : BOOL;\n\
        END_VAR\n\
        en := TRUE;\n\
        ton(IN := en, PT := T#10s);\n\
        tof(IN := en, PT := T#10s);\n\
        tonQ := ton.Q;\n\
        tofQ := tof.Q;\n\
        END_PROGRAM";
    let watch = emit_run_watch(src, "timers", 25);
    assert!(
        watch.contains(&"tofQ = TRUE".to_owned()),
        "watch: {watch:?}"
    );
    assert!(
        watch.contains(&"tonQ = FALSE".to_owned()),
        "watch: {watch:?}"
    );
}

#[test]
fn nested_function_blocks_compose() {
    // An outer FB `Doubler` contains an inner `Accum` instance, forwarding twice
    // the step. The program holds two `Doubler`s. This exercises FB-in-FB: nested
    // sub-frames, composed CALB data-offsets (outer_base + inner_rel_base), nested
    // INIT seeding, and member access through the nesting.
    let src = "FUNCTION_BLOCK Accum\n\
        VAR_INPUT step : INT; END_VAR\n\
        VAR_OUTPUT acc : INT; END_VAR\n\
        acc := acc + step;\n\
        END_FUNCTION_BLOCK\n\
        FUNCTION_BLOCK Doubler\n\
        VAR_INPUT inc : INT; END_VAR\n\
        VAR_OUTPUT total : INT; END_VAR\n\
        VAR inner : Accum; two : INT; END_VAR\n\
        two := inc + inc;\n\
        inner(step := two);\n\
        total := inner.acc;\n\
        END_FUNCTION_BLOCK\n\
        PROGRAM main\n\
        VAR\n\
            d1 : Doubler;\n\
            d2 : Doubler;\n\
            o1 : INT;\n\
            o2 : INT;\n\
        END_VAR\n\
        d1(inc := 1);\n\
        d2(inc := 10);\n\
        o1 := d1.total;\n\
        o2 := d2.total;\n\
        END_PROGRAM";
    // d1: each scan adds 2*1=2 -> after 4 scans o1 = 8.
    // d2: each scan adds 2*10=20 -> after 4 scans o2 = 80.
    let watch = emit_run_watch(src, "nested", 4);
    assert!(watch.contains(&"o1 = 8".to_owned()), "watch: {watch:?}");
    assert!(watch.contains(&"o2 = 80".to_owned()), "watch: {watch:?}");
}

#[test]
fn data_segment_exceeds_the_legacy_256_byte_cap() {
    // 200 DINT globals = 800 bytes of data, well past the vendored VM's 256-byte
    // static buffer. The backend allocates them and the shim sizes its heap data
    // buffer from the `.DCP`, so this loads and runs correctly. Each scan adds 1
    // to every variable; after 3 scans they all read 3.
    let mut src = String::from("PROGRAM main\nVAR\n");
    for i in 0..200 {
        src.push_str(&format!("    v{i} : DINT;\n"));
    }
    src.push_str("END_VAR\n");
    for i in 0..200 {
        src.push_str(&format!("v{i} := v{i} + 1;\n"));
    }
    src.push_str("END_PROGRAM");

    // Confirm the data segment really is over 256 bytes.
    let artifacts = plc_cpdev_backend::compile(&src).expect("compile big program");
    let data_size: usize = artifacts
        .dcp
        .split("Type=\"data\"")
        .nth(1)
        .and_then(|s| s.split("Size=\"").nth(1))
        .and_then(|s| s.split('"').next())
        .and_then(|s| s.parse().ok())
        .expect("data size in dcp");
    assert!(
        data_size > 256,
        "data segment was {data_size} bytes, expected > 256"
    );

    let watch = emit_run_watch(&src, "big", 3);
    assert_eq!(watch.len(), 200);
    assert!(
        watch.contains(&"v0 = 3".to_owned()),
        "watch[0]: {:?}",
        &watch[..3]
    );
    assert!(
        watch.contains(&"v199 = 3".to_owned()),
        "watch last: {:?}",
        &watch[197..]
    );
}

#[test]
fn strings_assign_and_watch() {
    // Literal assignment (MCD of the inline image), string var-to-var copy
    // (STRASGN), and an init value. Watch must decode the inline CPDev STRING
    // layout back to the text.
    let src = "PROGRAM main\n\
        VAR\n\
            greeting : STRING[16] := 'hi';\n\
            copy : STRING[16];\n\
            msg : STRING[16];\n\
        END_VAR\n\
        msg := 'hello';\n\
        copy := greeting;\n\
        END_PROGRAM";
    let watch = emit_run_watch(src, "strs", 3);
    assert!(
        watch.contains(&"msg = hello".to_owned()),
        "watch: {watch:?}"
    );
    assert!(watch.contains(&"copy = hi".to_owned()), "watch: {watch:?}");
    assert!(
        watch.contains(&"greeting = hi".to_owned()),
        "watch: {watch:?}"
    );
}

#[test]
fn string_input_to_function_block() {
    // A STRING flows into an FB input (copied into the instance frame) and back
    // out via a member read — strings through the FB data-offset model.
    let src = "FUNCTION_BLOCK Echo\n\
        VAR_INPUT inp : STRING[16]; END_VAR\n\
        VAR_OUTPUT outp : STRING[16]; END_VAR\n\
        outp := inp;\n\
        END_FUNCTION_BLOCK\n\
        PROGRAM main\n\
        VAR e : Echo; result : STRING[16]; END_VAR\n\
        e(inp := 'piped');\n\
        result := e.outp;\n\
        END_PROGRAM";
    let watch = emit_run_watch(src, "strfb", 2);
    assert!(
        watch.contains(&"result = piped".to_owned()),
        "watch: {watch:?}"
    );
}

#[test]
fn string_comparison_drives_output() {
    let src = "PROGRAM main\n\
        VAR mode : STRING[8] := 'AUTO'; isAuto : BOOL; isManual : BOOL; END_VAR\n\
        isAuto := mode = 'AUTO';\n\
        isManual := mode = 'MANUAL';\n\
        END_PROGRAM";
    let watch = emit_run_watch(src, "strcmp", 2);
    assert!(
        watch.contains(&"isAuto = TRUE".to_owned()),
        "watch: {watch:?}"
    );
    assert!(
        watch.contains(&"isManual = FALSE".to_owned()),
        "watch: {watch:?}"
    );
}

#[test]
fn string_functions_len_left_right_mid_concat() {
    let src = "PROGRAM main\n\
        VAR\n\
            s : STRING[16] := 'abcdef';\n\
            n : INT;\n\
            l : STRING[16];\n\
            r : STRING[16];\n\
            m : STRING[16];\n\
            c : STRING[16];\n\
        END_VAR\n\
        n := LEN(s);\n\
        l := LEFT(s, 3);\n\
        r := RIGHT(s, 2);\n\
        m := MID(s, 3, 2);\n\
        c := CONCAT(l, r);\n\
        END_PROGRAM";
    let watch = emit_run_watch(src, "strfns", 2);
    assert!(watch.contains(&"n = 6".to_owned()), "watch: {watch:?}");
    assert!(watch.contains(&"l = abc".to_owned()), "watch: {watch:?}");
    assert!(watch.contains(&"r = ef".to_owned()), "watch: {watch:?}");
    // IEC MID(IN, L:=3, P:=2): 3 chars starting at position 2 (1-based) = "bcd".
    assert!(watch.contains(&"m = bcd".to_owned()), "watch: {watch:?}");
    // CONCAT("abc", "ef") -> "abcef".
    assert!(watch.contains(&"c = abcef".to_owned()), "watch: {watch:?}");
}

#[test]
fn nested_member_access_two_levels() {
    // `outer.inner.acc` reads a scalar of an FB instance nested inside another FB
    // instance — resolved by composing frame-relative bases.
    let src = "FUNCTION_BLOCK Accum\n\
        VAR_INPUT step : INT; END_VAR\n\
        VAR_OUTPUT acc : INT; END_VAR\n\
        acc := acc + step;\n\
        END_FUNCTION_BLOCK\n\
        FUNCTION_BLOCK Wrapper\n\
        VAR_INPUT inc : INT; END_VAR\n\
        VAR inner : Accum; END_VAR\n\
        inner(step := inc);\n\
        END_FUNCTION_BLOCK\n\
        PROGRAM main\n\
        VAR w : Wrapper; total : INT; END_VAR\n\
        w(inc := 3);\n\
        total := w.inner.acc;\n\
        END_PROGRAM";
    // inner accumulates +3 per scan; after 4 scans w.inner.acc = 12.
    assert!(emit_run_watch(src, "nest2", 4).contains(&"total = 12".to_owned()));
}

#[test]
fn load_from_file_matches() {
    let src = "PROGRAM main\nVAR n : INT; END_VAR\nn := n + 1;\nEND_PROGRAM";
    let artifacts = plc_cpdev_backend::compile(src).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let xcp_path = dir.path().join("main.xcp");
    std::fs::write(&xcp_path, &artifacts.xcp).unwrap();
    std::fs::write(dir.path().join("main.dcp"), &artifacts.dcp).unwrap();
    let doc = SourceDocument::new(format!("file://{}", xcp_path.display()), 0, String::new());

    let mut engine = XcpEngine::default();
    engine.load(&doc).expect("load file");
    engine.run_scans(3);
    assert_eq!(engine.watch(), vec!["n = 3".to_owned()]);
}
