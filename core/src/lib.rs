#![warn(missing_docs)]
#![warn(unreachable_pub)]
#![doc = include_str!("../README.md")]
//!
//! # Pipeline
//!
//! ```text
//! SLEIGH spec (.sla)
//!   └─ source/          ← file I/O, include resolution, preprocessing
//!        ↓
//!   syntax/             ← Pest grammar → SleighFile AST (unresolved names)
//!        ↓
//!   resolve.rs          ← SleighFile → SpecBuilder (symbol IDs resolved)
//!        ↓
//!   builder.rs          ← SpecBuilder → Spec (concretized patterns + p-code)
//!        │  uses pmacro/  (expression AST, typed IDs, macro expansion)
//!        ↓
//!   CompiledSpec
//!        ↓  HOT PATH
//!   walker.rs           ← pattern matching → ConstructorInstance
//!        ↓
//!   runtime/pcode/collect.rs  ← pre-pass: resolve fields/tables
//!   runtime/pcode/expand.rs   ← tree-walk expansion → PcodeAst
//!        ↓
//!   PcodeAst  →  harbinger/emit.rs  →  qcode IR
//! ```
//!

pub mod bitrange;
pub mod compile;
pub mod diagnostic;
pub mod pcode_error;
pub mod runtime;
pub mod semantics;
pub mod source;
/// SLEIGH source AST, for tooling that works on specification text.
///
/// **Unstable.** Behind the `unstable-syntax` feature and exempt from this
/// crate's semantic versioning: it is the shape the compiler's own front end
/// happens to use, and it changes whenever the front end does. `sleigh-fmt`
/// is the intended consumer.
#[cfg(feature = "unstable-syntax")]
pub mod syntax;
#[cfg(not(feature = "unstable-syntax"))]
pub(crate) mod syntax;

mod action;
pub(crate) mod builder;
mod constraint;
mod constructor;
mod instance;
mod lint;
pub(crate) mod objects;
mod pattern;
mod pmacro;
pub(crate) mod raw_parsing;
mod resolve;
pub(crate) mod spec;
pub(crate) mod token;
mod tree;

/// Fixed-width size/offset type for serialized SLEIGH structures.
///
/// Must stay `u32` so that specs built on x86_64 deserialize correctly on
/// 32-bit targets such as wasm32 (where `usize` is only 4 bytes).
pub type Size = u32;

pub use bitrange::BitRange;
pub use compile::{CompileOptions, Compiler};
pub use diagnostic::{CompileError, Diagnostic, DiagnosticCode, DiagnosticLabel, Severity};
pub use objects::field::FieldId;
pub use pcode_error::{PcodeError, PcodeErrorTy, PcodeResult};
pub use pcode_types::{RegisterId, SpaceId};
pub use runtime::{
    CompiledSpec, Context, ContextBytes, ContextDatabase, ContextEffect, ContextError,
    ContextScope, DecodeError, Decoder, DelaySlotError, FieldRef, Instruction, RegisterRef,
    SpaceRef, SymbolKind, SymbolRef, TableRef, TokenRef,
};
pub use semantics::{
    Builtin, EmitError, InstructionInfo, LocalVarId, PcodeAst, PcodeBinaryOp, PcodeBinop,
    PcodeDelaySlot, PcodeExpr, PcodeExprKind, PcodeIdent, PcodeLoad, PcodeRange, PcodeSpaceRef,
    PcodeStatement, PcodeStatementKind, PcodeTarget, PcodeUnaryOp, RangeParam, SemanticsSink,
};
pub use source::{
    BytePos, FileId, FormatChunk, FormatChunkKind, FormatError, FormatLine, FormatLineKind,
    IncludeEdge, MacroDefinition, MacroDefinitionId, MacroExpansion, MacroExpansionId,
    PreparedSourceId, PreprocessOptions, SourceDb, SourceOrigin, Span,
};
pub use syntax::{AnalysisResult, analyze};

/// The source AST, re-exported at the crate root for `sleigh-fmt`.
///
/// **Unstable**, like the module itself: see [`syntax`].
#[cfg(feature = "unstable-syntax")]
pub use syntax::{
    AlignmentDef, AttachStrDef, AttachValDef, AttachVarDef, BitRangeDef, BitRangeItem,
    ConstructorDef, ContextDef, EndiannessDef, FieldDef, IncludeDirective, MacroDef, ParseResult,
    PcodeOpDef, RegisterDef, SleighFile, SleighItem, SourceIndex, SpaceDef, TokenDef, TriviaToken,
    UnresolvedAction, UnresolvedDisplayToken, UnresolvedExpr, WithBlockDef, parse,
};
pub use token::BitRangeFieldId;

#[cfg(test)]
mod tests;
