"""End-to-end streaming through the Python API against a real htsget-rs server.

Marked ``integration`` so it is excluded from the default unit run and only
exercised when explicitly enabled (``just test`` / ``pytest -m integration``).
Needs Docker. Unlike the Rust integration test (which drives ``start_stream``
directly), this exercises the public binding: ``stream_records`` ->
``RecordIter`` iteration -> ``.header`` -> record ``bytes``.
"""

import os
import socket
import time
from pathlib import Path

import pytest

import htslurp

# The fixture stops the container explicitly, so the Ryuk cleanup sidecar is
# redundant; disabling it avoids a failure mode where Ryuk's networking is
# unavailable.
os.environ.setdefault("TESTCONTAINERS_RYUK_DISABLED", "true")

pytestmark = pytest.mark.integration

DATA_DIR = Path(__file__).resolve().parents[2] / "data"
REGION = "11:4900000-5000000"


def _wait_for_port(host: str, port: int, timeout: float = 30.0) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
            sock.settimeout(1.0)
            if sock.connect_ex((host, port)) == 0:
                return
        time.sleep(0.5)
    raise TimeoutError(f"{host}:{port} not reachable within {timeout}s")


@pytest.fixture(scope="module")
def htsget_url():
    from testcontainers.core.container import DockerContainer

    # htsget-rs serves tickets on 8080 and data blocks on 8081, with the repo's
    # ./data mounted at /data. The data-block URL the server advertises is its
    # HTSGET_DATA_SERVER_ADDR, so it must be a host-reachable address (the
    # image's 0.0.0.0 default is rejected by the server's host check). Host
    # networking lets us bind and advertise 127.0.0.1 directly.
    container = (
        DockerContainer("ghcr.io/umccr/htsget-rs:dev-94")
        .with_volume_mapping(str(DATA_DIR), "/data", "ro")
        .with_env("HTSGET_TICKET_SERVER_ADDR", "127.0.0.1:8080")
        .with_env("HTSGET_DATA_SERVER_ADDR", "127.0.0.1:8081")
        .with_kwargs(network_mode="host")
    )
    container.start()
    try:
        _wait_for_port("127.0.0.1", 8080)
        yield "http://127.0.0.1:8080/reads"
    finally:
        container.stop()


def test_streams_cram_records_through_the_python_api(htsget_url):
    records = htslurp.stream_records(
        htsget_url,
        "data/cram/htsnexus_test_NA12878",
        "CRAM",
        region=REGION,
    )

    header = records.header.decode()
    assert header.startswith("@HD") or header.startswith("@SQ")

    rows = [line.decode() for line in records]
    assert rows, "stream should yield records"
    for row in rows:
        fields = row.split("\t")
        assert len(fields) >= 11, f"expected 11+ SAM fields, got {row!r}"
        assert fields[2] == "11", f"RNAME should be 11, got {fields[2]}"
        assert int(fields[3]) < 5_000_000
