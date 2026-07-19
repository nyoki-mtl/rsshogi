from __future__ import annotations

import pytest

rsshogi = pytest.importorskip("rsshogi")
numpy = pytest.importorskip("numpy")


def test_packed_sfen_dtypes() -> None:
    assert hasattr(rsshogi, "numpy")
    assert hasattr(rsshogi.numpy, "PackedSfen")
    assert hasattr(rsshogi.numpy, "PackedSfenValue")
    assert hasattr(rsshogi.numpy, "HuffmanCodedPos")
    assert hasattr(rsshogi.numpy, "HuffmanCodedPosAndEval")

    expected_packed = numpy.dtype([("sfen", numpy.uint8, 32)])
    expected_value = numpy.dtype(
        [
            ("sfen", numpy.uint8, 32),
            ("score", numpy.int16),
            ("move", numpy.uint16),
            ("game_ply", numpy.uint16),
            ("game_result", numpy.int8),
            ("padding", numpy.uint8),
        ]
    )
    expected_hcp = numpy.dtype([("hcp", numpy.uint8, 32)])
    expected_hcpe = numpy.dtype(
        [
            ("hcp", numpy.uint8, 32),
            ("eval", numpy.int16),
            ("bestMove16", numpy.uint16),
            ("gameResult", numpy.int8),
            ("dummy", numpy.uint8),
        ]
    )

    assert rsshogi.numpy.PackedSfen == expected_packed
    assert rsshogi.numpy.PackedSfenValue == expected_value
    assert rsshogi.numpy.HuffmanCodedPos == expected_hcp
    assert rsshogi.numpy.HuffmanCodedPosAndEval == expected_hcpe
    assert rsshogi.numpy.PackedSfen.itemsize == 32
    assert rsshogi.numpy.PackedSfenValue.itemsize == 40
    assert rsshogi.numpy.HuffmanCodedPos.itemsize == 32
    assert rsshogi.numpy.HuffmanCodedPosAndEval.itemsize == 38
