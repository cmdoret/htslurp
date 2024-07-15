import pytest
import htslurp


def test_sum_as_string():
    assert htslurp.sum_as_string(1, 1) == "2"
