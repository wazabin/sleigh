//! `sleigh-decode` – decode raw bytes using a SLEIGH processor specification.
//!
//! # Usage
//!
//! ```text
//! sleigh-decode <spec-file> <hex-bytes>
//!
//! Arguments:
//!   <spec-file>   Path to a .slaspec file
//!   <hex-bytes>   Hex-encoded instruction bytes, e.g. 90 or 4883c045
//! ```
//!
//! # Example
//!
//! ```text
//! $ sleigh-decode x86-64.slaspec 90
//! [0x0000] NOP
//! ```

use sleigh::{Compiler, DecodeError, Decoder, SourceDb};
use std::{env, path::Path, process};

fn usage(prog: &str) -> ! {
    eprintln!("Usage: {prog} <spec-file> <hex-bytes>");
    eprintln!();
    eprintln!("  <spec-file>   path to a .slaspec file");
    eprintln!("  <hex-bytes>   instruction bytes as a hex string (e.g. 90 or 4883c045)");
    process::exit(1);
}

fn parse_hex(s: &str) -> Vec<u8> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if s.len() % 2 != 0 {
        eprintln!("error: hex string must have an even number of digits");
        process::exit(1);
    }
    (0..s.len() / 2)
        .map(|i| {
            u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap_or_else(|_| {
                eprintln!("error: invalid hex byte '{}'", &s[2 * i..2 * i + 2]);
                process::exit(1);
            })
        })
        .collect()
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let prog = args.first().map(String::as_str).unwrap_or("sleigh-decode");

    if args.len() != 3 {
        usage(prog);
    }

    let spec_path = Path::new(&args[1]);
    let bytes = parse_hex(&args[2]);

    let mut sources = SourceDb::new();
    let root = sources.add_file_from_path(spec_path).unwrap_or_else(|e| {
        eprintln!("error: reading '{}' failed: {e}", spec_path.display());
        process::exit(1);
    });
    let compiled = Compiler::new(&mut sources).compile(root);
    let spec = match compiled {
        Ok(spec) => spec,
        Err(error) => {
            for diagnostic in error.diagnostics() {
                eprintln!("{}", diagnostic.render(&sources));
            }
            process::exit(1);
        }
    };
    let context = spec.new_context();
    let decoder = Decoder::new(&spec);

    match decoder.decode_one(0, &bytes, &context) {
        Ok(inst) => {
            println!("[0x{:04x}] {inst}", 0u64);
        }
        Err(DecodeError::NoMatch) => {
            eprintln!("error: no matching instruction for bytes: {}", args[2]);
            process::exit(1);
        }
        Err(error) => {
            eprintln!("error: failed to decode bytes {}: {error:?}", args[2]);
            process::exit(1);
        }
    }
}
