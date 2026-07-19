import math

import pytest
import rsshogi as rs


def test_game_result_hashable_for_dict_and_set() -> None:
    result = rs.record.GameResult.BLACK_WIN

    by_result = {result: "sente"}
    assert by_result[rs.record.GameResult(0)] == "sente"

    result_set = {result}
    assert rs.record.GameResult.BLACK_WIN in result_set
    assert rs.record.GameResult(0) in result_set
    assert hash(result) == hash(0)


def test_game_result_from_str() -> None:
    assert rs.record.GameResult.from_str("BLACK_WIN") == rs.record.GameResult.BLACK_WIN
    assert rs.record.GameResult.from_str("black_win") == rs.record.GameResult.BLACK_WIN

    with pytest.raises(ValueError):
        rs.record.GameResult.from_str("NOT_A_RESULT")


def test_game_result_members() -> None:
    members = rs.record.GameResult.__members__
    assert isinstance(members, dict)
    assert members["BLACK_WIN"] == rs.record.GameResult.BLACK_WIN
    assert members["WHITE_WIN"] == rs.record.GameResult.WHITE_WIN
    assert members["DRAW_BY_REPETITION"] == rs.record.GameResult.DRAW_BY_REPETITION


def test_record_build_and_update_api() -> None:
    move1 = rs.record.MoveEntry(
        rs.core.Move32.from_usi("7g7f"),
        time_ms=1000,
        engine_info=rs.record.EngineInfo(
            eval=120,
            depth=18,
            nodes=145000,
            seldepth=27,
            extras={"arena.wall_time_ms": 1300, "arena.latency_delta_ms": 25},
        ),
    )
    move2 = rs.record.MoveEntry(
        "3c3d",
        engine_info=rs.record.EngineInfo(eval=-80, depth=16, nodes=98000, seldepth=22),
    )
    terminal = rs.record.SpecialMoveEntry("RESIGN", rs.record.GameResult.WHITE_WIN, time_ms=5000)
    metadata = rs.record.RecordMetadata(
        black_player="sente",
        white_player="gote",
        attributes={"game_name": "arena-1"},
    )

    record = rs.record.Record(
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        moves=[move1],
        metadata=metadata,
    )
    created = record.extend_main_line([move2])
    assert len(created) == 1

    record.set_main_terminal(terminal)
    assert record.main_terminal is not None
    assert record.main_terminal.kind == "RESIGN"
    assert record.main_terminal.time_ms == 5000
    assert record.moves[0].engine_info is not None
    assert record.moves[0].engine_info.extras["arena.wall_time_ms"] == 1300
    assert record.moves[0].engine_info.extras["arena.latency_delta_ms"] == 25


def test_special_move_record_from_result_uses_standard_terminal_kind() -> None:
    assert rs.record.SpecialMoveEntry.from_result(rs.record.GameResult.BLACK_WIN_BY_TIMEOUT).kind == "TIMEOUT"
    assert rs.record.SpecialMoveEntry.from_result(rs.record.GameResult.ERROR).kind == "INTERRUPT"
    assert rs.record.SpecialMoveEntry.from_result(rs.record.GameResult.INVALID).kind == "INTERRUPT"
    assert (
        rs.record.SpecialMoveEntry.from_result(rs.record.GameResult.BLACK_WIN_BY_ILLEGAL_MOVE).kind
        == "WIN_BY_ILLEGAL_MOVE"
    )


