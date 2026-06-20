use plc_cli::{DEFAULT_SCANS, run_with};
use plc_compiler_core::{CompilerCore, SourceDocument};
use plc_lang::LanguageRegistry;
use plc_runtime::ScanRuntimeEngine;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

mod dap;

const USAGE: &str = "usage:\n  plc run <file.st> [scans]\n  plc build <file.st> [--target cpdev] [-o <out.xcp>]\n  plc convert <from-id> <to-id> <file>   (ids: plc languages)\n  plc debug                              (Debug Adapter Protocol over stdio)";

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("run") => {
            let path = args
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| USAGE.to_owned())?;
            // Optional scan-count override (e.g. `plc run file.st 1`).
            let scans = args
                .next()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(DEFAULT_SCANS);
            run_file(path, scans)
        }
        Some("build") => {
            let parsed = parse_build_args(args)?;
            build_file(parsed.path, parsed.output, &parsed.target)
        }
        Some("convert") => {
            let from = args.next().ok_or_else(|| USAGE.to_owned())?;
            let to = args.next().ok_or_else(|| USAGE.to_owned())?;
            let path = args
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| USAGE.to_owned())?;
            convert_file(&from, &to, path)
        }
        Some("languages") => {
            for id in LanguageRegistry::with_builtins().ids() {
                println!("{id}");
            }
            Ok(())
        }
        // Debug Adapter Protocol server over stdio; the program path arrives in
        // the DAP `launch` request, so no file argument here.
        Some("debug") => dap::run(),
        _ => Err(USAGE.to_owned()),
    }
}

/// Parsed `plc build` arguments. The path may appear before or after the flags.
struct BuildArgs {
    path: PathBuf,
    output: Option<PathBuf>,
    target: String,
}

/// Parse the arguments after `plc build`, accepting the file path in any
/// position relative to the flags (`-o`/`--output`, `--target`). The default
/// target is `cpdev`.
fn parse_build_args(args: impl Iterator<Item = String>) -> Result<BuildArgs, String> {
    let mut args = args;
    let mut path: Option<PathBuf> = None;
    let mut output = None;
    let mut target = "cpdev".to_owned();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-o" | "--output" => {
                output = Some(
                    args.next()
                        .map(PathBuf::from)
                        .ok_or_else(|| format!("`{arg}` needs a file path\n{USAGE}"))?,
                );
            }
            "--target" => {
                target = args
                    .next()
                    .ok_or_else(|| format!("`--target` needs a value\n{USAGE}"))?;
            }
            flag if flag.starts_with('-') => {
                return Err(format!("unknown build option `{flag}`\n{USAGE}"));
            }
            _ => {
                if path.is_some() {
                    return Err(format!("unexpected extra argument `{arg}`\n{USAGE}"));
                }
                path = Some(PathBuf::from(arg));
            }
        }
    }
    let path = path.ok_or_else(|| USAGE.to_owned())?;
    Ok(BuildArgs {
        path,
        output,
        target,
    })
}

fn run_file(path: PathBuf, scans: u64) -> Result<(), String> {
    // Compiled CPDev bytecode is a binary artifact: skip the text decode and the
    // ST analysis gate, and drive the vendored VM. Built only with `--features cpdev`.
    #[cfg(feature = "cpdev")]
    if path.extension().and_then(|ext| ext.to_str()) == Some("xcp") {
        let document = SourceDocument::new(format!("file://{}", path.display()), 0, String::new());
        let mut engine = plc_cpdev_vm::XcpEngine::default();
        return plc_cli::run_artifact(&mut engine, &document, scans);
    }

    // Without the `cpdev` feature the VM is not built in, so `.xcp` bytecode
    // cannot be executed. Detect it and explain, instead of letting the binary
    // fall through to the Structured Text parser (which emits noise like
    // `SYN0000: Invalid token`).
    #[cfg(not(feature = "cpdev"))]
    if path.extension().and_then(|ext| ext.to_str()) == Some("xcp") {
        return Err(format!(
            "`{}` is CPDev .xcp bytecode, which this `plc` build cannot execute \
             (compiled without the `cpdev` feature). Compile to .xcp with \
             `plc build` and run it on a CPDev target, or rebuild the CLI with \
             `--features cpdev` to execute .xcp locally.",
            path.display()
        ));
    }

    let text = read_source(&path)?;
    let document = SourceDocument::new(format!("file://{}", path.display()), 0, text);

    // Default wiring: the CompilerCore analyzer + the scan-cycle runtime engine.
    // Both are pluggable — `run_with` accepts any LanguageService/ExecutionEngine.
    let service = CompilerCore;
    let mut engine = ScanRuntimeEngine::default();
    run_with(&service, &mut engine, &document, scans)
}

