from __future__ import annotations

from pathlib import Path

import pytest

rsshogi = pytest.importorskip("rsshogi")

HIRATE_SFEN = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"


def _wdl(win: int, draw: int, loss: int) -> object:
    return rsshogi.sazpack.SazWdl(win, draw, loss)


def _entry(
    move: str,
    *,
    prior: int,
    visits_before: int,
    visits_after: int,
    lower: int = 0,
    upper: int = 2,
) -> object:
    return rsshogi.sazpack.SazPolicyEntry(
        move,
        prior,
        visits_before,
        visits_after,
        lower,
        upper,
    )


def _fixture_game() -> object:
    """SAZ2の主要fieldを持つ平手2局面fixture。"""
    saz = rsshogi.sazpack
    return saz.SazGame(
        HIRATE_SFEN,
        rsshogi.record.GameResult.BLACK_WIN,
        7,  # MaxGamePlies
        0,  # EnteringKingRule::None
        [
            saz.SazPosition(
                "7g7f",
                _wdl(32_768, 16_383, 16_384),
                _wdl(65_535, 0, 0),
                2,
                1_000,
                750,
                1,
                [
                    _entry("7g7f", prior=40_000, visits_before=10, visits_after=810),
                    _entry("2g2f", prior=15_000, visits_before=5, visits_after=155),
                    _entry("6g6f", prior=10_535, visits_before=0, visits_after=50),
                ],
                mate=5,
            ),
            saz.SazPosition(
                "3c3d",
                _wdl(10_000, 20_000, 35_535),
                _wdl(0, 0, 65_535),
                1,
                1_000,
                1_000,
                0,
                [_entry("3c3d", prior=65_535, visits_before=0, visits_after=1_000)],
            ),
        ],
    )


def test_sazpack_roundtrip_preserves_fields() -> None:
    saz = rsshogi.sazpack
    data = saz.write_sazpack([_fixture_game()])
    assert data[:4] == b"SAZ2"

    games = saz.decode_sazpack(data)
    assert len(games) == 1
    decoded = games[0]

    assert decoded.game_result == rsshogi.record.GameResult.BLACK_WIN
    assert decoded.termination_reason == 7
    assert decoded.entering_king_rule == 0
    assert [position.played for position in decoded.positions] == ["7g7f", "3c3d"]

    first = decoded.positions[0]
    assert (first.root_wdl.win, first.root_wdl.draw, first.root_wdl.loss) == (
        32_768,
        16_383,
        16_384,
    )
    assert (first.outcome_wdl.win, first.outcome_wdl.draw, first.outcome_wdl.loss) == (
        65_535,
        0,
        0,
    )
    assert first.plies_left == 2
    assert first.requested_visits == 1_000
    assert first.target_weight_milli == 750
    assert first.exploration_flags == 1
    assert first.mate == 5


def test_sazpack_policy_preserves_prior_and_visit_snapshots() -> None:
    game = rsshogi.sazpack.decode_sazpack(rsshogi.sazpack.write_sazpack([_fixture_game()]))[0]
    entries = game.positions[0].policy

    assert [(entry.mv, entry.prior) for entry in entries] == [
        ("7g7f", 40_000),
        ("2g2f", 15_000),
        ("6g6f", 10_535),
    ]
    assert [(entry.visits_before, entry.visits_after) for entry in entries] == [
        (10, 810),
        (5, 155),
        (0, 50),
    ]
    assert [(entry.lower, entry.upper) for entry in entries] == [(0, 2), (0, 2), (0, 2)]


def test_sazpack_moves_map_to_compact_labels() -> None:
    game = rsshogi.sazpack.decode_sazpack(rsshogi.sazpack.write_sazpack([_fixture_game()]))[0]
    colors = [rsshogi.types.Color.BLACK, rsshogi.types.Color.WHITE]

    labels = [
        rsshogi.policy.compact_move_label(position.played, color)
        for position, color in zip(game.positions, colors, strict=True)
    ]
    assert all(label is not None for label in labels)
    assert all(0 <= label < rsshogi.policy.COMPACT_MOVE_LABEL_COUNT for label in labels)


def test_sazpack_file_io(tmp_path: Path) -> None:
    saz = rsshogi.sazpack
    path = tmp_path / "sample.saz"
    saz.write_sazpack_file(path, [_fixture_game()])
    games = saz.decode_sazpack_file(path)
    assert len(games) == 1
    assert [position.played for position in games[0].positions] == ["7g7f", "3c3d"]


def test_sazpack_optional_mate_roundtrip() -> None:
    game = rsshogi.sazpack.decode_sazpack(rsshogi.sazpack.write_sazpack([_fixture_game()]))[0]
    assert game.positions[0].mate == 5
    assert game.positions[1].mate is None


def test_sazpack_rejects_invalid_distribution_sum() -> None:
    saz = rsshogi.sazpack
    game = saz.SazGame(
        HIRATE_SFEN,
        rsshogi.record.GameResult.BLACK_WIN,
        7,
        0,
        [
            saz.SazPosition(
                "7g7f",
                _wdl(1, 2, 3),
                _wdl(65_535, 0, 0),
                1,
                1,
                1_000,
                0,
                [_entry("7g7f", prior=65_535, visits_before=0, visits_after=1)],
            )
        ],
    )
    with pytest.raises(ValueError, match="distribution"):
        saz.write_sazpack([game])


def test_sazpack_rejects_invalid_magic() -> None:
    saz = rsshogi.sazpack
    data = bytearray(saz.write_sazpack([_fixture_game()]))
    data[0] = ord("X")
    with pytest.raises(ValueError, match="sazpack"):
        saz.decode_sazpack(bytes(data))


def test_sazpack_rejects_decreasing_visits() -> None:
    saz = rsshogi.sazpack
    game = saz.SazGame(
        HIRATE_SFEN,
        rsshogi.record.GameResult.BLACK_WIN,
        7,
        0,
        [
            saz.SazPosition(
                "7g7f",
                _wdl(65_535, 0, 0),
                _wdl(65_535, 0, 0),
                1,
                1,
                1_000,
                0,
                [_entry("7g7f", prior=65_535, visits_before=2, visits_after=1)],
            )
        ],
    )
    with pytest.raises(ValueError, match="visits decreased"):
        saz.write_sazpack([game])
