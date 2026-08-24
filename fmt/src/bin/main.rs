use sleigh::SourceDb;
use sleigh_fmt::{FmtError, Formatter};
use std::{env, fs, path::Path, process};

fn usage(prog: &str) -> ! {
    eprintln!("Usage: {prog} <file.slaspec> [--check]");
    eprintln!();
    eprintln!("  <file.slaspec>  path to a SLEIGH root spec file");
    eprintln!("  --check        exit 1 if any file would be reformatted (no writes)");
    process::exit(1);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let prog = args.first().map(String::as_str).unwrap_or("sleigh-fmt");

    let (spec_path, check) = match args.as_slice() {
        [_, path] => (path.as_str(), false),
        [_, path, flag] if flag == "--check" => (path.as_str(), true),
        [_, flag, path] if flag == "--check" => (path.as_str(), true),
        _ => usage(prog),
    };

    let mut sources = SourceDb::new();
    let root = sources
        .add_file_from_path(Path::new(spec_path))
        .unwrap_or_else(|e| {
            eprintln!("error: cannot read '{spec_path}': {e}");
            process::exit(1);
        });

    let result = match Formatter::new().format(&mut sources, root) {
        Ok(result) => result,
        Err(FmtError::ParseError(diagnostics)) => {
            for diagnostic in &diagnostics {
                eprintln!("{}", diagnostic.render(&sources));
            }
            process::exit(1);
        }
    };

    let mut changed = false;
    for formatted in &result.files {
        let original = sources.text(formatted.file).unwrap_or("");
        if formatted.content == original {
            continue;
        }

        changed = true;
        let path = sources.path(formatted.file).unwrap();

        if check {
            eprintln!("would reformat: {}", path.display());
        } else {
            fs::write(path, &formatted.content).unwrap_or_else(|e| {
                eprintln!("error: writing '{}': {e}", path.display());
                process::exit(1);
            });
            println!("reformatted: {}", path.display());
        }
    }

    if check && changed {
        process::exit(1);
    }
}
