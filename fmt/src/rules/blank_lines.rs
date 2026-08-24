use std::collections::HashMap;

use sleigh::{FileId, FormatLine, FormatLineKind, PreparedSourceId, SleighFile, SourceDb};

use crate::{Edit, Rule};

/// Reduces runs of consecutive blank active-source lines to at most
/// `max_consecutive`.
///
/// Directive lines and inactive conditional branches are never touched.
pub struct BlankLines {
    /// How many consecutive blank lines to allow before collapsing.
    pub max_consecutive: usize,
}

impl Rule for BlankLines {
    fn apply(
        &self,
        _file: &SleighFile,
        sources: &SourceDb,
        prepared: PreparedSourceId,
    ) -> Vec<Edit> {
        let Some(format_lines) = sources.format_lines(prepared) else {
            return Vec::new();
        };

        // Group lines by file, preserving source order within each file.
        let mut by_file: HashMap<FileId, Vec<&FormatLine>> = HashMap::new();
        for line in format_lines {
            by_file.entry(line.file).or_default().push(line);
        }

        let mut edits = Vec::new();
        for lines in by_file.values_mut() {
            lines.sort_by_key(|l| l.line);
            collect_blank_edits(lines, self.max_consecutive, &mut edits);
        }
        edits
    }
}

fn is_blank(line: &FormatLine) -> bool {
    line.kind == FormatLineKind::ActiveSource
        && line
            .text
            .trim_matches(|c: char| c == '\r' || c == '\n')
            .is_empty()
}

fn collect_blank_edits(lines: &[&FormatLine], max: usize, edits: &mut Vec<Edit>) {
    let mut run: Vec<&FormatLine> = Vec::new();

    for &line in lines {
        if is_blank(line) {
            run.push(line);
        } else {
            flush(&run, max, edits);
            run.clear();
        }
    }
    flush(&run, max, edits);
}

fn flush(run: &[&FormatLine], max: usize, edits: &mut Vec<Edit>) {
    if run.len() <= max {
        return;
    }
    for line in &run[max..] {
        edits.push(Edit::new(line.span, ""));
    }
}

#[cfg(test)]
mod tests {
    use sleigh::SourceDb;

    use crate::{Formatter, rules::BlankLines};

    fn apply(input: &str, max: usize) -> String {
        let mut sources = SourceDb::new();
        let root = sources.add_file("test.sla", input);
        Formatter::with_rules(vec![Box::new(BlankLines {
            max_consecutive: max,
        })])
        .format(&mut sources, root)
        .unwrap()
        .files
        .into_iter()
        .find(|f| f.file == root)
        .unwrap()
        .content
    }

    #[test]
    fn reduces_double_blank_to_single() {
        let input = "define endian=little;\n\n\ndefine alignment=1;\n";
        let expected = "define endian=little;\n\ndefine alignment=1;\n";
        assert_eq!(apply(input, 1), expected);
    }

    #[test]
    fn leaves_single_blank_alone() {
        let input = "define endian=little;\n\ndefine alignment=1;\n";
        assert_eq!(apply(input, 1), input);
    }

    #[test]
    fn removes_all_blanks_when_max_zero() {
        let input = "define endian=little;\n\n\ndefine alignment=1;\n";
        let expected = "define endian=little;\ndefine alignment=1;\n";
        assert_eq!(apply(input, 0), expected);
    }

    #[test]
    fn idempotent() {
        let input = "define endian=little;\n\n\n\ndefine alignment=1;\n";
        let once = apply(input, 1);
        let twice = apply(&once, 1);
        assert_eq!(once, twice);
    }
}
