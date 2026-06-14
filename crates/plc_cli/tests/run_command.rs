use std::process::Command;

#[test]
fn cli_run_prints_plc_print_output() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("hello.st");
    std::fs::write(
        &file,
        "PROGRAM Hello\nVAR\nEND_VAR\nPLC_PRINT('Hello from CLI');\nEND_PROGRAM\n",
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
        "Hello from CLI"
    );
}
