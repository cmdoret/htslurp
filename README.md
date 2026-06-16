<p align="center">
  <img src="./docs/assets/logo.svg" alt="HTSlurp logo" width="250">
  </br>
  <a href="https://github.com/cmdoret/htslurp/actions/workflows/CI.yml">
    <img alt="CI badge" src="https://img.shields.io/github/actions/workflow/status/cmdoret/htslurp/CI.yml?branch=main&style=for-the-badge&logo=githubactions&logoColor=white&label=CI">
  </a>
  <a href="https://cmdoret.github.io/htslurp">
    <img alt="Docs badge" src="https://img.shields.io/github/actions/workflow/status/cmdoret/htslurp/docs.yml?branch=main&style=for-the-badge&logo=materialformkdocs&logoColor=white&label=docs">
  </a>
  <a href="https://pypi.org/project/htslurp">
    <img alt="PyPI badge" src="https://img.shields.io/pypi/v/htslurp?style=for-the-badge&logo=pypi&logoColor=white">
  </a>
</p>

<!-- --8<-- [start:docs] -->

# htslurp

A noodles-based [htsget](https://samtools.github.io/hts-specs/htsget.html) client
that lazily deserializes alignment records in memory. It provides a Rust API with
Python bindings, so you can consume remote CRAM/BAM records over the network
without storing them locally.

Documentation: <https://cmdoret.github.io/htslurp/>

## Install

```sh
pip install htslurp
```

## Usage

```python
import htslurp

records = htslurp.stream_records(
    "https://htsget.ga4gh.org/reads",
    "giab.NA12878",
    "CRAM",
    region="11:4900000-5000000",
)

header_text = records.header.decode()
for line in records:
    fields = line.decode().split("\t")
    # ... or feed `line.decode()` and a pysam.AlignmentHeader built from
    # `header_text` to pysam.AlignedSegment.fromstring.
```

See the [quickstart](https://cmdoret.github.io/htslurp/quickstart/) for more.

## Context

The aim is to provide a convenient interface to consume remote CRAM/BCF records
over the network without storing them locally. The noodles crate fetches a binary
stream from the server and builds a reader over it that lazily instantiates records.

```mermaid
flowchart LR
    htsget[htsget-server] -->|bytes| Reader
    Reader -->|Cram records| python
```

## Development

This project requires:
* a working rust (>=1.76) and python (>=3.10) installation
* [maturin](https://github.com/PyO3/maturin)
* [uv](https://github.com/astral-sh/uv)
* [just](https://github.com/casey/just)

To build the python package:

```sh
just build
```

To install in editable mode for development:

```sh
just develop
```

To preview the docs locally:

```sh
just docs
```

<!-- --8<-- [end:docs] -->