def test_game_record_from_usi_main_line_builds_moves_and_terminal() -> None:
    metadata = rs.record.RecordMetadata(game_name="arena-helper")
    record = rs.record.Record.from_usi_main_line(
        "startpos",
        ["7g7f", "3c3d"],
        result=rs.record.GameResult.WHITE_WIN,
        move_times_ms=[100, None],
        evals=[12, -8],
        nodes=[1000, 2000],
        depths=[8, 9],
        wall_times_ms=[110, None],
        latency_deltas_ms=[10, None],
        metadata=metadata,
        initial_comment="initial",
    )

    assert record.init_position_sfen == rs.core.Board().to_sfen()
    assert [move.move.to_usi() for move in record.moves] == ["7g7f", "3c3d"]
    assert record.moves[0].time_ms == 100
    assert record.moves[0].engine_info is not None
    assert record.moves[0].engine_info.eval == 12
    assert record.moves[0].engine_info.nodes == 1000
    assert record.moves[0].engine_info.depth == 8
    assert record.moves[0].engine_info.wall_time_ms == 110
    assert record.moves[0].engine_info.latency_delta_ms == 10
    assert record.moves[0].engine_info.extras["wall_time_ms"] == 110
    assert record.moves[0].engine_info.extras["latency_delta_ms"] == 10
    assert record.main_terminal is not None
    assert record.main_terminal.kind == "RESIGN"
    assert record.result == rs.record.GameResult.WHITE_WIN
    assert record.metadata.game_name == "arena-helper"
    assert record.initial_comment == "initial"


def test_game_record_initial_comment_property_roundtrip() -> None:
    record = rs.record.Record(
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        initial_comment="序文",
    )

    assert record.initial_comment == "序文"
    record.initial_comment = "差し替え"
    assert record.initial_comment == "差し替え"
    record.initial_comment = None
    assert record.initial_comment is None


def test_move_engine_info_extras_dict_rw() -> None:
    info = rs.record.EngineInfo(eval=42, depth=12, nodes=1000, seldepth=18)
    info.extras = {"nps": 250000, "temperature": 0.65, "ponder": True, "pv": "7g7f 3c3d"}
    assert info.extras["nps"] == 250000
    assert info.extras["temperature"] == pytest.approx(0.65)
    assert info.extras["ponder"] is True
    assert info.extras["pv"] == "7g7f 3c3d"

    info.set_extra("book_hits", 3)
    assert info.extras["book_hits"] == 3
    removed = info.remove_extra("pv")
    assert removed == "7g7f 3c3d"
    assert "pv" not in info.extras


def test_move_engine_info_extras_rejects_non_finite_float() -> None:
    info = rs.record.EngineInfo()
    with pytest.raises(ValueError):
        info.set_extra("bad", math.nan)


def test_move_record_constructor_rejects_extra_positional_args() -> None:
    with pytest.raises(TypeError):
        rs.record.MoveEntry("7g7f", 120, "opening")


def test_metadata_attributes_dict_rw() -> None:
    metadata = rs.record.RecordMetadata(attributes={"game_name": "A"})
    assert metadata.attributes["game_name"] == "A"

    metadata.attributes = {"game_type": "blitz", "black_rate": "2100"}
    assert metadata.attributes["game_type"] == "blitz"
    assert metadata.attributes["black_rate"] == "2100"
    assert "game_name" not in metadata.attributes

    metadata.set_attribute("updated_date", "2026-02-11")
    assert metadata.attributes["updated_date"] == "2026-02-11"
    removed = metadata.remove_attribute("game_type")
    assert removed == "blitz"
    assert "game_type" not in metadata.attributes


