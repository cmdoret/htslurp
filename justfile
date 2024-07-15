set positional-arguments
set shell := ["bash", "-cue"]
root := justfile_directory()
src := "./src/modos.typ"
pdf := "./build/modos.pdf"


# build wheel
build *args:
  maturin build \
    --release \
    {{args}}

# development environment
develop *args:
  maturin develop \
  --uv \
  {{args}}

