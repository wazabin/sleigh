use sleigh::{BytePos, FormatLineKind, PreparedSourceId, SleighFile, SourceDb};

use crate::{Edit, Rule};

/// Removes trailing spaces and tabs from every active source line.
///
/// Directives and inactive conditional branches are left untouched.
pub struct TrailingWhitespace;

impl Rule for TrailingWhitespace {
    fn apply(
        &self,
        _file: &SleighFile,
        sources: &SourceDb,
        prepared: PreparedSourceId,
    ) -> Vec<Edit> {
        let Some(format_lines) = sources.format_lines(prepared) else {
            return Vec::new();
        };

        let mut edits = Vec::new();
        for line in format_lines {
            if line.kind != FormatLineKind::ActiveSource {
                continue;
            }

            // Strip the line ending to isolate content characters.
            let content = line.text.trim_end_matches(['\n', '\r']);
            let trimmed = content.trim_end_matches([' ', '\t']);

            if trimmed.len() == content.len() {
                continue;
            }

            let trailing_start = line.span.start.0 + trimmed.len();
            let trailing_end = line.span.start.0 + content.len();

            if let Some(span) =
                sources.span(line.file, BytePos(trailing_start), BytePos(trailing_end))
            {
                edits.push(Edit::new(span, ""));
            }
        }
        edits
    }
}

#[cfg(test)]
mod tests {
    use sleigh::SourceDb;

    use crate::{Formatter, rules::TrailingWhitespace};

    fn apply(input: &str) -> String {
        let mut sources = SourceDb::new();
        let root = sources.add_file("test.sla", input);
        Formatter::with_rules(vec![Box::new(TrailingWhitespace)])
            .format(&mut sources, root)
            .unwrap()
            .files
            .into_iter()
            .find(|f| f.file == root)
            .unwrap()
            .content
    }

    #[test]
    fn removes_trailing_spaces() {
        assert_eq!(
            apply("define endian=little;   \n"),
            "define endian=little;\n"
        );
    }

    #[test]
    fn removes_trailing_tabs() {
        assert_eq!(
            apply("define endian=little;\t\t\n"),
            "define endian=little;\n"
        );
    }

    #[test]
    fn leaves_blank_lines_alone() {
        assert_eq!(
            apply("define endian=little;\n\n"),
            "define endian=little;\n\n"
        );
    }

    #[test]
    fn leaves_clean_line_alone() {
        let clean = "define endian=little;\n";
        assert_eq!(apply(clean), clean);
    }

    #[test]
    fn idempotent() {
        let input = "define endian=little;   \ndefine alignment=1;  \n";
        let once = apply(input);
        let twice = apply(&once);
        assert_eq!(once, twice);
    }
}
