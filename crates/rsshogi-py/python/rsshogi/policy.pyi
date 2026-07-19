"""Type stubs for rsshogi.policy — policy label helpers."""

from __future__ import annotations

from rsshogi.core import Move, Move32
from rsshogi.types import Color

MOVE_LABEL_COUNT: int
COMPACT_MOVE_LABEL_COUNT: int
MOVE_LABEL_TO_COMPACT: tuple[int | None, ...]
COMPACT_TO_MOVE_LABEL: tuple[int, ...]

def move_label(move: Move | Move32 | int | str, turn: Color) -> int: ...
def compact_move_label(move: Move | Move32 | int | str, turn: Color) -> int | None: ...
def move_label_to_compact(label: int) -> int | None: ...
def compact_move_label_to_move_label(label: int) -> int: ...
def is_structurally_valid_move_label(label: int) -> bool: ...

__all__ = [
    "MOVE_LABEL_COUNT",
    "COMPACT_MOVE_LABEL_COUNT",
    "MOVE_LABEL_TO_COMPACT",
    "COMPACT_TO_MOVE_LABEL",
    "move_label",
    "compact_move_label",
    "move_label_to_compact",
    "compact_move_label_to_move_label",
    "is_structurally_valid_move_label",
]
