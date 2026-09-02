//! Prints the `#@family` breakdown of a precompiled specification.
//!
//! ```text
//! cargo run -p wazabin-sleigh-precompile --example families
//! ```

use std::collections::BTreeMap;

fn main() {
    for (name, families) in [
        ("x64", sleigh_precompile::x64::families()),
        ("x86", sleigh_precompile::x86::families()),
        ("riscv", sleigh_precompile::riscv::families()),
        ("aarch64", sleigh_precompile::aarch64::families()),
    ] {
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for (_, _, family) in families.iter() {
            *counts.entry(family).or_default() += 1;
        }

        println!("{name}: {} annotated constructors", families.len());
        for (family, count) in &counts {
            println!("  {family:<12} {count}");
        }
    }
}
