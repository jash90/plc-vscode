use plc_runtime::{Tof, Ton, Tp};

#[test]
fn ton_asserts_q_after_preset_elapsed() {
    let mut ton = Ton::new(100);

    assert!(!ton.update(true, 0));
    assert!(!ton.update(true, 50));
    assert_eq!(ton.et_ms(), 50);
    assert!(ton.update(true, 100));
    assert!(ton.q());
    assert_eq!(ton.et_ms(), 100);

    // Dropping the input resets Q and ET.
    assert!(!ton.update(false, 110));
    assert_eq!(ton.et_ms(), 0);
}

#[test]
fn tof_holds_q_for_preset_after_falling_edge() {
    let mut tof = Tof::new(100);

    assert!(tof.update(true, 0));
    // Falling edge begins the off-delay.
    assert!(tof.update(false, 0));
    assert!(tof.update(false, 50));
    assert!(tof.q());
    assert!(!tof.update(false, 150));
    assert!(!tof.q());
}

#[test]
fn tp_emits_fixed_length_pulse_on_rising_edge() {
    let mut tp = Tp::new(100);

    assert!(!tp.update(false, 0));
    assert!(tp.update(true, 10));
    assert!(tp.q());
    // Pulse ignores further input changes until PT elapses.
    assert!(tp.update(false, 80));
    assert!(tp.q());
    assert!(!tp.update(false, 120));
    assert!(!tp.q());
}
