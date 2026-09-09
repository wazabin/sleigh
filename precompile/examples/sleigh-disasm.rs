//! `sleigh-disasm` — disassemble bytes with an embedded SLEIGH specification.
//!
//! Arguments come either from flags or from one JSON object whose keys are
//! the flag names; `--json` switches the output to JSON.
//!
//! ```text
//! sleigh-disasm --arch x64 --address 0x401000 4889d84801c8c3
//! sleigh-disasm --args '{"arch":"x64","address":"0x401000","bytes":"4889d84801c8c3"}' --json
//! sleigh-disasm --arch aarch64 --file firmware.bin --offset 0x100 --count 20 --pcode
//! ```

use clap::Parser;
use serde::{Deserialize, Serialize};
use sleigh::{CompiledSpec, Decoder};
use std::{fs, process};

/// Disassemble bytes with an embedded SLEIGH specification.
#[derive(Parser)]
#[command(name = "sleigh-disasm", version)]
struct Cli {
    /// All options as one JSON object; keys are the long flag names
    #[arg(long, value_name = "JSON", conflicts_with_all = ["bytes", "arch", "address", "file", "offset", "count", "pcode"])]
    args: Option<String>,
    /// Emit JSON instead of text
    #[arg(long)]
    json: bool,
    #[command(flatten)]
    opts: Opts,
}

/// The options proper: one struct for both the flags and the `--args` JSON.
#[derive(clap::Args, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct Opts {
    /// Instruction bytes as hex
    bytes: Option<String>,
    /// Architecture: x64, x86, aarch64, riscv
    #[arg(long, default_value = "x64")]
    #[serde(default = "default_arch")]
    arch: String,
    /// Address of the first instruction (decimal or 0x-prefixed)
    #[arg(long, default_value = "0")]
    address: String,
    /// Read the bytes from this file instead of the command line
    #[arg(long, conflicts_with = "bytes")]
    file: Option<String>,
    /// Byte offset into --file (decimal or 0x-prefixed)
    #[arg(long, default_value = "0", requires = "file")]
    offset: String,
    /// Stop after this many instructions
    #[arg(long)]
    count: Option<usize>,
    /// Also print each instruction's p-code
    #[arg(long)]
    pcode: bool,
}

fn default_arch() -> String {
    "x64".to_string()
}

#[derive(Serialize)]
struct Output {
    arch: String,
    instructions: Vec<Insn>,
}

#[derive(Serialize)]
struct Insn {
    address: String,
    bytes: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pcode: Option<Vec<String>>,
}

fn spec_for(arch: &str) -> Result<&'static CompiledSpec, String> {
    Ok(match arch {
        "x64" => sleigh_precompile::x64::spec(),
        "x86" => sleigh_precompile::x86::spec(),
        "aarch64" => sleigh_precompile::aarch64::spec(),
        "riscv" => sleigh_precompile::riscv::spec(),
        other => return Err(format!("unknown arch '{other}' (x64, x86, aarch64, riscv)")),
    })
}

fn parse_int(what: &str, value: &str) -> Result<u64, String> {
    let value = value.trim();
    match value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        Some(hex) => u64::from_str_radix(hex, 16),
        None => value.parse(),
    }
    .map_err(|_| format!("invalid {what} '{value}'"))
}

fn parse_hex(value: &str) -> Result<Vec<u8>, String> {
    let value: String = value.chars().filter(|c| !c.is_whitespace()).collect();
    if value.is_empty() || value.len() % 2 != 0 {
        return Err("bytes must be a non-empty, even-length hex string".to_string());
    }
    (0..value.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&value[i..i + 2], 16)
                .map_err(|_| format!("invalid hex byte '{}'", &value[i..i + 2]))
        })
        .collect()
}

fn run(opts: &Opts) -> Result<Output, String> {
    let spec = spec_for(&opts.arch)?;
    let address = parse_int("address", &opts.address)?;
    let data = match (&opts.bytes, &opts.file) {
        (Some(hex), None) => parse_hex(hex)?,
        (None, Some(path)) => {
            let offset = parse_int("offset", &opts.offset)? as usize;
            let file = fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))?;
            file.get(offset..)
                .ok_or_else(|| format!("offset {offset:#x} is past the end of {path}"))?
                .to_vec()
        }
        (Some(_), Some(_)) => return Err("give either bytes or file, not both".to_string()),
        (None, None) => return Err("give bytes or --file".to_string()),
    };

    let decoder = Decoder::new(spec);
    let limit = opts.count.unwrap_or(usize::MAX);
    let mut instructions = Vec::new();
    let mut cursor = 0usize;
    while instructions.len() < limit && cursor < data.len() {
        let at = address + cursor as u64;
        let decoded = decoder.decode_one(at, &data[cursor..], &spec.new_context());
        let instruction = match decoded {
            Ok(instruction) => instruction,
            // The first instruction is the caller's request; later failures
            // just end the walk.
            Err(e) if instructions.is_empty() => {
                return Err(format!("no instruction decodes at {at:#x}: {e:?}"));
            }
            Err(_) => break,
        };
        let len = instruction.len();
        let pcode = if opts.pcode {
            let ast = instruction
                .pcode_ast()
                .map_err(|e| format!("p-code emission failed at {at:#x}: {e}"))?;
            Some(ast.pretty_print(spec).lines().map(str::to_string).collect())
        } else {
            None
        };
        instructions.push(Insn {
            address: format!("{at:#x}"),
            bytes: data[cursor..cursor + len]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
            text: instruction.to_string(),
            pcode,
        });
        cursor += len;
    }
    Ok(Output {
        arch: opts.arch.clone(),
        instructions,
    })
}

fn main() {
    let cli = Cli::parse();
    let opts = match &cli.args {
        Some(json) => match serde_json::from_str::<Opts>(json) {
            Ok(opts) => opts,
            Err(e) => fail(cli.json, &format!("invalid --args: {e}")),
        },
        None => cli.opts,
    };
    let output = match run(&opts) {
        Ok(output) => output,
        Err(e) => fail(cli.json, &e),
    };
    if cli.json {
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
        return;
    }
    for insn in &output.instructions {
        println!("{:<12} {:<24} {}", insn.address, insn.bytes, insn.text);
        for line in insn.pcode.iter().flatten() {
            println!("{:<12} {}", "", line);
        }
    }
}

fn fail(json: bool, message: &str) -> ! {
    if json {
        eprintln!("{}", serde_json::json!({ "error": message }));
    } else {
        eprintln!("error: {message}");
    }
    process::exit(1);
}
