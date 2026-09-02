# Fetch the open_sleigh processor-specification submodule.
setup:
    git submodule update --init --recursive

# Overlay selected processor specifications from a Ghidra installation/release.
setup-ghidra *args:
    python scripts/setup.py {{args}}

fmt:
    cargo fmt --all

lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

test:
    cargo test --workspace --all-features

# Set the release version, commit it, and create its v-tag.
release version:
    scripts/release.py "{{version}}"
