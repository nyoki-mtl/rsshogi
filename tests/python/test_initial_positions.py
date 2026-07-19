from __future__ import annotations

import pytest

initial_positions = pytest.importorskip("rsshogi.initial_positions")

InitialPosition = initial_positions.InitialPosition
HANDICAP_TO_SFEN = initial_positions.HANDICAP_TO_SFEN
SFEN_TO_HANDICAP = initial_positions.SFEN_TO_HANDICAP
handicap_to_sfen = initial_positions.handicap_to_sfen
sfen_to_handicap = initial_positions.sfen_to_handicap


def test_initial_position_has_extended_handicaps() -> None:
    assert InitialPosition.HANDICAP_3_PIECES.value == "lnsgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
    assert InitialPosition.HANDICAP_5_PIECES.value == "2sgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
    assert InitialPosition.HANDICAP_LEFT_5_PIECES.value == "1nsgkgs2/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"


def test_handicap_mapping_roundtrip() -> None:
    sfen = handicap_to_sfen("三枚落ち")
    assert sfen == InitialPosition.HANDICAP_3_PIECES.value
    assert sfen_to_handicap(sfen) == "三枚落ち"

    assert HANDICAP_TO_SFEN["五枚落ち"] == InitialPosition.HANDICAP_5_PIECES.value
    assert SFEN_TO_HANDICAP[InitialPosition.HANDICAP_LEFT_5_PIECES.value] == "左五枚落ち"


def test_sfen_to_handicap_returns_none_for_unknown() -> None:
    assert sfen_to_handicap("9/9/9/9/9/9/9/9/9 b - 1") is None
