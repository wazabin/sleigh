use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
};

use sleigh::{CompileOptions, CompiledSpec, Compiler, ContextBytes, SourceDb};

#[derive(serde::Deserialize)]
struct Arch {
    path: String,
    defines: Option<HashMap<String, String>>,
    context: Option<HashMap<String, u64>>,
}

fn main() {
    println!("cargo:rerun-if-changed=build_config.toml");

    let config_str = fs::read_to_string("build_config.toml").unwrap();
    let arches: HashMap<String, Arch> = toml::from_str(&config_str).unwrap();
    for (name, arch) in arches {
        if let Some(parent) = Path::new(&arch.path).parent() {
            println!("cargo:rerun-if-changed={}", parent.display());
        }

        compile_arch(&name, &arch);
    }
}

fn set_context_field(spec: &CompiledSpec, context: &mut ContextBytes, name: &str, value: u64) {
    let field = spec
        .field(name)
        .unwrap_or_else(|| panic!("Could not find field {name} in spec"));

    spec.set_context_field(context, field.id, value)
        .unwrap_or_else(|_| panic!("Could not set field {name} in spec"));
}

fn compile_arch(name: &str, arch: &Arch) {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let mut sources = SourceDb::new();
    let root = sources
        .add_file_from_path(&arch.path)
        .unwrap_or_else(|error| {
            eprintln!("Did you forget to run `just setup`?");
            panic!("Error loading {name} SLEIGH spec '{}': {error}", arch.path);
        });
    let mut compiled = Compiler::new(&mut sources)
        .with_options(CompileOptions {
            defines: arch.defines.clone().unwrap_or_default(),
        })
        .compile(root)
        .unwrap_or_else(|error| panic!("Error compiling {name}: {error}"));

    // Create the initial context
    if let Some(context_fields) = &arch.context {
        let mut context_bytes = compiled.new_context();

        for (field_name, value) in context_fields {
            set_context_field(&compiled, &mut context_bytes, field_name, *value);
        }

        compiled
            .set_context_bytes(context_bytes)
            .unwrap_or_else(|error| panic!("Error setting initial context for {name}: {error}"));
    }

    let bytes = bincode::serde::encode_to_vec(&compiled, bincode::config::standard())
        .unwrap_or_else(|error| panic!("Error serializing {name}: {error}"));
    let bin_path = out_dir.join(format!("sla/{name}_compiled.bin"));
    write_file(&bin_path, &bytes);
    println!(
        "cargo:rustc-env={}_COMPILED_SPEC={}",
        name.to_uppercase(),
        bin_path.to_str().unwrap()
    );

    let regs_path = out_dir.join(format!("regs/{name}_regs.rs"));
    write_file(&regs_path, generate_regs(&compiled).as_bytes());
    println!(
        "cargo:rustc-env={}_REGS={}",
        name.to_uppercase(),
        regs_path.to_str().unwrap()
    );
}

fn write_file(path: &PathBuf, contents: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn generate_regs(spec: &sleigh::CompiledSpec) -> String {
    spec.registers()
        .filter_map(|reg| {
            let name = reg.name();
            if name == "_" || name.is_empty() {
                return None;
            }
            let index = usize::from(reg.id);
            let const_name = name.to_uppercase();
            Some(format!(
                "/// The `{name}` register.\n\
                 pub const {const_name}: RegisterId = RegisterId::new({index});"
            ))
        })
        .collect::<Vec<_>>()
        .join("\n")
}
