use std::process::Command;

#[test]
fn cli_run_prints_initialized_string_state() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("hello.st");
    std::fs::write(
        &file,
        "PROGRAM Hello\nVAR\n    Message : STRING := 'Hello from standard ST via plugin';\nEND_VAR\nEND_PROGRAM\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_plc"))
        .arg("run")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "Message = Hello from standard ST via plugin"
    );
}
