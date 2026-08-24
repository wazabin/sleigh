use std::collections::{HashMap, HashSet};

use sleigh::{
    BytePos, ConstructorDef, FileId, FormatLineKind, PreparedSourceId, SleighFile, SleighItem,
    SourceDb,
};

use crate::{Edit, Rule};

/// Aligns the `is` keyword of consecutive constructors to the same column.
///
/// Two constructors are consecutive when there are no blank active-source lines
/// between them (in the same physical file). Alignment uses the maximum `is`
/// column in the group; spaces are inserted before the `is` keyword of each
/// under-aligned constructor.
///
/// Constructors whose `is` keyword originates from a macro expansion are
/// excluded from alignment groups (they produce no span via
/// `map_preprocessed_bytes`).
pub struct AlignIs;

impl Rule for AlignIs {
    fn apply(
        &self,
        file: &SleighFile,
        sources: &SourceDb,
        prepared: PreparedSourceId,
    ) -> Vec<Edit> {
        let format_lines = sources.format_lines(prepared).unwrap_or_default();

        let blank: HashSet<(FileId, usize)> = format_lines
            .iter()
            .filter(|l| {
                l.kind == FormatLineKind::ActiveSource
                    && l.text
                        .trim_matches(|c: char| c == '\r' || c == '\n')
                        .is_empty()
            })
            .map(|l| (l.file, l.line))
            .collect();

        let infos = collect_is_infos(file, sources, prepared);

        let mut by_file: HashMap<FileId, Vec<IsInfo>> = HashMap::new();
        for info in infos {
            by_file.entry(info.file).or_default().push(info);
        }

        let mut edits = Vec::new();
        for (file_id, mut infos) in by_file {
            infos.sort_by_key(|i| i.constructor_start_line);
            for group in split_consecutive(infos, file_id, &blank) {
                align_group(group, sources, &mut edits);
            }
        }
        edits
    }
}

struct IsInfo {
    file: FileId,
    constructor_start_line: usize,
    constructor_end_line: usize,
    is_col: usize,
    is_byte: usize,
}

fn collect_is_infos(
    file: &SleighFile,
    sources: &SourceDb,
    prepared: PreparedSourceId,
) -> Vec<IsInfo> {
    let mut result = Vec::new();
    collect_items(&file.items, sources, prepared, &mut result);
    result
}

fn collect_items(
    items: &[SleighItem],
    sources: &SourceDb,
    prepared: PreparedSourceId,
    out: &mut Vec<IsInfo>,
) {
    for item in items {
        match item {
            SleighItem::Constructor(def) => {
                if let Some(info) = is_info_for_constructor(def, sources, prepared) {
                    out.push(info);
                }
            }
            SleighItem::WithBlock(wb) => {
                collect_items(&wb.items, sources, prepared, out);
            }
            _ => {}
        }
    }
}

fn is_info_for_constructor(
    def: &ConstructorDef,
    sources: &SourceDb,
    prepared: PreparedSourceId,
) -> Option<IsInfo> {
    let constructor_span = def.span;
    let constructor_phys = sources.try_map_preprocessed_bytes(
        prepared,
        constructor_span.start.0,
        constructor_span.end.0,
    )?;

    // is_start == 0 means no display section was recorded; skip.
    if def.is_start == 0 {
        return None;
    }

    // Macro-generated `is` tokens produce None from try_map_preprocessed_bytes.
    let is_phys = sources.try_map_preprocessed_bytes(prepared, def.is_start, def.is_start + 2)?;

    // Only align when `is` is in the same file as the constructor start.
    if is_phys.file != constructor_phys.file {
        return None;
    }

    Some(IsInfo {
        file: constructor_phys.file,
        constructor_start_line: constructor_phys.start_line,
        constructor_end_line: constructor_phys.end_line,
        is_col: is_phys.start_col,
        is_byte: is_phys.start.0,
    })
}