def test_game_record_to_dict_from_dict_roundtrip() -> None:
    move = rs.record.MoveEntry(
        "7g7f",
        time_ms=900,
        comment="opening",
        engine_info=rs.record.EngineInfo(
            eval=60,
            depth=17,
            nodes=120000,
            seldepth=25,
            extras={
                "pv": "7g7f 3c3d",
                "arena.wall_time_ms": 1200,
                "arena.latency_delta_ms": 30,
                "temperature": 0.55,
                "book_hit": False,
            },
        ),
    )
    metadata = rs.record.RecordMetadata(
        game_name="arena-roundtrip",
        game_type="ladder",
        updated_date="2026-02-13T00:00:00+00:00",
    )
    record = rs.record.Record.from_main_line(
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        [move],
        rs.record.SpecialMoveEntry("TIMEOUT", rs.record.GameResult.BLACK_WIN_BY_TIMEOUT),
        metadata,
        "序文1\n序文2",
    )

    payload = record.to_dict()
    assert payload["initial_comment"] == "序文1\n序文2"
    assert payload["moves"][0]["engine_info"]["extras"]["arena.wall_time_ms"] == 1200
    assert payload["moves"][0]["engine_info"]["extras"]["pv"] == "7g7f 3c3d"
    assert payload["moves"][0]["engine_info"]["extras"]["temperature"] == pytest.approx(0.55)
    assert payload["moves"][0]["engine_info"]["extras"]["book_hit"] is False
    assert payload["metadata"]["game_name"] == "arena-roundtrip"
    assert payload["metadata"]["game_type"] == "ladder"
    assert payload["metadata"]["updated_date"] == "2026-02-13T00:00:00+00:00"

    rebuilt = rs.record.Record.from_dict(payload)
    assert rebuilt.move_count == 1
    assert rebuilt.moves[0].move.to_usi() == "7g7f"
    assert rebuilt.moves[0].engine_info is not None
    assert rebuilt.moves[0].engine_info.extras["arena.wall_time_ms"] == 1200
    assert rebuilt.moves[0].engine_info.extras["pv"] == "7g7f 3c3d"
    assert rebuilt.moves[0].engine_info.extras["temperature"] == pytest.approx(0.55)
    assert rebuilt.moves[0].engine_info.extras["book_hit"] is False
    assert rebuilt.main_terminal is not None
    assert rebuilt.main_terminal.kind == "TIMEOUT"
    assert rebuilt.initial_comment == "序文1\n序文2"
    assert rebuilt.metadata.game_name == "arena-roundtrip"
    assert rebuilt.metadata.game_type == "ladder"
    assert rebuilt.metadata.updated_date == "2026-02-13T00:00:00+00:00"


def test_record_from_jkf_str_roundtrip() -> None:
    record = rs.record.Record.from_main_line(
        rs.core.Board().to_sfen(),
        [
            rs.record.MoveEntry("7g7f", comment="first"),
            rs.record.MoveEntry("3c3d"),
        ],
    )
    record.initial_comment = "root"
    jkf = record.to_jkf()

    parsed = rs.record.Record.from_jkf_str(jkf)

    assert parsed.initial_comment == "root"
    assert [move.move.to_usi() for move in parsed.moves] == ["7g7f", "3c3d"]
    assert parsed.moves[0].comment == "first"


def test_record_from_jkf_file(tmp_path) -> None:
    record = rs.record.Record.from_usi_position("position startpos moves 7g7f")
    path = tmp_path / "game.jkf"
    record.write_jkf(path)

    parsed = rs.record.Record.from_jkf_file(path)

    assert [move.move.to_usi() for move in parsed.moves] == ["7g7f"]


def test_record_usi_position_strict_and_extended_special_tokens() -> None:
    record = rs.record.Record.from_usi_position("position startpos moves 7g7f 3c3d")
    assert record.to_usi_position() == "position startpos moves 7g7f 3c3d"

    with pytest.raises(ValueError):
        rs.record.Record.from_usi_position("position startpos moves resign")

    extended = rs.record.Record.from_usi_position(
        "position startpos moves 7g7f resign",
        allow_special_tokens=True,
    )
    assert extended.result == rs.record.GameResult.BLACK_WIN
    assert extended.to_usi_position(include_special_tokens=True) == "position startpos moves 7g7f resign"


def test_record_editor_append_navigation_branch_and_into_record() -> None:
    record = rs.record.Record.from_usi_position("position startpos moves 7g7f")
    editor = record.into_editor()
    root = record.root_node_id()

    assert editor.current_node == root
    assert editor.go_forward()
    assert editor.current_node == record.main_line_ids()[0]
    entry = record.node_entry(editor.current_node)
    assert entry is not None
    assert entry.kind == "move"
    assert entry.move is not None
    assert entry.move.move.to_usi() == "7g7f"
    assert editor.go_back()
    assert editor.current_node == root

    variation = editor.append_move(rs.record.MoveEntry("2g2f"))
    assert variation != record.main_line_ids()[0]
    assert editor.go_back()
    assert editor.branch_to(1)

    rebuilt = editor.into_record()
    assert [move.move.to_usi() for move in rebuilt.moves] == ["2g2f"]
    with pytest.raises(ValueError):
        _ = editor.current_node


