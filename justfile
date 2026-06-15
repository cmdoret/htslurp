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

# Run the full test suite (Rust, Python and docker).
test: (test-rust "--" "--include-ignored") (test-python "-m" "integration or not integration")

# Run the Rust test suite
[private]
test-rust *args:
    cargo test "$@"

# Run the Python test suite
[private]
test-python *args: develop
    pytest "$@"
