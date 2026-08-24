use sleigh::{PreparedSourceId, SleighFile, SourceDb};

use crate::Edit;

/// A formatting rule that inspects a parsed SLEIGH AST and returns a set of
/// edits to apply to the physical source files.
///
/// Rules receive the full typed AST and may produce edits spanning any number
/// of physical files.
pub trait Rule {
    /// Returns every edit this rule wants made to `file`.
    ///
    /// Edits are collected from all rules and applied together, so a rule must
    /// not assume it sees the result of another. Two rules editing overlapping
    /// spans is a bug in the rule set.
    ///
    /// `prepared` identifies the preprocessed stream the AST came from, for
    /// rules that need to map back to physical source.
    fn apply(&self, file: &SleighFile, sources: &SourceDb, prepared: PreparedSourceId)
    -> Vec<Edit>;
}
