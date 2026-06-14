use plc_runtime::{Ctd, Ctu, Ctud, FTrig, RTrig};

#[test]
fn ctu_counts_rising_edges_and_resets() {
    let mut ctu = Ctu::new();
    // Two rising edges.
    ctu.update(false, false, 3);
    ctu.update(true, false, 3);
    ctu.update(false, false, 3);
    ctu.update(true, false, 3);
    assert_eq!(ctu.cv(), 2);
    assert!(!ctu.q());

    // Third edge reaches PV.
    ctu.update(false, false, 3);
    ctu.update(true, false, 3);
    assert_eq!(ctu.cv(), 3);
    assert!(ctu.q());

    // Reset.
    ctu.update(false, true, 3);
    assert_eq!(ctu.cv(), 0);
    assert!(!ctu.q());
}

#[test]
fn ctd_loads_and_counts_down_to_zero() {
    let mut ctd = Ctd::new();
    ctd.update(false, true, 2); // load PV = 2
    assert_eq!(ctd.cv(), 2);
    assert!(!ctd.q());

    ctd.update(false, false, 2);
    ctd.update(true, false, 2);
    ctd.update(false, false, 2);
    ctd.update(true, false, 2);
    assert_eq!(ctd.cv(), 0);
    assert!(ctd.q());
}

#[test]
fn ctud_counts_both_directions() {
    let mut ctud = Ctud::new();
    ctud.update(false, false, false, false, 2);
    ctud.update(true, false, false, false, 2); // up edge -> 1
    ctud.update(false, false, false, false, 2);
    ctud.update(true, false, false, false, 2); // up edge -> 2
    assert_eq!(ctud.cv(), 2);
    assert!(ctud.qu());

    ctud.update(false, true, false, false, 2); // down edge -> 1
    assert_eq!(ctud.cv(), 1);
    assert!(!ctud.qu());
    assert!(!ctud.qd());
}

#[test]
fn r_trig_detects_single_rising_edge() {
    let mut rtrig = RTrig::new();
    assert!(!rtrig.update(false));
    assert!(rtrig.update(true));
    // Held high: no further pulse.
    assert!(!rtrig.update(true));
    assert!(!rtrig.update(false));
}

#[test]
fn f_trig_detects_single_falling_edge() {
    let mut ftrig = FTrig::new();
    assert!(!ftrig.update(true));
    assert!(ftrig.update(false));
    assert!(!ftrig.update(false));
}
