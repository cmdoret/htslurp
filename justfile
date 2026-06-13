set positional-arguments
set shell := ["bash", "-cue"]

root_dir := `git rev-parse --show-toplevel`

# Default recipe to list all recipes.
[private]
default:
    just --list

# Build the release wheel.
build *args:
    maturin build --release {{args}}

# Build and install the extension into the active environment for development.
develop *args:
    maturin develop --uv {{args}}

# Format Rust and Python sources.
format *args:
    cargo fmt {{args}}
    ruff format python {{args}}

# Lint Rust (clippy) and Python (ruff) sources.
lint *args:
    cargo clippy --all-targets {{args}}
    ruff check python {{args}}

# Run the full test suite (Rust and Python).
test: test-rust test-python

# Run the Rust test suite (the Docker integration test stays #[ignore]d).
[private]
test-rust *args:
    cargo test {{args}}

# Run the Python test suite (run `just develop` first to install the extension).
[private]
test-python *args:
    pytest {{args}}
