"""Tests for rsshogi.book module."""

import rsshogi as rs


def test_book_key_from_position():
    """Test book_key_from_position function."""
    board = rs.core.Board()
    key = rs.book.book_key_from_position(board)

    assert key is not None
    assert isinstance(key, rs.book.BookKey)
    assert key.low != 0
    assert key.high >= 0


def test_book_key_after():
    """Test book_key_after function."""
    board = rs.core.Board()
    mv = rs.core.Move.from_usi("7g7f")

    key_after = rs.book.book_key_after(board, mv)
    board.apply_usi("7g7f")
    key_current = rs.book.book_key_from_position(board)

    assert key_after.low == key_current.low
    assert key_after.high == key_current.high


def test_memory_book_basic():
    """Test MemoryBook basic operations."""
    book = rs.book.MemoryBook()
    assert len(book) == 0

    board = rs.core.Board()
    key = rs.book.book_key_from_position(board)

    # Initially no entry
    assert book.get(key) is None
    assert not book.contains(key)

    # Insert a move
    mv = rs.core.Move.from_usi("7g7f")
    book_move = rs.book.BookMove(mv, score=100, depth=10)
    book.insert_move(key, book_move)

    # Now entry should exist
    assert book.contains(key)
    entry = book.get(key)
    assert entry is not None
    assert len(entry.moves) == 1
    assert entry.moves[0].score == 100
    assert entry.moves[0].depth == 10
    assert str(entry.moves[0]) == "7g7f"


def test_static_book_from_memory():
    """Test StaticBook.from_memory_book."""
    memory_book = rs.book.MemoryBook()
    static_book = rs.book.StaticBook.from_memory_book(memory_book)

    assert len(static_book) == 0


def test_static_book_roundtrip():
    """Test StaticBook serialization roundtrip."""
    memory_book = rs.book.MemoryBook()
    static_book = rs.book.StaticBook.from_memory_book(memory_book)

    # Serialize and deserialize
    data = static_book.to_bytes()
    restored = rs.book.StaticBook.from_bytes(data)

    assert len(restored) == len(static_book)


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


def test_book_key_equality():
    """Test BookKey equality comparison."""
    board = rs.core.Board()
    key1 = rs.book.book_key_from_position(board)
    key2 = rs.book.book_key_from_position(board)

    assert key1 == key2
    assert not (key1 != key2)


def test_book_key_hash():
    """Test BookKey is hashable."""
    board = rs.core.Board()
    key = rs.book.book_key_from_position(board)

    # Should be able to use as dict key
    d = {key: "value"}
    assert d[key] == "value"


def test_book_key_repr():
    """Test BookKey string representation."""
    board = rs.core.Board()
    key = rs.book.book_key_from_position(board)

    repr_str = repr(key)
    assert "BookKey" in repr_str
    assert "low=" in repr_str

    str_str = str(key)
    assert "0x" in str_str

    expected = (key.high << 64) | key.low
    assert int(key) == expected


def test_book_builder_from_game_record():
    """Test book_builder_from_game_record function."""
    kif = """\
手合割：平手
手数----指手---------消費時間--
 1 ７六歩(77)
 2 ３四歩(33)
 3 ２六歩(27)
 4 投了
まで3手で先手の勝ち
"""
    record = rs.record.Record.from_kif_str(kif)
    memory_book = rs.book.book_builder_from_game_record(record, default_score=0, default_depth=1)

    assert isinstance(memory_book, rs.book.MemoryBook)
    # Should have at least the initial position
    assert len(memory_book) >= 1


def test_book_builder_class():
    """Test BookBuilder class."""
    builder = rs.book.BookBuilder()

    board = rs.core.Board()
    mv = rs.core.Move.from_usi("7g7f")

    # Test insert_for_position
    builder.insert_for_position(board, mv, score=100, depth=10)
    memory_book = builder.into_memory()
    assert len(memory_book) >= 1

    # Test extend_from_game_record
    kif = """\
手合割：平手
手数----指手---------消費時間--
 1 ７六歩(77)
 2 投了
まで1手で先手の勝ち
"""
    record = rs.record.Record.from_kif_str(kif)
    builder = rs.book.BookBuilder()
    builder.extend_from_game_record(record, default_score=0, default_depth=1)
    static_book = builder.build_static()
    assert len(static_book) >= 1


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
