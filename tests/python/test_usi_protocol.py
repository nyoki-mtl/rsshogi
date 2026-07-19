import pytest
import rsshogi as rs


def test_parse_info_score_bound_and_pv() -> None:
    info = rs.usi.parse_info("info depth 12 seldepth 18 nodes 12345 score cp 37 lowerbound pv 7g7f 3c3d")

    assert info.depth == 12
    assert info.seldepth == 18
    assert info.nodes == 12345
    assert info.score == 37
    assert info.bound is rs.usi.UsiBound.LOWER
    assert [move.to_usi() for move in info.pv or ()] == ["7g7f", "3c3d"]


def test_parse_info_mate_score_and_string() -> None:
    mate_info = rs.usi.UsiInfo.parse("info score mate 3")
    assert mate_info.score is not None
    assert mate_info.score.is_mate_score()
    assert mate_info.score.mate_in_ply() == 3
    assert mate_info.score.to_string() == "mate 3"

    string_info = rs.usi.parse_info("info string ready ok")
    assert string_info.string == "ready ok"


def test_parse_bestmove_special_moves_and_ponder() -> None:
    best = rs.usi.parse_bestmove("bestmove resign")
    assert best.bestmove == rs.core.Move.MOVE_RESIGN
    assert best.ponder is None

    ponder = rs.usi.UsiBestMove.parse("bestmove 7g7f ponder 3c3d")
    assert ponder.bestmove.to_usi() == "7g7f"
    assert ponder.ponder is not None
    assert ponder.ponder.to_usi() == "3c3d"
    assert ponder.to_string() == "bestmove 7g7f ponder 3c3d"

    assert rs.usi.move_from_usi("0000") == rs.core.Move.MOVE_NULL


def test_go_command_formats_common_fields() -> None:
    command = rs.usi.UsiGoCommand(
        searchmoves=(rs.core.Move.from_usi("7g7f"),),
        btime=1000,
        wtime=2000,
        byoyomi=500,
        depth=10,
    )

    assert command.to_string() == "go searchmoves 7g7f btime 1000 wtime 2000 byoyomi 500 depth 10"


def test_parse_info_rejects_non_info_line() -> None:
    with pytest.raises(ValueError, match="info"):
        rs.usi.parse_info("bestmove 7g7f")
