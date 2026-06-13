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

# Format Rust sources.
format *args:
    cargo fmt {{args}}

# Lint Rust sources with clippy.
lint *args:
    cargo clippy --all-targets {{args}}

# Run the Rust test suite (the Docker integration test stays #[ignore]d).
test *args:
    cargo test {{args}}

# Run the Python test suite (run `just develop` first to install the extension).
test-py *args:
    pytest {{args}}