def test_record_metadata_key_constants() -> None:
    assert rs.record.RecordMetadataKey.BLACK_PLAYER == "black_player"
    assert rs.record.RecordMetadataKey.COMMENT == "comment"


def test_game_record_to_psv_exports_all_main_line_entries() -> None:
    board = rs.core.Board()
    init_sfen = board.to_sfen()

    move1 = rs.record.MoveEntry("7g7f", engine_info=rs.record.EngineInfo(eval=120))
    move2 = rs.record.MoveEntry("3c3d", engine_info=rs.record.EngineInfo(eval=-80))
    terminal = rs.record.SpecialMoveEntry("RESIGN", rs.record.GameResult.WHITE_WIN)
    record = rs.record.Record.from_main_line(init_sfen, [move1, move2], terminal)

    psv_entries = record.to_psv()
    assert isinstance(psv_entries, list)
    assert len(psv_entries) == 2
    assert all(isinstance(entry, (bytes, bytearray)) for entry in psv_entries)
    assert all(len(entry) == 40 for entry in psv_entries)

    expected0 = board.to_psv(
        mv=rs.core.Move.from_usi("7g7f"),
        score=120,
        game_result=rs.record.GameResult.WHITE_WIN,
        game_ply=1,
    )
    assert psv_entries[0] == expected0

    board.apply_usi("7g7f")
    expected1 = board.to_psv(
        mv=rs.core.Move.from_usi("3c3d"),
        score=-80,
        game_result=rs.record.GameResult.WHITE_WIN,
        game_ply=2,
    )
    assert psv_entries[1] == expected1


def test_game_record_to_psv_include_flags_on_main_line_record() -> None:
    init_sfen = rs.core.Board().to_sfen()
    move1 = rs.record.MoveEntry("7g7f", engine_info=rs.record.EngineInfo(eval=120))
    move2 = rs.record.MoveEntry("3c3d", engine_info=rs.record.EngineInfo(eval=-80))
    record = rs.record.Record.from_main_line(init_sfen, [move1, move2], None)

    default_entries = record.to_psv()
    assert len(default_entries) == 2

    main_only = record.to_psv(include_main=True, include_variations=False)
    assert main_only == default_entries

    variation_only = record.to_psv(include_main=False, include_variations=True)
    assert variation_only == []

    none_selected = record.to_psv(include_main=False, include_variations=False)
    assert none_selected == []


def test_game_record_to_psv_rejects_missing_eval() -> None:
    init_sfen = rs.core.Board().to_sfen()
    move1 = rs.record.MoveEntry("7g7f", engine_info=rs.record.EngineInfo(eval=10))
    move2 = rs.record.MoveEntry("3c3d")
    record = rs.record.Record.from_main_line(init_sfen, [move1, move2], None)

    with pytest.raises(ValueError, match="missing eval"):
        record.to_psv()


def test_game_record_roundtrip_preserves_main_terminal_kind() -> None:
    record = rs.record.Record.from_main_line(
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        [rs.record.MoveEntry("7g7f")],
        rs.record.SpecialMoveEntry("WIN_BY_ILLEGAL_MOVE", rs.record.GameResult.BLACK_WIN_BY_ILLEGAL_MOVE),
    )

    payload = record.to_dict()
    assert "main_terminal" not in payload
    assert payload["result"]["result"] == int(rs.record.GameResult.BLACK_WIN_BY_ILLEGAL_MOVE)
    rebuilt = rs.record.Record.from_dict(payload)
    assert rebuilt.main_terminal is not None
    assert rebuilt.result == rs.record.GameResult.BLACK_WIN_BY_ILLEGAL_MOVE


def test_game_record_roundtrip_without_terminal_keeps_none() -> None:
    record = rs.record.Record(
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        moves=[rs.record.MoveEntry("7g7f")],
    )

    payload = record.to_dict()
    assert "main_terminal" not in payload
    assert payload["result"]["result"] == int(rs.record.GameResult.INVALID)

    rebuilt = rs.record.Record.from_dict(payload)
    assert rebuilt.main_terminal is None
    assert rebuilt.result == rs.record.GameResult.INVALID


