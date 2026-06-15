use plc_compiler_core::{CompilerCore, SourceDocument};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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
                .ok_or_else(|| "usage: plc run <file.st>".to_owned())?;
            run_file(path)
        }
        _ => Err("usage: plc run <file.st>".to_owned()),
    }
}

fn run_file(path: PathBuf) -> Result<(), String> {
    let text = read_source(&path)?;
    let document = SourceDocument::new(format!("file://{}", path.display()), 0, text);
    let result = CompilerCore.execute(&document);

    if !result.diagnostics().is_empty() {
        for diagnostic in result.diagnostics() {
            eprintln!("{}: {}", diagnostic.code, diagnostic.message);
        }
        return Err("execution failed due to diagnostics".to_owned());
    }

    if result.output().is_empty() {
        println!("(no output)");
    } else {
        for line in result.output() {
            println!("{line}");
        }
    }
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
    use super::decode_source;

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
