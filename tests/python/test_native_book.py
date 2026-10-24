"""Lossless native book bindings and exclusive editing behavior."""

from pathlib import Path

import pytest
from rsshogi.book import (
    NativeBook,
    NativeBookBuilder,
    NativeBookEditor,
    NativeBookEntry,
    NativeBookMetadata,
    NativeBookPosition,
    NativeBound,
    NativeCandidate,
    NativeScore,
)
from rsshogi.core import Board, Move


@pytest.mark.parametrize("depth_kind", [0, 1, 2, 3, 2**32 - 1])
def test_native_depth_kind_roundtrip(tmp_path: Path, depth_kind: int):
    path = tmp_path / "depth-kind.rsbk"
    metadata = NativeBookMetadata("title", "description", depth_kind=depth_kind)
    assert NativeBookMetadata().depth_kind == 0
    assert metadata.depth_kind == depth_kind
    builder = NativeBookBuilder(path, metadata)
    builder.push(NativeBookPosition(NativeBookEntry(Board())))
    builder.finish()
    book = NativeBook.open(path)
    book.validate_full()
    assert book.metadata().depth_kind == depth_kind
    del book
    editor = NativeBookEditor.open(path)
    assert editor.metadata().depth_kind == depth_kind
    editor.set_metadata(NativeBookMetadata("updated", "description", depth_kind=depth_kind))
    assert editor.save()
    editor.close()
    book = NativeBook.open(path)
    book.validate_full()
    assert book.metadata().depth_kind == depth_kind
    assert book.metadata().title == "updated"
    del book
    editor = NativeBookEditor.open(path)
    editor.compact()
    editor.close()
    book = NativeBook.open(path)
    book.validate_full()
    assert book.metadata().depth_kind == depth_kind


@pytest.mark.parametrize("depth_kind", [-1, 2**32])
def test_native_depth_kind_integer_limits(depth_kind: int):
    with pytest.raises(OverflowError):
        NativeBookMetadata(depth_kind=depth_kind)


def test_native_scores_and_integer_limits(tmp_path: Path):
    board = Board()
    moves = [Move.from_usi(text) for text in ("7g7f", "2g2f", "6g6f", "5g5f", "4g4f")]
    scores = [
        NativeScore.unknown(),
        NativeScore.cp(-(2**31), NativeBound.Lower),
        NativeScore.mate(2**31 - 1, NativeBound.Upper),
        NativeScore.mate_unknown(False),
        NativeScore.mate(-1),
    ]
    candidates = [
        NativeCandidate(mv, score, 2**32 - 1 if i == 1 else None, i)
        for i, (mv, score) in enumerate(zip(moves, scores, strict=True))
    ]
    path = tmp_path / "scores.rsbk"
    builder = NativeBookBuilder(path, NativeBookMetadata("title", "description"))
    builder.push(NativeBookPosition(NativeBookEntry(board, candidates)))
    builder.finish()
    with pytest.raises(RuntimeError, match="finished"):
        builder.finish()
    book = NativeBook.open(path)
    book.validate_full()
    assert len(book) == 1
    assert book.tail() is None
    assert book.metadata().title == "title"
    entry = book.get(board)
    assert entry is not None
    assert len(entry.key) == 32
    restored = sorted(entry.candidates, key=lambda candidate: candidate.display_order)
    assert [(c.score.kind, c.score.value, c.score.bound, c.score.winning) for c in restored] == [
        ("unknown", None, None, None),
        ("cp", -(2**31), NativeBound.Lower, None),
        ("mate", 2**31 - 1, NativeBound.Upper, None),
        ("mate_unknown", None, None, False),
        ("mate", -1, NativeBound.Exact, None),
    ]
    assert restored[1].depth == 2**32 - 1
    assert restored[0].depth is None
    assert not hasattr(restored[0], "count")
    assert len(book.entries()) == len(book.records()) == 1
    assert book.io_stats()[0] > 0
    with pytest.raises(OverflowError):
        NativeCandidate(moves[0], depth=2**32)
    with pytest.raises(OverflowError):
        NativeCandidate(moves[0], depth=-1)
    with pytest.raises(TypeError):
        NativeCandidate(moves[0], count=1)
    with pytest.raises(ValueError, match="mate"):
        NativeBookEntry(board, [NativeCandidate(moves[0], NativeScore.mate(0))])


def test_native_edit_comments_rank_save_lock_reopen(tmp_path: Path):
    path = tmp_path / "edit.rsbk"
    board = Board()
    first, second = Move.from_usi("7g7f"), Move.from_usi("2g2f")
    editor = NativeBookEditor.create(path)
    with pytest.raises(BlockingIOError, match="busy"):
        NativeBookEditor.open(path)
    assert not editor.is_dirty()
    assert not editor.save()
    editor.upsert_candidate(board, NativeCandidate(first), "move\r\ncomment")
    editor.upsert_candidate(board, NativeCandidate(second, NativeScore.cp(0), depth=0))
    editor.set_position_comment(board, "position\rcomment")
    assert editor.reorder_candidate(board, second, 0)
    editor.upsert_candidate(board, NativeCandidate(first, NativeScore.cp(42)))
    dirty = editor.record(board)
    assert dirty is not None
    assert dirty.move_comments[int(first)] == "move\ncomment"
    assert dirty.comment == "position\ncomment"
    assert editor.is_dirty()
    assert editor.save()
    size = path.stat().st_size
    assert not editor.save()
    assert path.stat().st_size == size
    editor.close()
    with pytest.raises(RuntimeError, match="closed"):
        editor.save()
    book = NativeBook.open(path)
    book.validate_full()
    record = book.record(board)
    assert record is not None
    ordered = sorted(record.entry.candidates, key=lambda candidate: candidate.display_order)
    assert str(ordered[0].mv) == str(second)
    assert ordered[0].depth == 0
    assert record.move_comments[int(first)] == "move\ncomment"
    assert book.position_comment(board) == "position\ncomment"
    assert book.move_comment(board, first) == "move\ncomment"
    assert book.move_comment(board, second) is None
    del book
    editor = NativeBookEditor.open(path)
    editor.upsert_candidate(board, NativeCandidate(first), "")
    assert int(first) not in editor.record(board).move_comments
    assert editor.remove_candidate(board, second)
    assert editor.save()
    editor.compact()
    editor.close()
    book = NativeBook.open(path)
    book.validate_full()
    assert len(book.get(board).candidates) == 1


