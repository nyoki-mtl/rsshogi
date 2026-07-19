from __future__ import annotations

import pytest
from rsshogi.core import Board, Move
from rsshogi.policy import (
    COMPACT_MOVE_LABEL_COUNT,
    COMPACT_TO_MOVE_LABEL,
    MOVE_LABEL_COUNT,
    MOVE_LABEL_TO_COMPACT,
    compact_move_label,
    compact_move_label_to_move_label,
    is_structurally_valid_move_label,
    move_label,
    move_label_to_compact,
)
from rsshogi.types import Color


def test_policy_tables_have_expected_sizes() -> None:
    assert MOVE_LABEL_COUNT == 2187
    assert COMPACT_MOVE_LABEL_COUNT == 1496
    assert len(MOVE_LABEL_TO_COMPACT) == MOVE_LABEL_COUNT
    assert len(COMPACT_TO_MOVE_LABEL) == COMPACT_MOVE_LABEL_COUNT
    assert sum(value is None for value in MOVE_LABEL_TO_COMPACT) == 691
    assert sum(value is not None for value in MOVE_LABEL_TO_COMPACT) == COMPACT_MOVE_LABEL_COUNT


def test_move_label_accepts_move_move32_and_usi() -> None:
    board = Board()
    mv = Move.from_usi("7g7f")
    mv32 = board.move32_from_move(mv)

    raw = move_label(mv, Color.BLACK)
    assert move_label(mv32, Color.BLACK) == raw
    assert move_label("7g7f", Color.BLACK) == raw

    compact = compact_move_label(mv, Color.BLACK)
    assert compact is not None
    assert compact_move_label(mv32, Color.BLACK) == compact
    assert compact_move_label("7g7f", Color.BLACK) == compact


def test_white_moves_are_rotated_to_black_perspective() -> None:
    assert move_label("7g7f", Color.BLACK) == move_label("3c3d", Color.WHITE)


def test_compact_label_roundtrip_works() -> None:
    raw = move_label("7g7f", Color.BLACK)
    compact = compact_move_label("7g7f", Color.BLACK)

    assert compact is not None
    assert move_label_to_compact(raw) == compact
    assert compact_move_label_to_move_label(compact) == raw
    assert COMPACT_TO_MOVE_LABEL[compact] == raw
    assert MOVE_LABEL_TO_COMPACT[raw] == compact
    assert is_structurally_valid_move_label(raw)


def test_structurally_invalid_labels_return_none_for_compact() -> None:
    pawn_drop_last_rank = move_label("P*5a", Color.BLACK)
    knight_non_promotion = move_label("5c4a", Color.BLACK)

    assert not is_structurally_valid_move_label(pawn_drop_last_rank)
    assert compact_move_label("P*5a", Color.BLACK) is None
    assert move_label_to_compact(pawn_drop_last_rank) is None

    assert not is_structurally_valid_move_label(knight_non_promotion)
    assert compact_move_label("5c4a", Color.BLACK) is None
    assert move_label_to_compact(knight_non_promotion) is None


def test_legal_moves_always_map_to_compact_labels() -> None:
    board = Board()

    for mv in board.legal_moves():
        raw = move_label(mv, board.turn)
        compact = compact_move_label(mv, board.turn)

        assert is_structurally_valid_move_label(raw)
        assert compact is not None
        assert compact_move_label_to_move_label(compact) == raw


def test_invalid_inputs_raise_value_error() -> None:
    with pytest.raises(ValueError):
        move_label(Move.MOVE_NULL, Color.BLACK)

    with pytest.raises(ValueError):
        move_label_to_compact(MOVE_LABEL_COUNT)

    with pytest.raises(ValueError):
        compact_move_label_to_move_label(COMPACT_MOVE_LABEL_COUNT)

    with pytest.raises(ValueError):
        is_structurally_valid_move_label(MOVE_LABEL_COUNT)
