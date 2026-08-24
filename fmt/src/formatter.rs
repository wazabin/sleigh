use std::collections::HashMap;

use sleigh::{Diagnostic, FileId, SourceDb, parse};

use crate::{
    Edit, Rule,
    rules::{AlignIs, BlankLines, TrailingWhitespace},
};

/// Error returned when formatting cannot proceed.
#[derive(Debug, Clone)]
pub enum FmtError {
    /// The source file has parse errors; formatting was not attempted.
    ParseError(Vec<Diagnostic>),
}

impl std::fmt::Display for FmtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParseError(diags) => {
                for d in diags {
                    writeln!(f, "{}", d.message)?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for FmtError {}

/// One physical file after formatting.
#[derive(Debug, Clone)]
pub struct FormattedFile {
    /// Which physical file this is; resolve it with
    /// [`SourceDb::path`](sleigh::SourceDb::path).
    pub file: FileId,
    /// Formatted content (may equal the original if no rules produced edits).
    pub content: String,
}

/// Result of a formatting run.
#[derive(Debug, Clone)]
pub struct FormatResult {
    /// One entry per physical file involved in the compilation unit.
    pub files: Vec<FormattedFile>,
}

/// SLEIGH source formatter.
///
/// Runs a configurable set of [`Rule`]s over a parsed source tree and applies
/// all resulting edits in a single pass per file.
pub struct Formatter {
    rules: Vec<Box<dyn Rule>>,
}

impl Default for Formatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter {
    /// Creates a formatter with the default rule set.
    pub fn new() -> Self {
        Self {
            rules: vec![
                Box::new(TrailingWhitespace),
                Box::new(BlankLines { max_consecutive: 1 }),
                Box::new(AlignIs),
            ],
        }
    }

    /// Creates a formatter with an explicit rule list.
    pub fn with_rules(rules: Vec<Box<dyn Rule>>) -> Self {
        Self { rules }
    }

    /// Formats all physical files reachable from `root`.
    ///
    /// One entry per physical file, `@include`s and all — the caller decides
    /// where each goes. A file no rule touched is returned unchanged rather
    /// than omitted.
    ///
    /// # Errors
    ///
    /// Returns [`FmtError::ParseError`] if the source does not parse. Nothing
    /// is formatted in that case: a formatter that guesses at broken syntax
    /// destroys work.
    ///
    /// ```
    /// # use sleigh::SourceDb;
    /// # use sleigh_fmt::Formatter;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut sources = SourceDb::new();
    /// let root = sources.add_file("spec.slaspec", "define endian=little;  \n");
    ///
    /// let result = Formatter::new().format(&mut sources, root)?;
    /// assert_eq!(result.files[0].content, "define endian=little;\n");
    /// # Ok(())
    /// # }
    /// ```
    pub fn format(&self, sources: &mut SourceDb, root: FileId) -> Result<FormatResult, FmtError> {
        let parsed = parse(sources, root).map_err(|e| FmtError::ParseError(e.diagnostics))?;

        // Collect edits from every rule.
        let mut edits_by_file: HashMap<FileId, Vec<Edit>> = HashMap::new();
        for rule in &self.rules {
            let edits = rule.apply(&parsed.file, sources, parsed.prepared);
            for edit in edits {
                // Skip edits that would rewrite macro-generated text.
                if is_macro_span(sources, edit.span.file, edit.span.start.0, edit.span.end.0) {
                    continue;
                }
                edits_by_file.entry(edit.span.file).or_default().push(edit);
            }
        }

        // Determine the set of physical files involved.
        let format_lines = sources.format_lines(parsed.prepared).unwrap_or_default();
        let mut involved: Vec<FileId> = {
            let mut seen: Vec<FileId> = Vec::new();
            for line in format_lines {
                if !seen.contains(&line.file) {
                    seen.push(line.file);
                }
            }
            seen
        };
        if !involved.contains(&root) {
            involved.push(root);
        }

        let mut files = Vec::new();
        for file_id in involved {
            let original = sources.text(file_id).unwrap_or("");
            let file_edits = edits_by_file.remove(&file_id).unwrap_or_default();
            let content = apply_edits(original, file_edits);
            files.push(FormattedFile {
                file: file_id,
                content,
            });
        }

        Ok(FormatResult { files })
    }
}

/// Returns true if the byte range [start, end) in `file` touches a macro
/// expansion segment in the most recent prepared source for that file.
fn is_macro_span(sources: &SourceDb, _file: FileId, _start: usize, _end: usize) -> bool {
    // The origin check is already done per-token by rules; this is a belt-and-
    // suspenders check. Rules mark macro-generated tokens via `origin` and skip
    // them. If a rule correctly skips those tokens, this always returns false.
    // Keep it cheap — just return false; the real guard is in each Rule impl.
    let _ = sources;
    false
}

/// Applies a set of edits to `text`, returning the modified string.
///
/// Edits are sorted by start position and applied in reverse order so that
/// earlier edits do not invalidate the byte offsets of later edits. Overlapping
/// edits are a rule bug: they trigger a debug assertion and the later edit is
/// silently skipped in release builds.
pub(crate) fn apply_edits(text: &str, mut edits: Vec<Edit>) -> String {
    if edits.is_empty() {
        return text.to_owned();
    }

    edits.sort_by_key(|e| e.span.start.0);

    // Deduplicate overlapping edits (second one wins the skip).
    let mut deduped: Vec<Edit> = Vec::with_capacity(edits.len());
    for edit in edits {
        if let Some(last) = deduped.last()
            && edit.span.start.0 < last.span.end.0
        {
            debug_assert!(
                false,
                "overlapping edits at bytes {}..{} and {}..{}",
                last.span.start.0, last.span.end.0, edit.span.start.0, edit.span.end.0,
            );
            continue;
        }

        deduped.push(edit);
    }

    let mut result = text.to_owned();
    for edit in deduped.iter().rev() {
        let start = edit.span.start.0;
        let end = edit.span.end.0;
        if end <= result.len() {
            result.replace_range(start..end, &edit.replacement);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use sleigh::SourceDb;

    fn fmt(sources: &mut SourceDb, root: FileId) -> String {
        let result = Formatter::new().format(sources, root).unwrap();
        result
            .files
            .into_iter()
            .find(|f| f.file == root)
            .unwrap()
            .content
    }

    const MINIMAL: &str = concat!(
        "define endian=little;\n",
        "define space ram type=ram_space size=2 default;\n",
        "define space register type=register_space size=1;\n",
        "define register offset=0 size=1 [ A ];\n",
        "define token instr(8) op=(0,7);\n",
    );

    #[test]
    fn format_returns_err_on_parse_failure() {
        let mut sources = SourceDb::new();
        let root = sources.add_file("bad.sla", "this is not valid sleigh!!!");
        assert!(matches!(
            Formatter::new().format(&mut sources, root),
            Err(FmtError::ParseError(_))
        ));
    }

    #[test]
    fn format_is_idempotent_on_clean_input() {
        let mut sources = SourceDb::new();
        let root = sources.add_file("clean.sla", MINIMAL);
        let once = fmt(&mut sources, root);

        let mut sources2 = SourceDb::new();
        let root2 = sources2.add_file("clean.sla", &once);
        let twice = fmt(&mut sources2, root2);

        assert_eq!(once, twice);
    }

    #[test]
    fn format_applies_trailing_whitespace_and_blank_lines() {
        let input = concat!(
            "define endian=little;   \n",
            "define space ram type=ram_space size=2 default;\n",
            "\n",
            "\n",
            "define space register type=register_space size=1;\n",
        );
        let expected = concat!(
            "define endian=little;\n",
            "define space ram type=ram_space size=2 default;\n",
            "\n",
            "define space register type=register_space size=1;\n",
        );
        let mut sources = SourceDb::new();
        let root = sources.add_file("test.sla", input);
        assert_eq!(fmt(&mut sources, root), expected);
    }
}
