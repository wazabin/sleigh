use std::path::{Path, PathBuf};

use sleigh::{CompiledSpec, Compiler, SourceDb};

use crate::Formatter;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("core/src/tests/fixtures")
}

/// Sorted `(name, kind)` pairs drawn from the spec's public symbol table.
/// Used to compare two compiled specs for semantic equivalence.
fn spec_fingerprint(spec: &CompiledSpec) -> Vec<(String, String)> {
    let mut symbols: Vec<_> = spec
        .symbols()
        .map(|s| (s.name.to_owned(), format!("{:?}", s.kind)))
        .collect();
    symbols.sort();
    symbols
}

/// Core assertion: `compile(x) == compile(fmt(x))`.
///
/// * If both the original and the formatter fail to parse the source, the test
///   passes — formatting cannot be expected to handle unparseable input, and
///   both sides are consistently failing.
/// * If the original compiles but formatting fails (or vice-versa), the test
///   fails immediately.
/// * If both succeed, the sorted symbol fingerprints are compared.
fn check_fixture(root: &Path) {
    let mut sources = SourceDb::new();
    let root_id = sources.add_file_from_path(root).unwrap();

    let original_result = Compiler::new(&mut sources).compile(root_id);
    let format_result = Formatter::new().format(&mut sources, root_id);

    match (original_result, format_result) {
        (Err(_), Err(_)) => {
            // Both fail — the formatter is consistent with the compiler.
        }

        (Ok(original_spec), Ok(formatted)) => {
            // Rebuild a SourceDb from the formatted content, preserving the
            // original paths so that @include directives resolve correctly.
            let mut fmt_sources = SourceDb::new();
            for ff in &formatted.files {
                let path = sources.path(ff.file).unwrap().to_owned();
                fmt_sources.add_file(path, ff.content.clone());
            }
            let fmt_root = fmt_sources.file_by_path(root).unwrap();

            let fmt_spec = Compiler::new(&mut fmt_sources)
                .compile(fmt_root)
                .unwrap_or_else(|e| {
                    panic!(
                        "compile(fmt(x)) failed for {:?}: {}",
                        root,
                        e.diagnostics()
                            .iter()
                            .map(|d| d.message.as_str())
                            .collect::<Vec<_>>()
                            .join("; ")
                    )
                });

            assert_eq!(
                spec_fingerprint(&original_spec),
                spec_fingerprint(&fmt_spec),
                "compile(x) != compile(fmt(x)) for {root:?}",
            );
        }

        (Ok(_), Err(e)) => {
            panic!("format failed but compile succeeded for {root:?}: {e}");
        }

        (Err(_), Ok(_)) => {
            panic!("format succeeded but compile failed for {root:?}");
        }
    }
}

#[test]
fn fixture_single() {
    check_fixture(&fixtures_dir().join("single/root.sla"));
}

#[test]
fn fixture_example() {
    check_fixture(&fixtures_dir().join("example.sla"));
}

#[test]
fn fixture_conditional() {
    check_fixture(&fixtures_dir().join("conditional/root.sla"));
}

#[test]
fn fixture_include_resolution() {
    check_fixture(&fixtures_dir().join("include_resolution/root.sla"));
}

#[test]
fn fixture_context_update() {
    check_fixture(&fixtures_dir().join("context_update/root.sla"));
}

#[test]
fn fixture_semantic_assignment() {
    check_fixture(&fixtures_dir().join("semantic_assignment/root.sla"));
}

#[test]
fn fixture_semantics_branching() {
    check_fixture(&fixtures_dir().join("semantics/branching.sla"));
}

#[test]
fn fixture_semantics_build_export() {
    check_fixture(&fixtures_dir().join("semantics/build_export.sla"));
}

#[test]
fn fixture_semantics_expressions() {
    check_fixture(&fixtures_dir().join("semantics/expressions.sla"));
}

#[test]
fn fixture_semantics_load_store() {
    check_fixture(&fixtures_dir().join("semantics/load_store.sla"));
}

#[test]
fn fixture_semantics_userop_macro() {
    check_fixture(&fixtures_dir().join("semantics/userop_macro.sla"));
}

#[test]
fn fixture_nested_inactive() {
    check_fixture(&fixtures_dir().join("nested_inactive/root.sla"));
}

#[test]
fn fixture_inactive_missing_include() {
    check_fixture(&fixtures_dir().join("inactive_missing_include/root.sla"));
}

#[test]
fn fixture_malformed() {
    // Malformed source fails to parse; both compile and format must fail.
    check_fixture(&fixtures_dir().join("malformed/root.sla"));
}

#[test]
fn fixture_preprocessor_comment() {
    check_fixture(&fixtures_dir().join("preprocessor_comment/root.sla"));
}

