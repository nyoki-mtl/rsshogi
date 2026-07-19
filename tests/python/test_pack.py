from __future__ import annotations

from pathlib import Path

import pytest

rsshogi = pytest.importorskip("rsshogi")

FIXTURE_PATH = Path("crates/rsshogi/tests/test_data/pack/sample.pack")


def _make_record(
    *,
    terminal_kind: str,
    result: object,
    raw: str | None = None,
    metadata: object | None = None,
) -> object:
    moves = [
        rsshogi.record.MoveEntry(
            "7g7f",
            engine_info=rsshogi.record.EngineInfo(eval=120),
        ),
        rsshogi.record.MoveEntry(
            "3c3d",
            engine_info=rsshogi.record.EngineInfo(eval=-80),
        ),
    ]
    terminal = rsshogi.record.SpecialMoveEntry(terminal_kind, result, raw=raw)
    return rsshogi.record.Record.from_main_line(
        rsshogi.core.Board().to_sfen(),
        moves,
        terminal,
        metadata,
    )


def test_game_record_pack_roundtrip_single_game() -> None:
    record = _make_record(
        terminal_kind="TIMEOUT",
        result=rsshogi.record.GameResult.WHITE_WIN_BY_TIMEOUT,
    )

    packed = record.to_pack()
    restored = rsshogi.record.Record.from_pack(packed)

    assert restored.result == rsshogi.record.GameResult.WHITE_WIN_BY_TIMEOUT
    assert restored.moves[0].engine_info is not None
    assert restored.moves[0].engine_info.eval == 120
    assert restored.moves[1].engine_info is not None
    assert restored.moves[1].engine_info.eval == -80
    assert restored.main_terminal is not None
    assert restored.main_terminal.raw == "pack:end_reason=time_up"


@pytest.mark.parametrize(
    ("terminal_kind", "result", "metadata", "expected_raw"),
    [
        (
            "TRY",
            lambda: rsshogi.record.GameResult.BLACK_WIN_BY_TRY_RULE,
            lambda: rsshogi.record.RecordMetadata(impasse_rule="TryRule"),
            "pack:end_reason=win_try_rule",
        ),
        (
            "WIN_BY_DECLARATION",
            lambda: rsshogi.record.GameResult.BLACK_WIN_BY_DECLARATION,
            lambda: rsshogi.record.RecordMetadata(impasse_rule="CSARule24"),
            "pack:end_reason=win_csa24",
        ),
        (
            "LOSE_BY_ILLEGAL_MOVE",
            lambda: rsshogi.record.GameResult.WHITE_WIN_BY_ILLEGAL_MOVE,
            lambda: None,
            "pack:end_reason=illegal_move",
        ),
        (
            "TIMEOUT",
            lambda: rsshogi.record.GameResult.BLACK_WIN_BY_TIMEOUT,
            lambda: None,
            "pack:end_reason=time_up",
        ),
    ],
)
def test_pack_roundtrip_terminal_reason_variants(
    terminal_kind: str,
    result: object,
    metadata: object,
    expected_raw: str,
) -> None:
    record = _make_record(
        terminal_kind=terminal_kind,
        result=result(),
        metadata=metadata(),
    )

    restored = rsshogi.record.Record.from_pack(record.to_pack())
    assert restored.result == result()
    assert restored.main_terminal is not None
    assert restored.main_terminal.raw == expected_raw


def test_pack_multi_game_helpers_and_file_io(tmp_path: Path) -> None:
    record1 = _make_record(
        terminal_kind="RESIGN",
        result=rsshogi.record.GameResult.BLACK_WIN,
    )
    record2 = _make_record(
        terminal_kind="TRY",
        result=rsshogi.record.GameResult.BLACK_WIN_BY_TRY_RULE,
        metadata=rsshogi.record.RecordMetadata(impasse_rule="TryRule"),
    )

    packed = rsshogi.record.write_pack([record1, record2])
    restored = rsshogi.record.decode_pack(packed)
    assert len(restored) == 2
    assert restored[0].result == rsshogi.record.GameResult.BLACK_WIN
    assert restored[1].result == rsshogi.record.GameResult.BLACK_WIN_BY_TRY_RULE

    with pytest.raises(ValueError, match="multiple games not supported"):
        rsshogi.record.Record.from_pack(packed)

    path = tmp_path / "sample.pack"
    rsshogi.record.write_pack_file(path, [record1, record2])
    restored_from_file = rsshogi.record.decode_pack_file(path)
    assert [record.result for record in restored_from_file] == [
        rsshogi.record.GameResult.BLACK_WIN,
        rsshogi.record.GameResult.BLACK_WIN_BY_TRY_RULE,
    ]