def test_game_record_from_dict_rejects_main_terminal_payload() -> None:
    payload = {
        "init_position_sfen": "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        "moves": [{"move": "7g7f"}],
        "result": {"result": "BLACK_WIN_BY_TIMEOUT"},
        "main_terminal": {
            "kind": "RESIGN",
            "result": "WHITE_WIN",
            "comment": "terminal wins",
            "time_ms": 1234,
            "raw": "resigned",
        },
    }

    with pytest.raises(ValueError):
        rs.record.Record.from_dict(payload)


def test_game_record_from_dict_uses_result() -> None:
    payload = {
        "init_position_sfen": "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        "moves": [{"move": "7g7f"}],
        "result": {
            "result": "BLACK_WIN_BY_TIMEOUT",
            "reason": "time over",
            "end_time_ms": 4321,
            "end_comment": "timeout",
        },
    }

    record = rs.record.Record.from_dict(payload)
    assert record.main_terminal is not None
    assert record.main_terminal.kind == "TIMEOUT"
    assert record.main_terminal.raw == "time over"
    assert record.main_terminal.time_ms == 4321
    assert record.main_terminal.comment == "timeout"
    assert record.result == rs.record.GameResult.BLACK_WIN_BY_TIMEOUT


def test_game_record_from_dict_shogiarena_shape() -> None:
    payload = {
        "metadata": {
            "game_name": "run-20260211-0001",
            "game_type": "tournament",
            "black_player": "EngineA",
            "white_player": "EngineB",
            "start_date": "2026-02-11T10:12:33+00:00",
            "end_date": "2026-02-11T10:18:02+00:00",
            "updated_date": "2026-02-11T10:18:02+00:00",
            "black_time_control": "300+10+0",
            "white_time_control": "300+10+0",
            "attributes": {"runMode": "tournament", "recordFormat": "sbinpack"},
        },
        "init_position_sfen": "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        "moves": [
            {
                "move": "7g7f",
                "time_ms": 812,
                "engine_info": {
                    "eval": 34,
                    "nodes": 145000,
                    "depth": 18,
                    "seldepth": 27,
                    "extras": {
                        "nps": 180000,
                        "arena.wall_time_ms": 820,
                        "arena.latency_delta_ms": 8,
                    },
                },
            },
            {
                "move": "3c3d",
                "time_ms": 701,
                "engine_info": {"eval": -22, "depth": 16, "nodes": 98000, "seldepth": 21},
            },
        ],
        "result": {
            "result": "BLACK_WIN",
            "ply_count": 2,
            "reason": "まで2手で先手の勝ち",
            "end_time_ms": 0,
            "end_comment": None,
        },
    }
    record = rs.record.Record.from_dict(payload)
    assert record.move_count == 2
    assert record.result == rs.record.GameResult.BLACK_WIN
    assert record.main_terminal is not None
    assert record.main_terminal.raw == "まで2手で先手の勝ち"
    assert record.main_terminal.time_ms == 0
    assert record.moves[0].engine_info is not None
    assert record.moves[0].engine_info.eval == 34
    assert record.moves[0].engine_info.extras["nps"] == 180000
    assert record.metadata.black_player == "EngineA"
    assert record.metadata.white_player == "EngineB"
    assert record.metadata.game_name == "run-20260211-0001"
    assert record.metadata.game_type == "tournament"
    assert record.metadata.updated_date == "2026-02-11T10:18:02+00:00"

    encoded = record.to_dict()
    assert encoded["result"]["result"] == int(rs.record.GameResult.BLACK_WIN)
    assert encoded["result"]["ply_count"] == 2
    assert encoded["metadata"]["game_name"] == "run-20260211-0001"
    assert encoded["metadata"]["black_time_control"]["base_seconds"] == 300
    assert "time_specs" not in encoded["metadata"]
    assert encoded["moves"][0]["engine_info"]["eval"] == 34


