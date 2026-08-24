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
}
