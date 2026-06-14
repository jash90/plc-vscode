# PLC ST-only test corpus

Source root: `/tmp/st-corpus`

Prepared folders:
- `all/` contains every copied `.st` file, preserving repository-relative paths.
- `test-ready/` contains UTF-8 `.st` files up to 100 KB for normal automated runs.
- `large-over-100kb/` contains `.st` files over 100 KB for separate timeout/performance tests.
- `non-utf8/` contains `.st` files that failed UTF-8 decoding.

Counts:
- all `.st`: 1469
- test-ready: 1464
- large over 100 KB: 3
- non-UTF-8: 2

Run VS Code/plugin corpus test against the prepared subset with:

```bash
PLC_CORPUS_ROOT=/Users/bartlomiejzimny/Projects/plc-vscode/tests/st/test-ready PLC_REPORT_PATH=/tmp/plc_vscode_plugin_st_only_report_20260614.json PLC_TEST_MODE=small PLC_MAX_BYTES=100000 PLC_PROVIDER_TIMEOUT_MS=5000 PLC_DIAGNOSTIC_SETTLE_MS=25 '/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code'   --user-data-dir /tmp/plc-vscode-st-only-user   --extensions-dir /tmp/plc-vscode-st-only-exts   --disable-workspace-trust   --extensionDevelopmentPath=/Users/bartlomiejzimny/Projects/plc-vscode/editors/vscode   --extensionTestsPath=/tmp/plc-vscode-corpus-extension-test/index.js   /Users/bartlomiejzimny/Projects/plc-vscode/tests/st/test-ready
```