def test_time_control_from_spec_and_normalize_spec() -> None:
    tc = rs.record.TimeControl.from_spec("300+10+0")
    assert tc.base_seconds == 300
    assert tc.byoyomi_seconds == 10
    assert tc.increment_seconds == 0
    assert rs.record.TimeControl.normalize_spec("300+10+0") == "300+10+0"
    assert rs.record.TimeControl.normalize_spec("300.0+10+0") == "300+10+0"

    with pytest.raises(ValueError):
        rs.record.TimeControl.from_spec("bad-spec")

    with pytest.raises(ValueError):
        rs.record.TimeControl.normalize_spec("300+10")


@pytest.mark.parametrize(
    ("label", "move_value"),
    [
        ("usi", "7g7f"),
        ("mv", rs.core.Move.from_usi("7g7f")),
        ("u16", int(rs.core.Move.from_usi("7g7f"))),
        ("u32", int(rs.core.Move32.from_usi("7g7f"))),
    ],
)
def test_from_dict_move_normalization_supports_exports(label: str, move_value: object) -> None:
    payload = {
        "init_position_sfen": "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        "moves": [
            {"move": move_value, "engine_info": {"eval": 0}},
            {"move": "3c3d", "engine_info": {"eval": 0}},
        ],
        "result": {"result": "WHITE_WIN"},
    }

    record = rs.record.Record.from_dict(payload)
    assert record.move_count == 2
    assert record.moves[0].move.to_usi() == "7g7f"
    assert record.moves[1].move.to_usi() == "3c3d"

    csa = record.to_csa()
    assert "+7776FU" in csa

    kif = record.to_kif()
    assert "７六歩" in kif

    metadata = b"user-defined-metadata"
    packed = record.to_sbinpack(metadata=metadata)
    assert isinstance(packed, bytes)
    assert packed[:4] == b"SBN2"
    assert len(packed) > 0
    restored = rs.record.Record.from_sbinpack(packed)
    restored_with_metadata, restored_metadata = rs.record.Record.from_sbinpack_with_metadata(packed)
    assert restored.moves[0].move.to_usi() == "7g7f"
    assert restored_with_metadata.moves[1].move.to_usi() == "3c3d"
    assert restored_metadata == metadata
    with pytest.raises(ValueError, match="MetadataTooLarge"):
        record.to_sbinpack(metadata=b"x" * 128)


def test_from_dict_rejects_illegal_move_immediately() -> None:
    payload = {
        "init_position_sfen": "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        "moves": ["7f7e"],
        "result": {"result": "WHITE_WIN"},
    }
    with pytest.raises(ValueError):
        rs.record.Record.from_dict(payload)


def test_update_metadata_patch_rules_and_unknown_key_error() -> None:
    record = rs.record.Record("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1")

    record.update_metadata(
        {
            "event": "match-1",
            "black_player": "EngineA",
            "white_player": "EngineB",
            "game_name": "run-1",
            "black_time_control": "300+10+0",
            "white_time_control": "300+10+0",
        }
    )
    assert record.metadata.event == "match-1"
    assert record.metadata.black_player == "EngineA"
    assert record.metadata.white_player == "EngineB"
    assert record.metadata.game_name == "run-1"
    assert record.metadata.black_time_control.to_spec() == "300+10+0"

    record.update_metadata({"event": None, "attributes": None})
    assert record.metadata.event is None
    assert record.metadata.attributes == {}
    assert record.metadata.black_time_control is not None
    assert record.metadata.white_time_control is not None

    record.update_metadata({"unknown_key": "x"})
    assert record.metadata.attributes["unknown_key"] == "x"

    with pytest.raises(ValueError):
        record.update_metadata({"unknown_key_strict": "x"}, strict=True)


def test_time_control_black_white_are_always_present_as_properties() -> None:
    metadata = rs.record.RecordMetadata()
    assert metadata.black_time_control is None
    assert metadata.white_time_control is None


def test_metadata_updated_date_and_game_fields_are_first_class() -> None:
    metadata = rs.record.RecordMetadata(
        game_name="championship",
        game_type="league",
        updated_date="2026-02-13T10:00:00+09:00",
    )
    assert metadata.game_name == "championship"
    assert metadata.game_type == "league"
    assert metadata.updated_date == "2026-02-13T10:00:00+09:00"
    assert "game_name" not in metadata.attributes
    assert "game_type" not in metadata.attributes
    assert "updated_date" not in metadata.attributes


