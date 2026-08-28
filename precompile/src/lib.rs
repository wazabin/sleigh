#![warn(missing_docs)]
//! SLEIGH specifications, compiled at build time.
//!
//! Compiling a real processor specification takes hundreds of milliseconds —
//! too long to do at every program start, and far too long to do in a test.
//! This crate compiles a fixed set once, during its own build, serialises the
//! result, and embeds it. Loading is then a deserialisation on first use,
//! cached in a `OnceLock`.
//!
//! ```no_run
//! use sleigh::Decoder;
//!
//! let spec = sleigh_precompile::x64::spec();
//! let instruction = Decoder::new(spec)
//!     .decode_one(0x1000, &[0x90], &spec.new_context())
//!     .expect("NOP decodes");
//! assert_eq!(instruction.display().unwrap(), "NOP");
//! ```
//!
//! Which specifications are built is configured in `build_config.toml`, and
//! their sources come from the vendored Ghidra corpus in `open_sleigh/`. Each
//! module also exposes a `regs` submodule of named
//! [`RegisterId`](sleigh::RegisterId) constants, generated alongside the
//! specification so a consumer can refer to registers without a name lookup.
//!
//! The embedded form is `bincode` over this crate's `sleigh` dependency, so
//! it is only valid for that exact version — which is why it is regenerated
//! by the build script rather than checked in.

use sleigh::CompiledSpec;
use std::sync::OnceLock;

/// The instruction family each constructor belongs to.
///
/// A specification records this in `#@family` comments — see
/// [`sleigh::annotate`] — and the build script reads them, so this is a lookup
/// table rather than a computation. A specification that carries no markers
/// yields an empty set; only the x86 ones are annotated today.
#[derive(Debug, Clone, Copy)]
pub struct Families {
    /// Sorted by `(table, index)`, so lookup can binary-search.
    entries: &'static [(&'static str, usize, &'static str)],
}

impl Families {
    /// The family of one constructor, or `None` if it is unannotated.
    ///
    /// `table` and `index` are what the decoder reports for each
    /// [`ConstructorMatch`](sleigh::ConstructorMatch), so a consumer that
    /// records which constructors it reached can look each one up directly.
    pub fn get(&self, table: &str, index: usize) -> Option<&'static str> {
        self.entries
            .binary_search_by(|(name, idx, _)| (*name, *idx).cmp(&(table, index)))
            .ok()
            .map(|found| self.entries[found].2)
    }

    /// Every annotated constructor, as `(table, index, family)`, sorted.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, usize, &'static str)> {
        self.entries.iter().copied()
    }

    /// How many constructors carry a family.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether this specification carries no family markers at all.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests;

/// x86-64.
pub mod x64 {
    use super::*;

    const SPEC_BYTES: &[u8] = include_bytes!(env!("X64_COMPILED_SPEC"));
    static SPEC: OnceLock<CompiledSpec> = OnceLock::new();

    /// The compiled specification, deserialised on first use.
    pub fn spec() -> &'static CompiledSpec {
        SPEC.get_or_init(|| {
            bincode::serde::decode_from_slice(SPEC_BYTES, bincode::config::standard())
                .map(|(spec, _)| spec)
                .expect("x64 spec should deserialize")
        })
    }

    /// Named [`RegisterId`](sleigh::RegisterId) constants for this
    /// specification, generated at build time.
    pub mod regs {
        use sleigh::RegisterId;
        include!(env!("X64_REGS"));
    }

    mod family_table {
        include!(env!("X64_FAMILIES"));
    }

    /// The `#@family` annotations on this specification's constructors.
    pub fn families() -> Families {
        Families {
            entries: &family_table::FAMILIES,
        }
    }
}

/// 32-bit x86.
pub mod x86 {
    use super::*;

    const SPEC_BYTES: &[u8] = include_bytes!(env!("X86_COMPILED_SPEC"));
    static SPEC: OnceLock<CompiledSpec> = OnceLock::new();

    /// The compiled specification, deserialised on first use.
    pub fn spec() -> &'static CompiledSpec {
        SPEC.get_or_init(|| {
            bincode::serde::decode_from_slice(SPEC_BYTES, bincode::config::standard())
                .map(|(spec, _)| spec)
                .expect("x86 spec should deserialize")
        })
    }

    /// Named [`RegisterId`](sleigh::RegisterId) constants for this
    /// specification, generated at build time.
    pub mod regs {
        use sleigh::RegisterId;
        include!(env!("X86_REGS"));
    }

    mod family_table {
        include!(env!("X86_FAMILIES"));
    }

    /// The `#@family` annotations on this specification's constructors.
    pub fn families() -> Families {
        Families {
            entries: &family_table::FAMILIES,
        }
    }
}

/// 64-bit RISC-V.
pub mod riscv {
    use super::*;

    const SPEC_BYTES: &[u8] = include_bytes!(env!("RISCV_COMPILED_SPEC"));
    static SPEC: OnceLock<CompiledSpec> = OnceLock::new();

    /// The compiled specification, deserialised on first use.
    pub fn spec() -> &'static CompiledSpec {
        SPEC.get_or_init(|| {
            bincode::serde::decode_from_slice(SPEC_BYTES, bincode::config::standard())
                .map(|(spec, _)| spec)
                .expect("riscv spec should deserialize")
        })
    }

    /// Named [`RegisterId`](sleigh::RegisterId) constants for this
    /// specification, generated at build time.
    pub mod regs {
        use sleigh::RegisterId;
        include!(env!("RISCV_REGS"));
    }

    mod family_table {
        include!(env!("RISCV_FAMILIES"));
    }

    /// The `#@family` annotations on this specification's constructors.
    pub fn families() -> Families {
        Families {
            entries: &family_table::FAMILIES,
        }
    }
}

/// AArch64.
pub mod aarch64 {
    use super::*;

    const SPEC_BYTES: &[u8] = include_bytes!(env!("AARCH64_COMPILED_SPEC"));
    static SPEC: OnceLock<CompiledSpec> = OnceLock::new();

    /// The compiled specification, deserialised on first use.
    pub fn spec() -> &'static CompiledSpec {
        SPEC.get_or_init(|| {
            bincode::serde::decode_from_slice(SPEC_BYTES, bincode::config::standard())
                .map(|(spec, _)| spec)
                .expect("aarch64 spec should deserialize")
        })
    }

    /// Named [`RegisterId`](sleigh::RegisterId) constants for this
    /// specification, generated at build time.
    pub mod regs {
        use sleigh::RegisterId;
        include!(env!("AARCH64_REGS"));
    }

    mod family_table {
        include!(env!("AARCH64_FAMILIES"));
    }

    /// The `#@family` annotations on this specification's constructors.
    pub fn families() -> Families {
        Families {
            entries: &family_table::FAMILIES,
        }
    }
}
