use plc_compiler_core::{CompilerCore, SourceDocument};
use std::env;
use std::fs;
use std::path::PathBuf;

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
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let document = SourceDocument::new(format!("file://{}", path.display()), 0, text);
    let result = CompilerCore::default().execute(&document);

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