def test_from_dict_rejects_time_specs_key() -> None:
    base = {
        "init_position_sfen": "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        "moves": [{"move": "7g7f"}],
        "result": {"result": "WHITE_WIN"},
    }

    with pytest.raises(ValueError):
        rs.record.Record.from_dict(
            {
                **base,
                "metadata": {"time_specs": None},
            },
            strict=True,
        )

    with pytest.raises(ValueError):
        rs.record.Record.from_dict(
            {
                **base,
                "metadata": {"time_specs": None},
            }
        )

    with pytest.raises(ValueError):
        rs.record.Record.from_dict(
            {
                **base,
                "metadata": {"black_player_name": "EngineA"},
            }
        )

    with pytest.raises(ValueError):
        rs.record.Record.from_dict(
            {
                **base,
                "metadata": {"time_control_black": "300+10+0"},
            }
        )


def test_from_dict_strict_and_permissive_modes() -> None:
    base = {
        "init_position_sfen": "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        "moves": [{"move": "7g7f"}],
        "result": {"result": "WHITE_WIN"},
    }

    permissive_payload = dict(base)
    permissive_payload["metadata"] = {
        "black_player": "EngineA",
        "black_time_control": "300+10+0",
        "custom_flag": "x",
    }
    permissive = rs.record.Record.from_dict(permissive_payload, strict=False)
    assert permissive.metadata.black_player == "EngineA"
    assert permissive.metadata.black_time_control is not None
    assert permissive.metadata.attributes["custom_flag"] == "x"

    with pytest.raises(ValueError):
        rs.record.Record.from_dict(permissive_payload, strict=True)

    strict_ok_payload = dict(base)
    strict_ok_payload["metadata"] = {
        "black_player": "EngineA",
        "black_time_control": "300+10+0",
    }
    strict_ok = rs.record.Record.from_dict(strict_ok_payload, strict=True)
    assert strict_ok.metadata.black_player == "EngineA"
    assert strict_ok.metadata.black_time_control is not None


def test_from_dict_strict_rejects_unknown_nested_keys() -> None:
    base = {
        "init_position_sfen": "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        "metadata": {"event": "match"},
    }

    move_unknown = dict(base)
    move_unknown["moves"] = [{"move": "7g7f", "unknown_key": "x"}]
    move_unknown["result"] = {"result": "WHITE_WIN"}
    with pytest.raises(ValueError):
        rs.record.Record.from_dict(move_unknown, strict=True)

    result_unknown = dict(base)
    result_unknown["moves"] = [{"move": "7g7f"}]
    result_unknown["result"] = {"result": "WHITE_WIN", "unknown_key": "x"}
    with pytest.raises(ValueError):
        rs.record.Record.from_dict(result_unknown, strict=True)


def test_strict_from_dict_accepts_to_dict_output_roundtrip() -> None:
    record = rs.record.Record.from_main_line(
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        [rs.record.MoveEntry("7g7f", engine_info=rs.record.EngineInfo(eval=10))],
        rs.record.SpecialMoveEntry("RESIGN", rs.record.GameResult.WHITE_WIN),
        rs.record.RecordMetadata(game_name="match"),
    )
    payload = record.to_dict()
    rebuilt = rs.record.Record.from_dict(payload, strict=True)
    assert rebuilt.game_name == "match"
    assert rebuilt.move_count == 1


def test_update_metadata_strict_rejects_compat_keys() -> None:
    record = rs.record.Record("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1")

    with pytest.raises(ValueError):
        record.update_metadata({"black_player_name": "EngineA"}, strict=True)

    with pytest.raises(ValueError):
        record.update_metadata({"time_control_black": "300+10+0"}, strict=True)

    record.update_metadata({"black_time_control": "300+10+0"}, strict=True)
    assert record.metadata.black_time_control is not None


