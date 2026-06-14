use plc_runtime::{Runtime, VirtualClock};

const PROGRAM: &str =
    "PROGRAM Main\nVAR\n    Count : INT;\nEND_VAR\nCount := Count + 1;\nEND_PROGRAM\n";

#[test]
fn scans_advance_virtual_time_deterministically() {
    let mut runtime = Runtime::from_source(PROGRAM);
    runtime.set_scan_interval_ms(10);

    runtime.run_scans(5);

    assert_eq!(runtime.now_ms(), 50);
}

#[test]
fn virtual_time_can_be_advanced_explicitly() {
    let mut runtime = Runtime::from_source(PROGRAM);
    runtime.advance_time(250);
    assert_eq!(runtime.now_ms(), 250);

    runtime.set_scan_interval_ms(5);
    runtime.run_scan();
    assert_eq!(runtime.now_ms(), 255);
}

#[test]
fn clock_is_repeatable_for_identical_runs() {
    let clock = VirtualClock::with_scan_interval_ms(20);
    assert_eq!(clock.now_ms(), 0);
    assert_eq!(clock.scan_interval_ms(), 20);

    let mut first = Runtime::from_source(PROGRAM);
    first.set_scan_interval_ms(20);
    first.run_scans(3);

    let mut second = Runtime::from_source(PROGRAM);
    second.set_scan_interval_ms(20);
    second.run_scans(3);

    assert_eq!(first.now_ms(), second.now_ms());
    assert_eq!(first.now_ms(), 60);
}
