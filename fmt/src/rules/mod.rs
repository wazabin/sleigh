mod align_is;
mod blank_lines;
mod pcode_spacing;
mod statement_lines;
mod trailing_whitespace;

/// Aligns the `is` keyword across a run of neighbouring constructors.
pub use align_is::AlignIs;
/// Collapses runs of blank lines.
pub use blank_lines::BlankLines;
/// Normalizes whitespace inside p-code statements.
pub use pcode_spacing::PcodeSpacing;
/// Breaks a constructor's semantic body one statement per line.
pub use statement_lines::StatementLines;
/// Strips trailing whitespace from every line.
pub use trailing_whitespace::TrailingWhitespace;
