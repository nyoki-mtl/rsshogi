"""Initial position SFEN definitions."""

from __future__ import annotations

from enum import Enum
from types import MappingProxyType


class InitialPosition(Enum):
    STANDARD = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"
    EMPTY = "9/9/9/9/9/9/9/9/9 b - 1"
    HANDICAP_LANCE = "lnsgkgsn1/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
    HANDICAP_RIGHT_LANCE = "1nsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
    HANDICAP_BISHOP = "lnsgkgsnl/1r7/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
    HANDICAP_ROOK = "lnsgkgsnl/7b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
    HANDICAP_ROOK_LANCE = "lnsgkgsn1/7b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
    HANDICAP_2_PIECES = "lnsgkgsnl/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
    HANDICAP_3_PIECES = "lnsgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
    HANDICAP_4_PIECES = "1nsgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
    HANDICAP_5_PIECES = "2sgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
    HANDICAP_LEFT_5_PIECES = "1nsgkgs2/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
    HANDICAP_6_PIECES = "2sgkgs2/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
    HANDICAP_8_PIECES = "3gkg3/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
    HANDICAP_10_PIECES = "4k4/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"


HANDICAP_TO_SFEN = MappingProxyType(
    {
        "平手": InitialPosition.STANDARD.value,
        "香落ち": InitialPosition.HANDICAP_LANCE.value,
        "右香落ち": InitialPosition.HANDICAP_RIGHT_LANCE.value,
        "角落ち": InitialPosition.HANDICAP_BISHOP.value,
        "飛車落ち": InitialPosition.HANDICAP_ROOK.value,
        "飛香落ち": InitialPosition.HANDICAP_ROOK_LANCE.value,
        "二枚落ち": InitialPosition.HANDICAP_2_PIECES.value,
        "三枚落ち": InitialPosition.HANDICAP_3_PIECES.value,
        "四枚落ち": InitialPosition.HANDICAP_4_PIECES.value,
        "五枚落ち": InitialPosition.HANDICAP_5_PIECES.value,
        "左五枚落ち": InitialPosition.HANDICAP_LEFT_5_PIECES.value,
        "六枚落ち": InitialPosition.HANDICAP_6_PIECES.value,
        "八枚落ち": InitialPosition.HANDICAP_8_PIECES.value,
        "十枚落ち": InitialPosition.HANDICAP_10_PIECES.value,
    }
)

SFEN_TO_HANDICAP = MappingProxyType({sfen: handicap for handicap, sfen in HANDICAP_TO_SFEN.items()})


def handicap_to_sfen(handicap: str) -> str:
    return HANDICAP_TO_SFEN[handicap]


def sfen_to_handicap(sfen: str) -> str | None:
    return SFEN_TO_HANDICAP.get(sfen)


__all__ = [
    "InitialPosition",
    "HANDICAP_TO_SFEN",
    "SFEN_TO_HANDICAP",
    "handicap_to_sfen",
    "sfen_to_handicap",
]
