//! IEC 61131-3 standard function blocks, synthesized as ordinary Structured
//! Text definitions so they lower through the same path as user function blocks.
//!
//! Timers use the VM's `CUR_TIME()` intrinsic; counters and edge detectors use
//! prev-value state + comparisons. The semantics mirror the interpreter's
//! reference implementations (`plc_runtime::{timers, counters, edge}`) so the
//! CPDev backend and the interpreter agree on behaviour. Port names follow IEC
//! (TON: `IN`/`PT`->`Q`/`ET`; CTU: `CU`/`R`/`PV`->`Q`/`CV`; etc.).
//!
//! Note: the CPDev VM's `CUR_TIME` is wall-clock, so a timer's elapsed time
//! advances by real time between scans (not a fixed per-scan tick).

use plc_runtime::ast::{Unit, build_units};

/// Parse the standard-FB definitions into [`Unit`]s (FB POUs only).
pub fn units() -> Vec<Unit> {
    build_units(SOURCE)
}

/// The standard function blocks as Structured Text. Kept to the subset the
/// backend supports: BOOL/INT/TIME, IF/ELSIF/ELSE, comparisons, AND/NOT, +/-,
/// `CUR_TIME()`, and literal assignment.
const SOURCE: &str = "\
FUNCTION_BLOCK TON
VAR_INPUT IN : BOOL; PT : TIME; END_VAR
VAR_OUTPUT Q : BOOL; ET : TIME; END_VAR
VAR running : BOOL; startTime : TIME; END_VAR
IF IN THEN
    IF NOT running THEN
        running := TRUE;
        startTime := CUR_TIME();
    END_IF;
    ET := CUR_TIME() - startTime;
    IF ET >= PT THEN
        ET := PT;
        Q := TRUE;
    ELSE
        Q := FALSE;
    END_IF;
ELSE
    running := FALSE;
    Q := FALSE;
    ET := T#0ms;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK TOF
VAR_INPUT IN : BOOL; PT : TIME; END_VAR
VAR_OUTPUT Q : BOOL; ET : TIME; END_VAR
VAR offRunning : BOOL; offStart : TIME; END_VAR
IF IN THEN
    offRunning := FALSE;
    ET := T#0ms;
    Q := TRUE;
ELSIF offRunning THEN
    ET := CUR_TIME() - offStart;
    IF ET >= PT THEN
        ET := PT;
        Q := FALSE;
    ELSE
        Q := TRUE;
    END_IF;
ELSIF Q THEN
    offRunning := TRUE;
    offStart := CUR_TIME();
    ET := T#0ms;
    Q := TRUE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK TP
VAR_INPUT IN : BOOL; PT : TIME; END_VAR
VAR_OUTPUT Q : BOOL; ET : TIME; END_VAR
VAR pulsing : BOOL; pulseStart : TIME; prevIn : BOOL; END_VAR
IF pulsing THEN
    ET := CUR_TIME() - pulseStart;
    IF ET >= PT THEN
        ET := PT;
        Q := FALSE;
        pulsing := FALSE;
    ELSE
        Q := TRUE;
    END_IF;
END_IF;
IF IN AND NOT prevIn AND NOT pulsing AND NOT Q THEN
    pulsing := TRUE;
    pulseStart := CUR_TIME();
    ET := T#0ms;
    Q := TRUE;
END_IF;
prevIn := IN;
END_FUNCTION_BLOCK

FUNCTION_BLOCK CTU
VAR_INPUT CU : BOOL; R : BOOL; PV : INT; END_VAR
VAR_OUTPUT Q : BOOL; CV : INT; END_VAR
VAR prevCU : BOOL; END_VAR
IF R THEN
    CV := 0;
ELSIF CU AND NOT prevCU THEN
    CV := CV + 1;
END_IF;
prevCU := CU;
Q := CV >= PV;
END_FUNCTION_BLOCK

FUNCTION_BLOCK CTD
VAR_INPUT CD : BOOL; LD : BOOL; PV : INT; END_VAR
VAR_OUTPUT Q : BOOL; CV : INT; END_VAR
VAR prevCD : BOOL; END_VAR
IF LD THEN
    CV := PV;
ELSIF CD AND NOT prevCD THEN
    CV := CV - 1;
END_IF;
prevCD := CD;
Q := CV <= 0;
END_FUNCTION_BLOCK

FUNCTION_BLOCK CTUD
VAR_INPUT CU : BOOL; CD : BOOL; R : BOOL; LD : BOOL; PV : INT; END_VAR
VAR_OUTPUT QU : BOOL; QD : BOOL; CV : INT; END_VAR
VAR prevCU : BOOL; prevCD : BOOL; END_VAR
IF R THEN
    CV := 0;
ELSIF LD THEN
    CV := PV;
ELSE
    IF CU AND NOT prevCU THEN
        CV := CV + 1;
    END_IF;
    IF CD AND NOT prevCD THEN
        CV := CV - 1;
    END_IF;
END_IF;
prevCU := CU;
prevCD := CD;
QU := CV >= PV;
QD := CV <= 0;
END_FUNCTION_BLOCK

FUNCTION_BLOCK R_TRIG
VAR_INPUT CLK : BOOL; END_VAR
VAR_OUTPUT Q : BOOL; END_VAR
VAR prev : BOOL; END_VAR
Q := CLK AND NOT prev;
prev := CLK;
END_FUNCTION_BLOCK

FUNCTION_BLOCK F_TRIG
VAR_INPUT CLK : BOOL; END_VAR
VAR_OUTPUT Q : BOOL; END_VAR
VAR prev : BOOL; END_VAR
Q := NOT CLK AND prev;
prev := CLK;
END_FUNCTION_BLOCK
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_eight_standard_blocks_parse() {
        let names: Vec<String> = units()
            .iter()
            .map(|u| u.name.to_ascii_uppercase())
            .collect();
        for expected in ["TON", "TOF", "TP", "CTU", "CTD", "CTUD", "R_TRIG", "F_TRIG"] {
            assert!(
                names.contains(&expected.to_owned()),
                "missing {expected} in {names:?}"
            );
        }
    }
}
