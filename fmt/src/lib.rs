#![warn(missing_docs)]
//! A formatter for SLEIGH processor specifications.
//!
//! Formats `.slaspec` and `.sinc` text the way `rustfmt` formats Rust: parse,
//! collect edits from a set of [`Rule`]s, apply them in one pass per physical
//! file. Nothing is reformatted from the AST — the original bytes are edited —
//! so anything no rule has an opinion about is left exactly as written.
//!
//! ```
//! use sleigh::SourceDb;
//! use sleigh_fmt::Formatter;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut sources = SourceDb::new();
//!     let root = sources.add_file(
//!         "untidy.slaspec",
//!         "define endian=little;   \n\n\n\ndefine alignment=1;\n",
//!     );
//!
//!     let result = Formatter::new().format(&mut sources, root)?;
//!
//!     // Trailing whitespace gone, runs of blank lines collapsed.
//!     assert_eq!(
//!         result.files[0].content,
//!         "define endian=little;\n\ndefine alignment=1;\n"
//!     );
//!     Ok(())
//! }
//! ```
//!
//! # Includes
//!
//! A specification is usually several files stitched together by `@include`.
//! [`Formatter::format`] takes the root and returns one [`FormattedFile`] per
//! *physical* file it reached, so a caller writes each back to its own path.
//! Text that came from a preprocessor macro expansion is never edited — the
//! edit would land in the wrong file.
//!
//! # Stability
//!
//! This crate works on the SLEIGH source AST, which `sleigh` exposes behind
//! its `unstable-syntax` feature. That AST is exempt from semantic
//! versioning, so this crate tracks `sleigh` closely.

/// Byte-range replacements, the unit a rule produces.
pub mod edit;
/// The formatter itself, and the result of running it.
pub mod formatter;
/// The [`Rule`] trait every formatting rule implements.
pub mod rule;
/// The rules shipped with this crate.
pub mod rules;

pub use edit::Edit;
pub use formatter::{FmtError, FormatResult, FormattedFile, Formatter};
pub use rule::Rule;

#[cfg(test)]
mod tests;