/// `plc build <file.st> [--target cpdev] [-o <out.xcp>]`: compile Structured Text
/// to a CPDev `.xcp` bytecode file plus its `.dcp` variable-map sidecar. Runs the
/// same diagnostics gate as `run`, then emits with `plc_cpdev_backend` (pure Rust,
/// no C++ toolchain). Run the result with `plc run <out.xcp>` (needs `--features cpdev`).
fn build_file(path: PathBuf, output: Option<PathBuf>, target: &str) -> Result<(), String> {
    if target != "cpdev" {
        return Err(format!(
            "unknown build target `{target}` (supported: cpdev)"
        ));
    }
    let text = read_source(&path)?;
    let document = SourceDocument::new(format!("file://{}", path.display()), 0, text.clone());

    // Diagnostics gate, identical to `run`: don't emit bytecode for broken source.
    let analysis = CompilerCore.analyze(&document);
    if !analysis.diagnostics().is_empty() {
        for diagnostic in analysis.diagnostics() {
            eprintln!("{}: {}", diagnostic.code, diagnostic.message);
        }
        return Err("build failed due to diagnostics".to_owned());
    }

    let artifacts = plc_cpdev_backend::compile(&text)
        .map_err(|error| format!("cpdev codegen failed: {error}"))?;

    let xcp_path = output.unwrap_or_else(|| path.with_extension("xcp"));
    let dcp_path = xcp_path.with_extension("dcp");
    fs::write(&xcp_path, &artifacts.xcp)
        .map_err(|error| format!("failed to write {}: {error}", xcp_path.display()))?;
    fs::write(&dcp_path, &artifacts.dcp)
        .map_err(|error| format!("failed to write {}: {error}", dcp_path.display()))?;

    eprintln!(
        "wrote {} ({} bytes) and {}",
        xcp_path.display(),
        artifacts.xcp.len(),
        dcp_path.display()
    );
    Ok(())
}

/// `plc convert <from-id> <to-id> <file>`: transpile one PLC language into
/// another through the canonical-IR hub, printing the converted source to stdout
/// and any fidelity notes / diagnostics to stderr.
fn convert_file(from: &str, to: &str, path: PathBuf) -> Result<(), String> {
    let text = read_source(&path)?;
    let document = SourceDocument::new(format!("file://{}", path.display()), 0, text);

    let registry = LanguageRegistry::with_builtins();
    let result = registry.convert(from, to, &document);

    for note in &result.fidelity {
        eprintln!("note: {note}");
    }
    if let Some(error) = result.error {
        for diagnostic in &result.diagnostics {
            eprintln!("{}: {}", diagnostic.code, diagnostic.message);
        }
        return Err(format!("conversion {from} -> {to} failed: {error:?}"));
    }
    print!("{}", result.text);
    Ok(())
}

/// Read a source file, decoding common non-UTF-8 encodings so legacy ST files
/// are not dropped from coverage: UTF-16 LE/BE (detected by BOM), a UTF-8 BOM,
/// and a Latin-1 fallback for other non-UTF-8 bytes (e.g. Windows-1252).
fn read_source(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    Ok(decode_source(&bytes))
}

fn decode_source(bytes: &[u8]) -> String {
    if let [0xFF, 0xFE, rest @ ..] = bytes {
        let units: Vec<u16> = rest
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    if let [0xFE, 0xFF, rest @ ..] = bytes {
        let units: Vec<u16> = rest
            .chunks_exact(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }

    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        // Legacy 8-bit encoding: map each byte to its Latin-1 code point so the
        // file still parses instead of being skipped.
        Err(_) => bytes.iter().map(|&byte| byte as char).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_source, parse_build_args};
    use std::path::PathBuf;

    fn build(args: &[&str]) -> Result<super::BuildArgs, String> {
        parse_build_args(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn build_args_path_first() {
        let a = build(&["prog.st", "--target", "cpdev"]).unwrap();
        assert_eq!(a.path, PathBuf::from("prog.st"));
        assert_eq!(a.target, "cpdev");
        assert_eq!(a.output, None);
    }

    #[test]
    fn build_args_flags_before_path() {
        // The bug this fixes: `plc build --target cpdev prog.st` used to fail.
        let a = build(&["--target", "cpdev", "prog.st"]).unwrap();
        assert_eq!(a.path, PathBuf::from("prog.st"));
        assert_eq!(a.target, "cpdev");
    }

    #[test]
    fn build_args_output_and_default_target() {
        let a = build(&["-o", "out.xcp", "prog.st"]).unwrap();
        assert_eq!(a.path, PathBuf::from("prog.st"));
        assert_eq!(a.output, Some(PathBuf::from("out.xcp")));
        assert_eq!(a.target, "cpdev"); // default
    }

    #[test]
    fn build_args_missing_path_errors() {
        assert!(build(&["--target", "cpdev"]).is_err());
    }

    #[test]
    fn build_args_unknown_flag_and_extra_positional_error() {
        assert!(build(&["prog.st", "--bogus"]).is_err());
        assert!(build(&["a.st", "b.st"]).is_err());
    }

    #[test]
    fn decodes_utf16_le_with_bom() {
        // "AB" encoded as UTF-16 LE with a byte-order mark.
        let bytes = [0xFF, 0xFE, b'A', 0x00, b'B', 0x00];
        assert_eq!(decode_source(&bytes), "AB");
    }

    #[test]
    fn decodes_utf16_be_with_bom() {
        let bytes = [0xFE, 0xFF, 0x00, b'A', 0x00, b'B'];
        assert_eq!(decode_source(&bytes), "AB");
    }

    #[test]
    fn falls_back_to_latin1_for_non_utf8() {
        // Windows-1252 / Latin-1 'ö' (0xF6) after 'A'; invalid as UTF-8.
        assert_eq!(decode_source(&[b'A', 0xF6]), "A\u{F6}");
    }

    #[test]
    fn passes_through_plain_utf8() {
        assert_eq!(decode_source("PROGRAM Main".as_bytes()), "PROGRAM Main");
    }
}