def test_native_position_comments_empty_positions_and_io(tmp_path: Path):
    path = tmp_path / "empty-position.rsbk"
    editor = NativeBookEditor.create(path)
    board = Board()
    editor.upsert_position(NativeBookPosition(NativeBookEntry(board), "only comment"))
    editor.set_metadata(NativeBookMetadata("updated", ""))
    assert editor.save()
    editor.close()
    book = NativeBook.open(path)
    assert book.metadata().description is None
    assert book.record(board).comment == "only comment"
    assert book.get(board).candidates == []
    del book
    editor = NativeBookEditor.open(path)
    assert editor.remove_position(board)
    assert not editor.remove_position(board)
    assert editor.save()
    editor.close()
    assert len(NativeBook.open(path)) == 0
    with pytest.raises(OSError):
        NativeBook.open(tmp_path / "absent.rsbk")
    invalid = tmp_path / "invalid.rsbk"
    invalid.write_bytes(b"invalid")
    with pytest.raises(ValueError):
        NativeBook.open(invalid)


def test_native_tail_diagnostics_recovery_and_salvage(tmp_path: Path):
    from rsshogi.book import NativeBookLogError

    path = tmp_path / "tail.rsbk"
    editor = NativeBookEditor.create(path)
    prefix = path.read_bytes()
    editor.set_position_comment(Board(), "incomplete save")
    editor.save()
    editor.close()
    path.write_bytes(path.read_bytes()[:-1])
    damaged = path.read_bytes()
    book = NativeBook.open(path)
    assert book.tail() == (len(prefix), len(damaged))
    del book
    with pytest.raises(NativeBookLogError) as caught:
        NativeBookEditor.open(path)
    assert caught.value.offset == caught.value.valid_end == len(prefix)
    assert caught.value.sequence == 0
    assert "recovery" in caught.value.reason
    output = tmp_path / "salvaged.rsbk"
    report = NativeBookEditor.salvage_prefix(path, output)
    assert report.source_bytes == len(damaged)
    assert report.valid_end == report.error_offset == len(prefix)
    assert report.sequence == 0
    assert Path(report.output) == output
    assert path.read_bytes() == damaged
    NativeBook.open(output).validate_full()
    backup = NativeBookEditor.recover_tail(path)
    assert Path(backup).is_file()
    assert Path(backup).read_bytes()[24:] == damaged[len(prefix) :]
    assert path.read_bytes() == prefix
    editor = NativeBookEditor.open(path)
    editor.close()


@pytest.mark.parametrize("block_positions", [1, 3, 64, 256, 1024, 16384])
def test_native_builder_block_positions(tmp_path: Path, block_positions: int):
    path = tmp_path / "blocks.book"
    builder = NativeBookBuilder(path, block_positions=block_positions)
    board = Board()
    for move in ["7g7f", "3c3d", "2g2f", "8c8d"]:
        builder.push(NativeBookPosition(NativeBookEntry(board), comment="comment"))
        board.push_usi(move)
    builder.finish()
    book = NativeBook.open(path)
    book.validate_full()
    assert book.block_positions == block_positions
    assert len(book) == 4
    del book
    editor = NativeBookEditor.open(path)
    editor.set_metadata(NativeBookMetadata("updated", depth_kind=2**32 - 1))
    editor.compact()
    editor.close()
    book = NativeBook.open(path)
    book.validate_full()
    assert book.block_positions == block_positions
    assert book.metadata().depth_kind == 2**32 - 1
    assert all(record.comment == "comment" for record in book.records())


@pytest.mark.parametrize("block_positions", [0, 16385, 2**32 - 1])
def test_native_builder_invalid_block_positions(tmp_path: Path, block_positions: int):
    path = tmp_path / "invalid.book"
    with pytest.raises(ValueError):
        NativeBookBuilder(path, block_positions=block_positions)
    assert not path.exists()


@pytest.mark.parametrize("exists_before_start", [True, False])
def test_native_builder_existing_destination(tmp_path: Path, exists_before_start: bool):
    path = tmp_path / "existing.book"
    original = b"existing destination"
    if exists_before_start:
        path.write_bytes(original)
        with pytest.raises(FileExistsError):
            NativeBookBuilder(path)
    else:
        builder = NativeBookBuilder(path)
        path.write_bytes(original)
        with pytest.raises(FileExistsError):
            builder.finish()
    assert path.read_bytes() == original


def test_native_builder_default_block_positions(tmp_path: Path):
    path = tmp_path / "default.book"
    NativeBookBuilder(path).finish()
    assert NativeBook.open(path).block_positions == 64