def test_sbinpack_batch_writer_accepts_per_record_metadata(tmp_path: Path) -> None:
    record1 = _make_record(
        terminal_kind="RESIGN",
        result=rsshogi.record.GameResult.BLACK_WIN,
    )
    record2 = _make_record(
        terminal_kind="TIMEOUT",
        result=rsshogi.record.GameResult.WHITE_WIN_BY_TIMEOUT,
    )

    packed = rsshogi.record.write_sbinpack([record1], metadatas=[b"batch-meta"])
    restored, metadata = rsshogi.record.Record.from_sbinpack_with_metadata(packed)
    assert metadata == b"batch-meta"
    assert restored.result == rsshogi.record.GameResult.BLACK_WIN

    empty_meta = rsshogi.record.write_sbinpack([record1], metadatas=[None])
    _, metadata = rsshogi.record.Record.from_sbinpack_with_metadata(empty_meta)
    assert metadata == b""

    multi = rsshogi.record.write_sbinpack([record1, record2], metadatas=[b"one", b"two"])
    assert multi[:4] == b"SBN2"
    decoded = rsshogi.record.decode_sbinpack(multi)
    assert [record.result for record, _ in decoded] == [
        rsshogi.record.GameResult.BLACK_WIN,
        rsshogi.record.GameResult.WHITE_WIN_BY_TIMEOUT,
    ]
    assert [metadata for _, metadata in decoded] == [b"one", b"two"]
    with pytest.raises(ValueError, match="multiple chains not supported"):
        rsshogi.record.Record.from_sbinpack(multi)

    path = tmp_path / "sample.sbinpack"
    rsshogi.record.write_sbinpack_file(path, [record1], metadatas=[b"file-meta"])
    _, file_metadata = rsshogi.record.Record.from_sbinpack_with_metadata(path.read_bytes())
    assert file_metadata == b"file-meta"
    decoded_file = rsshogi.record.decode_sbinpack_file(path)
    assert len(decoded_file) == 1
    assert decoded_file[0][0].result == rsshogi.record.GameResult.BLACK_WIN
    assert decoded_file[0][1] == b"file-meta"

    with pytest.raises(ValueError, match="metadatas length"):
        rsshogi.record.write_sbinpack([record1], metadatas=[])
    with pytest.raises(ValueError, match="MetadataTooLarge"):
        rsshogi.record.write_sbinpack([record1], metadatas=[b"x" * 128])


def test_pack_export_requires_eval() -> None:
    record = rsshogi.record.Record.from_main_line(
        rsshogi.core.Board().to_sfen(),
        [rsshogi.record.MoveEntry("7g7f")],
        rsshogi.record.SpecialMoveEntry("RESIGN", rsshogi.record.GameResult.BLACK_WIN),
    )
    with pytest.raises(ValueError, match="missing eval"):
        record.to_pack()


def test_pack_fixture_smoke() -> None:
    records = rsshogi.record.decode_pack_file(FIXTURE_PATH)
    assert len(records) == 5
    assert all(record.move_count > 0 for record in records)
    assert records[0].main_terminal is not None


def test_game_result_try_rule_members_exposed() -> None:
    assert rsshogi.record.GameResult.BLACK_WIN_BY_TRY_RULE.value == 20
    assert rsshogi.record.GameResult.WHITE_WIN_BY_TRY_RULE.value == 21
    assert rsshogi.record.GameResult.from_str("black_win_by_try_rule") == (
        rsshogi.record.GameResult.BLACK_WIN_BY_TRY_RULE
    )
