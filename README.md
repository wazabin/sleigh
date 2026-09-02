# wazabin-sleigh

[![CI](https://github.com/wazabin/sleigh/actions/workflows/ci.yml/badge.svg)](https://github.com/wazabin/sleigh/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/wazabin-sleigh.svg)](https://crates.io/crates/wazabin-sleigh)

Rust tools for working with [SLEIGH], Ghidra's processor-specification
language.

This workspace contains:

- `sleigh`: compiler and instruction decoder library, plus `sleigh-decode`.
- `wazabin-sleigh-fmt`: formatter for `.slaspec` and `.sinc` files.
- `wazabin-sleigh-precompile`: selected processor specifications embedded at build time.

Developed by [Thalium](https://blog.thalium.re/about/).

## Development

```sh
cargo test --workspace
```

## License

The Rust code in this repository is licensed under the [MIT License](LICENSE).
The vendored `precompile/open_sleigh` processor specifications remain licensed
under Apache-2.0; see that directory's `LICENCE` and `NOTICE` files.

[SLEIGH]: https://ghidra-sre.org/SleighConcepts.html
