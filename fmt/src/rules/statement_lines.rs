use sleigh::{BytePos, ConstructorDef, FileId, PreparedSourceId, SleighFile, SleighItem, SourceDb};

use crate::{Edit, Rule};

/// Puts each semantic statement of a constructor on its own indented line.
///
/// A specification often writes a whole body inline — `{ local t = a; b = t; }`
/// — which reads badly anywhere the constructor is shown on its own. This rule
/// breaks the body at the statement boundaries the parser already recorded, so
/// nothing is re-lexed and no token moves relative to another.
///
/// Whitespace that already contains a newline is left untouched: a body a human
/// has laid out keeps its shape, the rule is idempotent, and it never competes
/// with [`TrailingWhitespace`](crate::rules::TrailingWhitespace) for the same
/// bytes.
pub struct StatementLines {
    /// Spaces of indentation given to each statement inside the body.
    pub indent: usize,
}

impl Default for StatementLines {
    fn default() -> Self {
        Self { indent: 4 }
    }
}

impl Rule for StatementLines {
    fn apply(
        &self,
        file: &SleighFile,
        sources: &SourceDb,
        prepared: PreparedSourceId,
    ) -> Vec<Edit> {
        let mut edits = Vec::new();
        collect_items(&file.items, sources, prepared, self.indent, &mut edits);
        edits
    }
}

fn collect_items(
    items: &[SleighItem],
    sources: &SourceDb,
    prepared: PreparedSourceId,
    indent: usize,
    out: &mut Vec<Edit>,
) {
    for item in items {
        match item {
            SleighItem::Constructor(def) => break_body(def, sources, prepared, indent, out),
            SleighItem::WithBlock(block) => {
                collect_items(&block.items, sources, prepared, indent, out)
            }
            _ => {}
        }
    }
}

fn break_body(
    def: &ConstructorDef,
    sources: &SourceDb,
    prepared: PreparedSourceId,
    indent: usize,
    out: &mut Vec<Edit>,
) {
    let statements = def.statement_spans();
    // `def.span` is already physical (the item builder mapped it); the p-code
    // statement spans are prepared-source offsets and still need mapping.
    let constructor = def.span;
    let Some(text) = sources.text(constructor.file) else {
        return;
    };
    // A statement whose text came from a preprocessor macro expansion cannot be
    // moved — the edit would land in the wrong file — so it keeps its place
    // while the rest of the body is laid out.
    let mut anchors = Vec::with_capacity(statements.len());
    for (start, end) in statements {
        if let Some(span) = sources.try_map_preprocessed_bytes(prepared, start, end)
            && span.file == constructor.file
            && span.start.0 > constructor.start.0
            && span.end.0 <= constructor.end.0
            && starts_a_statement(text, span.start.0)
        {
            anchors.push(span.start.0);
        }
    }
    // The body's closing brace is the last one in the constructor's own text.
    let Some(closing) = text[..constructor.end.0.min(text.len())].rfind('}') else {
        return;
    };
    if let Some(last) = anchors.last()
        && closing < *last
    {
        return;
    }
    // The opening brace precedes the first statement; the display section is
    // before `is`, so a `{` written there can never be picked up here. With no
    // statements the body is empty, and the two braces face each other.
    let search_end = anchors.first().copied().unwrap_or(closing);
    let Some(opening) = text[..search_end].rfind('{') else {
        return;
    };
    if opening < constructor.start.0 {
        return;
    }
    if anchors.is_empty() && !text[opening + 1..closing].trim().is_empty() {
        // A body whose statements could not be located is left as written.
        return;
    }
    let mut breaks = vec![(opening, 0usize)];
    breaks.extend(anchors.iter().map(|start| (*start, indent)));
    breaks.push((closing, 0));
    for (position, width) in breaks {
        if let Some(edit) = break_before(sources, text, constructor.file, position, width) {
            out.push(edit);
        }
    }
}

/// Lays out the blanks before `position` as a line break plus `indent` spaces.
///
/// A run that already contains a newline keeps it: only the indentation after
/// the last newline is rewritten, so the edit never covers the trailing
/// whitespace another rule owns, and a second run changes nothing.
fn break_before(
    sources: &SourceDb,
    text: &str,
    file: FileId,
    position: usize,
    indent: usize,
) -> Option<Edit> {
    let mut start = position;
    let mut newline = None;
    for (offset, ch) in text[..position].char_indices().rev() {
        match ch {
            ' ' | '\t' => start = offset,
            '\n' | '\r' => {
                start = offset;
                newline = Some(offset);
                break;
            }
            _ => break,
        }
    }
    if start == 0 {
        return None;
    }
    let (start, replacement) = match newline {
        Some(offset) => (offset + 1, " ".repeat(indent)),
        None => (start, format!("\n{}", " ".repeat(indent))),
    };
    if text[start..position] == replacement {
        return None;
    }
    let span = sources.span(file, BytePos(start), BytePos(position))?;
    Some(Edit::new(span, replacement))
}

/// Whether `position` in `text` can be the first byte of a statement: the last
/// thing before it, comments and blanks aside, opened or ended one.
///
/// The layout anchors are byte offsets the parser recorded; this is the check
/// that one of them really lands where a statement begins, so a body the rule
/// cannot place correctly is left alone instead of broken in the wrong spot.
fn starts_a_statement(text: &str, position: usize) -> bool {
    let mut rest = &text[..position];
    loop {
        let trimmed = rest.trim_end();
        // Step back over a whole trailing comment line, then keep looking.
        if let Some(line_start) = trimmed.rfind('\n')
            && let Some(hash) = trimmed[line_start..].find('#')
            && trimmed[line_start + hash..].lines().count() == 1
        {
            rest = &trimmed[..line_start + hash];
            continue;
        }
        return matches!(
            trimmed.chars().next_back(),
            Some('{') | Some(';') | Some('}')
        );
    }
}
