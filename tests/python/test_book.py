"""Tests for rsshogi.book module."""

import rsshogi as rs


def test_yaneuraou_book_lookup(tmp_path):
    """Test YaneuraOu DB2016 lookup preserves external metadata."""
    path = tmp_path / "book.db"
    path.write_text(
        "\ufeff#YANEURAOU-DB2016 1.00\r\n"
        "sfen lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 23\r\n"
        "// root comment\r\n"
        "7g7f 3c3d 12 8 99\r\n"
        "# move comment\r\n",
        encoding="utf-8",
    )

    book = rs.book.YaneuraOuBook.open(str(path))
    diagnostics = book.diagnostics()
    assert diagnostics.kind == "sorted"

    entry = book.lookup_sfen("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1")
    assert entry is not None
    assert entry.min_ply == 23
    assert "root comment" in entry.comment
    assert len(entry.moves) == 1
    assert str(entry.moves[0].mv) == "7g7f"
    assert str(entry.moves[0].ponder) == "3c3d"
    assert entry.moves[0].score == 12
    assert entry.moves[0].depth == 8
    assert entry.moves[0].count == 99
    assert entry.moves[0].comment == "move comment"


def test_sbk_book_open(tmp_path):
    """Test SBK lookup with generated protobuf data."""
    path = tmp_path / "book.sbk"
    path.write_bytes(
        _field_string(1, "author")
        + _field_string(2, "desc")
        + _field_message(
            3,
            _field_varint(1, 0)
            + _field_string(
                7,
                "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
            )
            + _field_varint(4, 10)
            + _field_varint(5, 6)
            + _field_varint(6, 4)
            + _field_string(8, "root")
            + _field_message(
                9,
                _field_varint(1, 0x01007677) + _field_varint(2, 3) + _field_varint(3, 42),
            )
            + _field_message(
                10,
                _field_varint(1, 120)
                + _field_varint(2, 18)
                + _field_varint(3, 24)
                + _field_varint(4, 1234)
                + _field_string(5, "7g7f")
                + _field_string(6, "engine"),
            ),
        )
    )

    progress = []
    book = rs.book.SbkBook.open_with_control(
        str(path), lambda step: progress.append((step.indexed_states, step.total_states))
    )
    entry = book.lookup_sfen("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1")

    assert progress == [(1, 1)]
    assert book.author == "author"
    assert book.description == "desc"
    assert len(book) == 1
    assert book.diagnostics().duplicate_positions == 0
    assert entry is not None
    assert entry.state_id == 0
    assert entry.games == 10
    assert entry.won_black == 6
    assert entry.won_white == 4
    assert entry.comment == "root"
    assert str(entry.moves[0].mv) == "7g7f"
    assert entry.moves[0].evaluation == 3
    assert entry.moves[0].weight == 42
    assert entry.evals[0].evaluation_value == 120
    assert entry.evals[0].depth == 18
    assert entry.evals[0].sel_depth == 24
    assert entry.evals[0].nodes == 1234
    assert entry.evals[0].variation == "7g7f"
    assert entry.evals[0].engine_name == "engine"


def test_sbk_book_direct_state_lookup(tmp_path):
    """Test SBK direct state-id and child-entry lookup."""
    path = tmp_path / "book.sbk"
    root_sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"
    root_move = _field_message(
        9,
        _field_varint(1, 0x0100_7677) + _field_varint(3, 1) + _field_varint(4, 20),
    )
    child_move = _field_message(9, _field_varint(1, 0x0100_3433) + _field_varint(3, 2))
    path.write_bytes(
        _field_message(3, _field_varint(1, 10) + root_move) + _field_message(3, _field_varint(1, 20) + child_move)
    )

    book = rs.book.SbkBook.open(str(path))
    root = book.lookup_sfen(root_sfen)
    assert root is not None

    child = book.child_entry(root, 0)
    by_id = book.lookup_state_id(20)
    by_index = book.lookup_state_index(1)

    assert root.state_id == 10
    assert child is not None
    assert by_id is not None
    assert by_index is not None
    assert child.state_id == 20
    assert child.state_index == 1
    assert child.sfen == by_id.sfen == by_index.sfen
    assert str(child.moves[0].mv) == "3c3d"
    assert book.lookup_state_id(-1) is None
    assert book.lookup_state_id(999) is None
    assert book.lookup_state_index(999) is None
    assert book.child_entry(root, 99) is None


def test_sbk_book_open_with_control_cancel(tmp_path):
    """Test SBK open cancellation from a Python callback."""
    path = tmp_path / "book.sbk"
    path.write_bytes(
        _field_message(
            3,
            _field_varint(1, 0)
            + _field_string(
                7,
                "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
            ),
        )
    )

    try:
        rs.book.SbkBook.open_with_control(str(path), lambda step: False)
    except ValueError as exc:
        assert "cancelled" in str(exc)
    else:
        raise AssertionError("expected cancellation error")


def test_removed_v2_search_api_is_not_exported():
    """Version 2 requires explicit migration to native v3 instead of legacy aliases."""
    for name in (
        "BookKey", "BookMove", "BookEntry", "MemoryBook", "StaticBook", "BookBuilder",
        "book_key_from_position", "book_key_after", "book_builder_from_game_record",
    ):
        assert not hasattr(rs.book, name)


def _field_varint(field: int, value: int) -> bytes:
    return _varint(field << 3) + _varint(value)


def _field_string(field: int, value: str) -> bytes:
    return _field_message(field, value.encode("utf-8"))


def _field_message(field: int, value: bytes) -> bytes:
    return _varint((field << 3) | 2) + _varint(len(value)) + value


def _varint(value: int) -> bytes:
    data = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            byte |= 0x80
        data.append(byte)
        if not value:
            return bytes(data)
