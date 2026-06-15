# Quickstart

## Install

```sh
pip install htslurp
```

## Stream records

[`stream_records`](api.md#htslurp.stream_records) returns a lazy
[`RecordIter`](api.md#htslurp.RecordIter). Records are decoded on demand by a
background worker thread, so memory stays bounded no matter how much the server
returns.

```python
import htslurp

records = htslurp.stream_records(
    "https://htsget.ga4gh.org/reads",  # htsget endpoint
    "giab.NA12878",                    # resource id on the server
    "CRAM",                            # "BAM" or "CRAM"
    region="11:4900000-5000000",       # optional; non-overlapping records are dropped
)

# The SAM header is exposed separately, as bytes.
header_text = records.header.decode()

# Iterate to get one SAM-format line (bytes) per record.
for line in records:
    fields = line.decode().split("\t")
    rname, pos = fields[2], fields[3]
    print(rname, pos)
```

## Arguments

- **`format`** — `"BAM"` or `"CRAM"`.
- **`region`** — optional `"name:start-end"` (e.g. `"11:4900000-5000000"`). htsget
  responses are coarser than the request, so htslurp re-checks each record and drops
  those that don't overlap. Omit it to stream the whole resource.
- **`reference`** — optional path to an indexed FASTA, required only for CRAMs that
  use external reference-based compression.

A bad request (unreachable server, unknown id, invalid region) raises
`RuntimeError`.

## Handing off to pysam

Each yielded line is one SAM record with no trailing newline, so it can go straight
to `pysam.AlignedSegment.fromstring`:

```python
import pysam

header = pysam.AlignmentHeader.from_text(records.header.decode())
for line in records:
    segment = pysam.AlignedSegment.fromstring(line.decode(), header)
    ...
```

See the [API reference](api.md) for the full contract.