#[test]
fn fixture_preprocessor_comment_preserves_directives_with_inline_comments() {
    let root = fixtures_dir().join("preprocessor_comment/root.sla");
    let mut sources = SourceDb::new();
    let root_id = sources.add_file_from_path(&root).unwrap();
    let result = Formatter::new().format(&mut sources, root_id).unwrap();
    let content = &result
        .files
        .iter()
        .find(|f| f.file == root_id)
        .unwrap()
        .content;

    assert!(
        content.contains("@define MODE \"on\" # enable the active token"),
        "directive with inline comment should be preserved:\n{content}"
    );
    assert!(
        content.contains("@else # inactive branch"),
        "@else with inline comment should be preserved:\n{content}"
    );
    assert!(
        content.contains("# this comment sits between directives"),
        "standalone comment line should be preserved:\n{content}"
    );
}

#[test]
fn fixture_include_cycle() {
    // The cycle is caught by the preprocessor's path-dedup logic, so both
    // compile and format fail consistently.
    check_fixture(&fixtures_dir().join("include_cycle/root.sla"));
}

#[test]
fn statement_lines_breaks_an_inline_body() {
    use crate::rules::StatementLines;
    let mut sources = SourceDb::new();
    let root = sources.add_file(
        "spec.slaspec",
        "define endian=little;\n\
         define space ram type=ram_space size=4 default;\n\
         define space register type=register_space size=4;\n\
         define register offset=0 size=4 [ r0 r1 ];\n\
         define token instr(8) op=(0,7);\n\
         :add r0, r1 is op=1 { local t:4 = r1;  # keep\n r0 = r0 + t; }\n",
    );
    let formatter = Formatter::with_rules(vec![Box::new(StatementLines::default())]);
    let result = formatter.format(&mut sources, root).expect("formats");
    let body = &result.files[0].content;
    assert!(
        body.ends_with(
            ":add r0, r1 is op=1\n{\n    local t:4 = r1;  # keep\n    r0 = r0 + t;\n}\n"
        ),
        "unexpected layout:\n{body}"
    );
}

#[test]
fn statement_lines_is_idempotent_and_keeps_laid_out_bodies() {
    use crate::rules::StatementLines;
    let prelude = "define endian=little;\n\
                   define space ram type=ram_space size=4 default;\n\
                   define space register type=register_space size=4;\n\
                   define register offset=0 size=4 [ r0 r1 ];\n\
                   define token instr(8) op=(0,7);\n";
    let laid_out = format!("{prelude}:add r0, r1 is op=1\n{{\n    r0 = r0 + r1;\n}}\n");
    let mut sources = SourceDb::new();
    let root = sources.add_file("spec.slaspec", laid_out.clone());
    let formatter = Formatter::with_rules(vec![Box::new(StatementLines::default())]);
    let once = formatter.format(&mut sources, root).expect("formats").files[0]
        .content
        .clone();
    assert_eq!(once, laid_out);
}

#[test]
fn pcode_spacing_normalizes_statements() {
    use crate::rules::PcodeSpacing;
    let cases = [
        ("local tmp =    AL -   imm8;", "local tmp = AL - imm8;"),
        ("subflags(   AL,imm8 );", "subflags(AL, imm8);"),
        (
            "XmmReg1[0,32]  = XmmReg1[0,32]  f+ m[ 0,32 ];",
            "XmmReg1[0,32] = XmmReg1[0,32] f+ m[0,32];",
        ),
        ("tmp:8 = sext( EAX );", "tmp:8 = sext(EAX);"),
        ("if(!cc)goto inst_next;", "if (!cc) goto inst_next;"),
        ("EAX = -1;", "EAX = -1;"),
        ("goto <done>;", "goto <done>;"),
        ("*[ram]:4 EAX = tmp;", "*[ram]:4 EAX = tmp;"),
        ("CF = CF==0;", "CF = CF == 0;"),
        ("tmp = a s>> b;", "tmp = a s>> b;"),
    ];
    let prelude = "define endian=little;\n\
                   define space ram type=ram_space size=4 default;\n\
                   define space register type=register_space size=4;\n\
                   define register offset=0 size=4 [ EAX AL CF cc tmp imm8 ];\n\
                   define token instr(8) op=(0,7);\n";
    for (written, expected) in cases {
        let mut sources = SourceDb::new();
        let root = sources.add_file(
            "spec.slaspec",
            format!("{prelude}:op is op=1 {{ {written} }}\n"),
        );
        let formatter = Formatter::with_rules(vec![Box::new(PcodeSpacing)]);
        let content = match formatter.format(&mut sources, root) {
            Ok(result) => result.files[0].content.clone(),
            Err(error) => panic!("{written:?} did not parse: {error}"),
        };
        assert!(
            content.contains(expected),
            "{written:?}\n  expected: {expected:?}\n  got: {:?}",
            content.lines().last().unwrap_or_default()
        );
    }
}
