//! Browser playground for `wazabin-sleigh`.
//!
//! Mirrors the `sleigh-compile` and `sleigh-disasm` examples' JSON shapes.
//! [`compile`] compiles source text and keeps the result in a thread-local so
//! [`decode`] can decode against it without recompiling. [`presets`] lists
//! the small self-contained toy specifications the toolbar offers; each is
//! ordinary editable source, not a precompiled bundle. [`highlight`] lexes
//! source for the editor.

use std::cell::RefCell;

use serde::{Deserialize, Serialize};
use sleigh::{
    CompileOptions, CompiledSpec, Compiler, ContextDatabase, Decoder, SourceDb, highlight,
};
use wasm_bindgen::prelude::*;

/// The toy specifications the toolbar offers: `(name, source, bytes, address)`.
///
/// Each is a self-contained ISA small enough to read in one sitting, with a
/// byte string that decodes to a sensible program at the given address.
const PRESETS: &[(&str, &str, &str, &str)] = &[
    (
        "toy8",
        include_str!("../presets/toy8.slaspec"),
        "16203b4450fa00",
        "0x0",
    ),
    (
        "toy16",
        include_str!("../presets/toy16.slaspec"),
        "12052464065046 0267fc8000",
        "0x100",
    ),
    (
        "toymode",
        include_str!("../presets/toymode.slaspec"),
        "302a0430341221081140 0010",
        "0x0",
    ),
];

thread_local! {
    static COMPILED: RefCell<Option<(SourceDb, CompiledSpec)>> = const { RefCell::new(None) };
}

// ── compile ──────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct CompileArgs {
    source: String,
    defines: std::collections::HashMap<String, String>,
    context: std::collections::HashMap<String, u64>,
    lint: bool,
}

#[derive(Serialize)]
struct CompileOutput {
    ok: bool,
    diagnostics: Vec<DiagnosticOut>,
    registers: usize,
    tables: usize,
    context_fields: usize,
    spaces: Vec<SpaceOut>,
    default_space: Option<String>,
    compile_ms: f64,
}

#[derive(Serialize)]
struct DiagnosticOut {
    severity: String,
    message: String,
    line: usize,
    column: usize,
    rendered: String,
}

#[derive(Serialize)]
struct SpaceOut {
    name: Option<String>,
    size: Option<usize>,
    wordsize: Option<usize>,
}

/// Compiles `args.source` and returns the `sleigh-compile` output shape.
///
/// Keeps the compiled spec (and its [`SourceDb`], which diagnostics need to
/// render) in a thread-local so [`decode`] can use it without recompiling.
#[wasm_bindgen]
pub fn compile(args: &str) -> String {
    match serde_json::from_str::<CompileArgs>(args) {
        Ok(args) => run_compile(&args),
        Err(e) => error(&format!("invalid arguments: {e}")),
    }
}

fn run_compile(args: &CompileArgs) -> String {
    if args.source.contains("#include") {
        return error(
            "this spec uses #include, which needs a filesystem the browser doesn't have; \
             paste a self-contained .slaspec instead",
        );
    }

    let mut sources = SourceDb::new();
    let root = sources.add_file("input.slaspec", args.source.clone());

    let start = now();
    let compiler = Compiler::new(&mut sources).with_options(CompileOptions {
        defines: args.defines.clone(),
    });
    let compiled = if args.lint {
        compiler.compile_with_lints(root)
    } else {
        compiler.compile(root).map(|spec| (spec, Vec::new()))
    };
    let compile_ms = elapsed_ms(start);

    let (mut spec, lint_diagnostics) = match compiled {
        Ok(result) => result,
        Err(error) => {
            let diagnostics = error
                .diagnostics()
                .iter()
                .map(|d| diagnostic_out(d, &sources))
                .collect();
            COMPILED.with(|cell| *cell.borrow_mut() = None);
            return serde_json::to_string(&CompileOutput {
                ok: false,
                diagnostics,
                registers: 0,
                tables: 0,
                context_fields: 0,
                spaces: Vec::new(),
                default_space: None,
                compile_ms,
            })
            .unwrap();
        }
    };

    // Apply the requested initial context, same as sleigh-compile --context.
    let mut context = spec.new_context();
    let mut context_error = None;
    for (field_name, value) in &args.context {
        match spec.field(field_name) {
            Some(field) => {
                if let Err(e) = spec.set_context_field(&mut context, field.id, *value) {
                    context_error = Some(format!("setting '{field_name}' failed: {e:?}"));
                    break;
                }
            }
            None => {
                context_error =
                    Some(format!("no context field '{field_name}' in this specification"));
                break;
            }
        }
    }
    if let Some(e) = &context_error {
        return error(e);
    }
    if !args.context.is_empty()
        && let Err(e) = spec.set_context_bytes(context)
    {
        return error(&format!("setting the initial context failed: {e:?}"));
    }

    let registers = spec.registers().count();
    let tables = spec
        .symbols()
        .filter(|s| matches!(s.kind, sleigh::SymbolKind::Table))
        .count();
    let context_fields = spec
        .symbols()
        .filter(|s| matches!(s.kind, sleigh::SymbolKind::Field))
        .count();
    let spaces: Vec<SpaceOut> = spec
        .spaces()
        .map(|s| SpaceOut {
            name: s.name().map(str::to_string),
            size: Some(s.address_size()),
            wordsize: Some(s.word_size()),
        })
        .collect();
    let default_space = spec
        .spaces()
        .find(|s| s.id == spec.default_space())
        .and_then(|s| s.name().map(str::to_string));

    let diagnostics = lint_diagnostics
        .iter()
        .map(|d| diagnostic_out(d, &sources))
        .collect();

    let output = CompileOutput {
        ok: true,
        diagnostics,
        registers,
        tables,
        context_fields,
        spaces,
        default_space,
        compile_ms,
    };
    COMPILED.with(|cell| *cell.borrow_mut() = Some((sources, spec)));
    serde_json::to_string(&output).unwrap()
}

