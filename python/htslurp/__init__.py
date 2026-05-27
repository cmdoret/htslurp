"""Stream alignment records from htsget servers.

Example:
    >>> import htslurp
    >>> records = htslurp.stream_records(
    ...     "https://htsget.ga4gh.org/reads",
    ...     "giab.NA12878",
    ...     "CRAM",
    ...     region="11:4900000-5000000",
    ... )
    >>> header_text = records.header.decode()
    >>> for line in records:
    ...     fields = line.decode().split("\\t")
    ...     # ... or hand `line.decode()` and a pysam.AlignmentHeader
    ...     # built from `header_text` to pysam.AlignedSegment.fromstring.
"""

from .htslurp import htsget_client

stream_records = htsget_client.stream_records
RecordIter = htsget_client.RecordIter

__all__ = ["stream_records", "RecordIter"]
