# wazabin-sleigh-fmt

[![CI](https://github.com/wazabin/sleigh/actions/workflows/ci.yml/badge.svg)](https://github.com/wazabin/sleigh/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/wazabin-sleigh-fmt.svg)](https://crates.io/crates/wazabin-sleigh-fmt)

A formatter for SLEIGH processor specifications — `rustfmt` for `.slaspec`
and `.sinc` files.

It edits the original bytes rather than reprinting from a parse tree, so
anything no rule has an opinion about is left exactly as written.

```text
$ cargo run -p wazabin-sleigh-fmt --bin sleigh-fmt -- path/to/spec.slaspec
```

A specification is usually several files stitched together by `@include`;
formatting the root formats every physical file it reaches, and each is
written back to its own path. Text produced by a preprocessor macro expansion
is never edited, because the edit would land in the wrong file.

If the source does not parse, nothing is formatted — a formatter that guesses
at broken syntax destroys work.

Developed by [Thalium](https://blog.thalium.re/about/).

## License

Licensed under the [MIT License](LICENSE).
