from __future__ import annotations

from collections.abc import Mapping
from enum import Enum

class InitialPosition(Enum):
    STANDARD: str
    EMPTY: str
    HANDICAP_LANCE: str
    HANDICAP_RIGHT_LANCE: str
    HANDICAP_BISHOP: str
    HANDICAP_ROOK: str
    HANDICAP_ROOK_LANCE: str
    HANDICAP_2_PIECES: str
    HANDICAP_3_PIECES: str
    HANDICAP_4_PIECES: str
    HANDICAP_5_PIECES: str
    HANDICAP_LEFT_5_PIECES: str
    HANDICAP_6_PIECES: str
    HANDICAP_8_PIECES: str
    HANDICAP_10_PIECES: str

HANDICAP_TO_SFEN: Mapping[str, str]
SFEN_TO_HANDICAP: Mapping[str, str]

def handicap_to_sfen(handicap: str) -> str: ...
def sfen_to_handicap(sfen: str) -> str | None: ...

__all__: list[str]
