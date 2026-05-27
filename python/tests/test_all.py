import pytest
import htslurp


def test_module_importable():
    assert hasattr(htslurp, "stream_records")


def test_stream_records_bad_url_raises():
    with pytest.raises(Exception):
        list(htslurp.stream_records("http://localhost:1", "nonexistent", "CRAM"))
