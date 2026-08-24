mod align_is;
mod blank_lines;
mod trailing_whitespace;

/// Aligns the `is` keyword across a run of neighbouring constructors.
pub use align_is::AlignIs;
/// Collapses runs of blank lines.
pub use blank_lines::BlankLines;
/// Strips trailing whitespace from every line.
pub use trailing_whitespace::TrailingWhitespace;