fn diagnostic_out(d: &sleigh::Diagnostic, sources: &SourceDb) -> DiagnosticOut {
    let span = d.span();
    DiagnosticOut {
        severity: format!("{:?}", d.severity),
        message: d.message.clone(),
        line: span.start_line,
        column: span.start_col,
        rendered: d.render(sources),
    }
}

// ── decode ───────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct DecodeArgs {
    bytes: String,
    address: String,
    count: Option<usize>,
    pcode: bool,
}

#[derive(Serialize)]
struct DecodeOutput {
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

/// Decodes against the last spec passed to [`compile`].
#[wasm_bindgen]
pub fn decode(args: &str) -> String {
    let args = match serde_json::from_str::<DecodeArgs>(args) {
        Ok(args) => args,
        Err(e) => return error(&format!("invalid arguments: {e}")),
    };
    COMPILED.with(|cell| match &*cell.borrow() {
        Some((_, spec)) => run_decode(spec, &args),
        None => error("no spec compiled yet; compile one first"),
    })
}

fn parse_int(what: &str, value: &str) -> Result<u64, String> {
    let value = value.trim();
    match value.strip_prefix("0x").or_else(|| value.strip_prefix("0X")) {
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

fn run_decode(spec: &CompiledSpec, args: &DecodeArgs) -> String {
    let data = match parse_hex(&args.bytes) {
        Ok(data) => data,
        Err(e) => return error(&e),
    };
    let address = match parse_int("address", &args.address) {
        Ok(a) => a,
        Err(e) => return error(&e),
    };

    let decoder = Decoder::new(spec);
    // A linear sweep with `globalset` effects carried forward, so a spec
    // whose instructions switch modes decodes the way a disassembler would.
    let mut context = ContextDatabase::new(spec);
    let limit = args.count.unwrap_or(usize::MAX);
    let mut instructions = Vec::new();
    let mut cursor = 0usize;
    while instructions.len() < limit && cursor < data.len() {
        let at = address + cursor as u64;
        let decoded = decoder.decode_one(at, &data[cursor..], &context.context_at(at));
        let instruction = match decoded {
            Ok(instruction) => instruction,
            Err(e) if instructions.is_empty() => {
                return error(&format!("no instruction decodes at {at:#x}: {e:?}"));
            }
            Err(_) => break,
        };
        context.apply(&instruction);
        let len = instruction.len();
        let pcode = if args.pcode {
            match instruction.pcode_ast() {
                Ok(ast) => Some(ast.pretty_print(spec).lines().map(str::to_string).collect()),
                Err(e) => return error(&format!("p-code emission failed at {at:#x}: {e}")),
            }
        } else {
            None
        };
        instructions.push(Insn {
            address: format!("{at:#x}"),
            bytes: data[cursor..cursor + len].iter().map(|b| format!("{b:02x}")).collect(),
            text: instruction.to_string(),
            pcode,
        });
        cursor += len;
    }
    serde_json::to_string(&DecodeOutput { instructions }).unwrap()
}

// ── presets / highlight ──────────────────────────────────────────────────

/// The toy specifications the toolbar offers, as
/// `[{name, source, bytes, address}]`.
#[wasm_bindgen]
pub fn presets() -> String {
    let list: Vec<_> = PRESETS
        .iter()
        .map(|(name, source, bytes, address)| {
            serde_json::json!({
                "name": name,
                "source": source,
                "bytes": bytes,
                "address": address,
            })
        })
        .collect();
    serde_json::Value::Array(list).to_string()
}

/// Lexes `source` for the editor, as `[[start, end, kind], ...]` with byte
/// offsets and the lowercase [`highlight::TokenKind`] name.
#[wasm_bindgen]
pub fn highlight(source: &str) -> String {
    let list: Vec<_> = highlight::tokens(source)
        .into_iter()
        .map(|t| {
            serde_json::json!([t.start, t.end, format!("{:?}", t.kind).to_lowercase()])
        })
        .collect();
    serde_json::Value::Array(list).to_string()
}

// ── helpers ──────────────────────────────────────────────────────────────

fn error(message: &str) -> String {
    serde_json::json!({ "error": message }).to_string()
}

fn now() -> f64 {
    web_time_now()
}

fn elapsed_ms(start: f64) -> f64 {
    web_time_now() - start
}

#[wasm_bindgen(inline_js = "export function web_time_now() { return performance.now(); }")]
extern "C" {
    fn web_time_now() -> f64;
}
