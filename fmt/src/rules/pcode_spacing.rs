use sleigh::{ConstructorDef, PreparedSourceId, SleighFile, SleighItem, SourceDb};

use crate::{Edit, Rule};

/// Normalizes the whitespace *inside* each p-code statement.
///
/// Specifications accumulate ad-hoc spacing — `subflags(   AL,imm8 )`,
/// `tmp  =   a f+  b` — which reads badly beside statements written by someone
/// else. This rule reprints the run of tokens the parser found for a statement
/// with one consistent spacing, without touching the pattern, the display
/// section, or anything outside a semantic body.
///
/// Only spacing changes: every token, comment and their order survive, so a
/// reprinted statement parses to the same p-code as the one it replaced.
pub struct PcodeSpacing;

impl Rule for PcodeSpacing {
    fn apply(
        &self,
        file: &SleighFile,
        sources: &SourceDb,
        prepared: PreparedSourceId,
    ) -> Vec<Edit> {
        let mut edits = Vec::new();
        collect_items(&file.items, sources, prepared, &mut edits);
        edits
    }
}

fn collect_items(
    items: &[SleighItem],
    sources: &SourceDb,
    prepared: PreparedSourceId,
    out: &mut Vec<Edit>,
) {
    for item in items {
        match item {
            SleighItem::Constructor(def) => respace(def, sources, prepared, out),
            SleighItem::WithBlock(block) => collect_items(&block.items, sources, prepared, out),
            _ => {}
        }
    }
}

fn respace(
    def: &ConstructorDef,
    sources: &SourceDb,
    prepared: PreparedSourceId,
    out: &mut Vec<Edit>,
) {
    // `def.span` is physical; statement spans are prepared offsets.
    let Some(text) = sources.text(def.span.file) else {
        return;
    };
    for (start, end) in def.statement_spans() {
        let Some(span) = sources.try_map_preprocessed_bytes(prepared, start, end) else {
            continue; // Macro-expanded text belongs to no single physical file.
        };
        if span.file != def.span.file || span.end.0 > text.len() {
            continue;
        }
        let original = &text[span.start.0..span.end.0];
        // A statement broken across lines is laid out by hand; respacing it
        // would join those lines, so leave it alone.
        if original.contains('\n') || original.contains('#') {
            continue;
        }
        let respaced = respace_statement(original);
        if respaced != original {
            out.push(Edit::new(span, respaced));
        }
    }
}

#[derive(PartialEq, Clone, Copy)]
enum Token {
    Word,
    Operator,
    Open,
    Close,
    Comma,
    Colon,
    Semicolon,
    Label,
    String,
}

/// Splits a statement into (kind, text) tokens.
///
/// Two spellings are kept whole because they only mean what they mean unspaced:
/// the typed operators (`f+`, `s>>`) and `<label>` targets.
fn tokenize(text: &str) -> Vec<(Token, &str)> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let ch = bytes[index] as char;
        if ch.is_whitespace() {
            index += 1;
        } else if ch == '"' {
            let mut stop = index + 1;
            while stop < bytes.len() && bytes[stop] != b'"' {
                stop += 1;
            }
            stop = (stop + 1).min(bytes.len());
            tokens.push((Token::String, &text[index..stop]));
            index = stop;
        } else if ch == '<'
            && let Some(close) = text[index..].find('>')
            && text[index + 1..index + close]
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
            && close > 1
        {
            tokens.push((Token::Label, &text[index..index + close + 1]));
            index += close + 1;
        } else if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '$' {
            let mut stop = index;
            while stop < bytes.len()
                && ((bytes[stop] as char).is_ascii_alphanumeric()
                    || bytes[stop] == b'_'
                    || bytes[stop] == b'.'
                    || bytes[stop] == b'$')
            {
                stop += 1;
            }
            let word = &text[index..stop];
            // `f+`, `s>>`: a type prefix glued to the operator it types.
            if (word == "f" || word == "s") && stop < bytes.len() && is_operator_byte(bytes[stop]) {
                let mut op_end = stop;
                while op_end < bytes.len() && is_operator_byte(bytes[op_end]) {
                    op_end += 1;
                }
                tokens.push((Token::Operator, &text[index..op_end]));
                index = op_end;
            } else {
                tokens.push((Token::Word, word));
                index = stop;
            }
        } else if is_operator_byte(bytes[index]) {
            let mut stop = index;
            while stop < bytes.len() && is_operator_byte(bytes[stop]) {
                stop += 1;
            }
            tokens.push((Token::Operator, &text[index..stop]));
            index = stop;
        } else {
            let kind = match ch {
                '(' | '[' => Token::Open,
                ')' | ']' => Token::Close,
                ',' => Token::Comma,
                ':' => Token::Colon,
                ';' => Token::Semicolon,
                _ => Token::Word,
            };
            tokens.push((kind, &text[index..index + 1]));
            index += 1;
        }
    }
    tokens
}

fn is_operator_byte(byte: u8) -> bool {
    matches!(
        byte,
        b'=' | b'+'
            | b'-'
            | b'*'
            | b'/'
            | b'%'
            | b'!'
            | b'<'
            | b'>'
            | b'&'
            | b'|'
            | b'^'
            | b'~'
            | b'@'
    )
}

/// True when an operator in this position takes one operand on its right.
fn is_unary(previous: Option<(Token, &str)>) -> bool {
    match previous {
        None => true,
        Some((Token::Operator | Token::Open | Token::Comma | Token::Colon, _)) => true,
        Some((Token::Word, word)) => matches!(word, "goto" | "call" | "return" | "if" | "export"),
        _ => false,
    }
}

fn respace_statement(text: &str) -> String {
    let indent: String = text
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect();
    let tokens = tokenize(text);
    let mut out = String::with_capacity(text.len());
    let mut bracket_depth = 0usize;
    let mut previous: Option<(Token, &str)> = None;
    for (index, (kind, value)) in tokens.iter().copied().enumerate() {
        let space = match kind {
            Token::Semicolon | Token::Comma | Token::Close | Token::Colon => false,
            Token::Open if value == "(" => matches!(
                previous,
                Some((Token::Word, "if" | "goto" | "call" | "return" | "export"))
            ),
            Token::Open => false,
            Token::Operator if is_unary(previous) => !matches!(previous, Some((Token::Open, _))),
            _ => match previous {
                None => false,
                Some((Token::Open, _)) | Some((Token::Colon, _)) => false,
                Some((Token::Comma, _)) => bracket_depth == 0,
                Some((Token::Operator, op)) => {
                    !(is_unary(tokens.get(index.wrapping_sub(2)).copied())
                        && op.len() <= 2
                        && matches!(op, "-" | "!" | "~" | "*" | "&"))
                }
                _ => true,
            },
        };
        if space && !out.is_empty() {
            out.push(' ');
        }
        out.push_str(value);
        match kind {
            Token::Open if value == "[" => bracket_depth += 1,
            Token::Close if value == "]" => bracket_depth = bracket_depth.saturating_sub(1),
            _ => {}
        }
        previous = Some((kind, value));
    }
    format!("{indent}{out}")
}
