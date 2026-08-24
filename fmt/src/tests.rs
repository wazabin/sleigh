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