fn split_consecutive(
    infos: Vec<IsInfo>,
    file_id: FileId,
    blank: &HashSet<(FileId, usize)>,
) -> Vec<Vec<IsInfo>> {
    if infos.is_empty() {
        return Vec::new();
    }

    let mut groups: Vec<Vec<IsInfo>> = Vec::new();
    let mut current: Vec<IsInfo> = Vec::new();

    for info in infos {
        if let Some(prev) = current.last() {
            let prev_end = prev.constructor_end_line;
            let curr_start = info.constructor_start_line;
            let has_blank = (prev_end + 1..curr_start).any(|l| blank.contains(&(file_id, l)));
            if has_blank {
                groups.push(std::mem::take(&mut current));
            }
        }
        current.push(info);
    }
    if !current.is_empty() {
        groups.push(current);
    }
    groups
}

fn align_group(group: Vec<IsInfo>, sources: &SourceDb, edits: &mut Vec<Edit>) {
    if group.len() < 2 {
        return;
    }

    let max_col = group.iter().map(|i| i.is_col).max().unwrap();

    for info in &group {
        if info.is_col >= max_col {
            continue;
        }
        let extra = max_col - info.is_col;
        if let Some(span) = sources.span(info.file, BytePos(info.is_byte), BytePos(info.is_byte)) {
            edits.push(Edit::new(span, " ".repeat(extra)));
        }
    }
}

#[cfg(test)]
mod tests {
    use sleigh::SourceDb;

    use crate::{Formatter, rules::AlignIs};

    fn apply(input: &str) -> String {
        let mut sources = SourceDb::new();
        let root = sources.add_file("test.sla", input);
        Formatter::with_rules(vec![Box::new(AlignIs)])
            .format(&mut sources, root)
            .unwrap()
            .files
            .into_iter()
            .find(|f| f.file == root)
            .unwrap()
            .content
    }

    fn with_preamble(constructors: &str) -> String {
        format!(
            concat!(
                "define endian=little;\n",
                "define space ram type=ram_space size=2 default;\n",
                "define space register type=register_space size=1;\n",
                "define register offset=0 size=1 [ A ];\n",
                "define token instr(8) op=(0,7);\n",
                "{constructors}",
            ),
            constructors = constructors
        )
    }

    #[test]
    fn aligns_consecutive_constructors() {
        let input =
            with_preamble(":A A is op=0 { A = A + 1; }\n:LONGER A is op=1 { A = A - 1; }\n");
        let output = apply(&input);

        let is_cols: Vec<usize> = output
            .lines()
            .filter(|l| l.starts_with(':'))
            .map(|l| l.find(" is ").or_else(|| l.find(" is\t")).unwrap_or(0) + 2)
            .collect();

        assert!(is_cols.len() >= 2, "should have at least two constructors");
        assert_eq!(
            is_cols[0], is_cols[1],
            "is columns should be equal:\n{output}"
        );
    }

    #[test]
    fn blank_line_breaks_alignment_group() {
        let input =
            with_preamble(":A A is op=0 { A = A + 1; }\n\n:LONGER A is op=1 { A = A - 1; }\n");
        let output = apply(&input);

        let lines: Vec<&str> = output.lines().filter(|l| l.starts_with(':')).collect();
        assert_eq!(lines.len(), 2);
        assert!(
            lines[1].starts_with(":LONGER A is"),
            "second constructor unchanged"
        );
    }

    #[test]
    fn single_constructor_not_modified() {
        let input = with_preamble(":A A is op=0 { A = A + 1; }\n");
        assert_eq!(apply(&input), input);
    }

    #[test]
    fn idempotent() {
        let input =
            with_preamble(":A A is op=0 { A = A + 1; }\n:LONGER A is op=1 { A = A - 1; }\n");
        let once = apply(&input);
        let twice = apply(&once);
        assert_eq!(once, twice);
    }
}