def test_update_metadata_permissive_rejects_removed_keys() -> None:
    record = rs.record.Record("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1")

    with pytest.raises(ValueError):
        record.update_metadata({"black_player_name": "EngineA"})

    with pytest.raises(ValueError):
        record.update_metadata({"time_control_black": "300+10+0"})

    with pytest.raises(ValueError):
        record.update_metadata({"time_specs": {"black": "300+10+0"}})


def test_record_node_id_overflow_returns_python_error() -> None:
    record = rs.record.Record("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1")
    huge_node_id = 2**64 - 1

    with pytest.raises(IndexError):
        record.node_parent(huge_node_id)

    with pytest.raises(IndexError):
        record.node_move(huge_node_id)

    with pytest.raises(IndexError):
        rs.record.RecordNodeId(huge_node_id)

    editor = rs.record.RecordEditor("startpos")
    with pytest.raises(IndexError):
        editor.go_to(huge_node_id)


def test_game_record_typed_accessors() -> None:
    record = rs.record.Record.from_main_line(
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        [rs.record.MoveEntry("7g7f")],
        rs.record.SpecialMoveEntry(
            "TIMEOUT",
            rs.record.GameResult.WHITE_WIN_BY_TIMEOUT,
            time_ms=2345,
            comment="timeout",
        ),
        rs.record.RecordMetadata(
            game_name="run-1",
            game_type="arena",
            updated_date="2026-02-13T11:00:00+09:00",
            black_time_control=rs.record.TimeControl.from_spec("300+10+0"),
            white_time_control=rs.record.TimeControl.from_spec("300+10+0"),
        ),
    )

    assert record.game_name == "run-1"
    assert record.game_type == "arena"
    assert record.updated_date == "2026-02-13T11:00:00+09:00"
    assert record.black_time_control is not None
    assert record.white_time_control is not None
    assert record.end_time_ms == 2345
    assert record.end_comment == "timeout"


def test_move_engine_info_typed_extras_accessors() -> None:
    info = rs.record.EngineInfo()
    assert info.wall_time_ms is None
    assert info.latency_delta_ms is None

    info.wall_time_ms = 1234
    info.latency_delta_ms = 15
    assert info.wall_time_ms == 1234
    assert info.latency_delta_ms == 15
    assert info.extras["wall_time_ms"] == 1234
    assert info.extras["latency_delta_ms"] == 15


def test_result_info_typed_access() -> None:
    record = rs.record.Record.from_main_line(
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        [rs.record.MoveEntry("7g7f"), rs.record.MoveEntry("3c3d")],
        rs.record.SpecialMoveEntry(
            "RESIGN",
            rs.record.GameResult.BLACK_WIN,
            raw="まで2手で先手の勝ち",
            time_ms=1000,
            comment="終局",
        ),
    )
    info = record.result_info
    assert info.result == rs.record.GameResult.BLACK_WIN
    assert info.ply_count == 2
    assert info.reason == "まで2手で先手の勝ち"
    assert info.end_time_ms == 1000
    assert info.end_comment == "終局"


def test_result_info_is_snapshot() -> None:
    record = rs.record.Record.from_main_line(
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        [rs.record.MoveEntry("7g7f"), rs.record.MoveEntry("3c3d")],
        rs.record.SpecialMoveEntry(
            "RESIGN",
            rs.record.GameResult.BLACK_WIN,
            raw="まで2手で先手の勝ち",
            time_ms=1000,
            comment="終局A",
        ),
    )

    before = record.result_info

    record.set_main_terminal(
        rs.record.SpecialMoveEntry(
            "TIMEOUT",
            rs.record.GameResult.WHITE_WIN_BY_TIMEOUT,
            raw="まで2手で後手の時間切れ勝ち",
            time_ms=2000,
            comment="終局B",
        )
    )
    after = record.result_info

    assert before.result == rs.record.GameResult.BLACK_WIN
    assert before.reason == "まで2手で先手の勝ち"
    assert before.end_time_ms == 1000
    assert before.end_comment == "終局A"

    assert after.result == rs.record.GameResult.WHITE_WIN_BY_TIMEOUT
    assert after.reason == "まで2手で後手の時間切れ勝ち"
    assert after.end_time_ms == 2000
    assert after.end_comment == "終局B"
