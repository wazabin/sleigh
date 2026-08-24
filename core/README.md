# wazabin-sleigh

A Rust implementation of SLEIGH, the processor-specification language Ghidra
uses to describe instruction sets. Give it a `.slaspec` and some bytes; it
gives you back the instruction, its assembly text, and its p-code semantics.

```rust
use sleigh::{Compiler, Decoder, SourceDb};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // A whole (very small) processor.
    let spec_source = r#"
        define endian=little;
        define space ram      type=ram_space      size=4 default;
        define space register type=register_space size=4;
        define register offset=0 size=4 [ r0 r1 ];

        define token instr(8) op=(0,3) reg=(4,7);
        attach variables [ reg ] [ r0 r1 _ _ _ _ _ _ _ _ _ _ _ _ _ _ ];

        :inc reg is op=1 & reg { reg = reg + 1; }
    "#;

    // 1. Compile the specification. Do this once; it is the expensive part.
    let mut sources = SourceDb::new();
    let root = sources.add_file("tiny.slaspec", spec_source);
    let spec = Compiler::new(&mut sources).compile(root)?;

    // 2. Decode. The context carries decode state between instructions —
    //    start from the specification's default.
    let decoder = Decoder::new(&spec);
    let instruction = decoder.decode_one(0x1000, &[0x11], &spec.new_context())?;

    assert_eq!(instruction.display()?, "inc r1");
    assert_eq!(instruction.len(), 1);

    // 3. Ask what it *does*.
    assert_eq!(
        instruction.pcode_ast()?.pretty_print(&spec),
        "r1 = (r1 + 1);"
    );

    Ok(())
}
```

## What you get

- **Decoding** — [`Decoder::decode_one`] matches one instruction against the
  compiled specification and reports a typed error rather than panicking when
  it cannot.
- **Disassembly** — [`Instruction::display`] renders the specification's
  display section.
- **Semantics** — [`Instruction::pcode_ast`] gives a source-shaped p-code AST
  with macros, sub-tables and delay slots already expanded. See the
  [`semantics`] module for the type graph and the conventions it follows.
- **Source tooling** — [`analyze`] reports diagnostics and lints without
  building a runtime specification. `sleigh-fmt` formats specification text.

## Scope

The parser and decoder are complete enough to compile 142 of the 149
specifications in the vendored Ghidra corpus; the remaining seven are defects
in those files. Big-endian tokens, context flow and `globalset`, delay slots,
and variable-length register lists all work.

## `sleigh-decode`

A small command-line decoder ships with the crate:

```text
$ cargo run -p wazabin-sleigh --bin sleigh-decode -- ./src/tests/fixtures/example.sla 108c
[0x0000] and r3,r4
```
