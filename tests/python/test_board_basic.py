from __future__ import annotations

import pytest

rsshogi = pytest.importorskip("rsshogi")
core_mod = pytest.importorskip("rsshogi.core")
types_mod = pytest.importorskip("rsshogi.types")

Board = core_mod.Board
Move = core_mod.Move
AperyMove = core_mod.AperyMove
Move32 = core_mod.Move32
AperyMove32 = core_mod.AperyMove32
PositionState = core_mod.PositionState
to_move_util = core_mod.to_move
parse_usi_position = core_mod.parse_usi_position
parse_usi_position_parts = core_mod.parse_usi_position_parts
normalize_usi_position = core_mod.normalize_usi_position
Color = types_mod.Color
Square = types_mod.Square
PieceType = types_mod.PieceType
Piece = types_mod.Piece
Bitboard = types_mod.Bitboard
MoveType = types_mod.MoveType


def test_sfen_roundtrip_startpos() -> None:
    board = Board()
    sfen = board.to_sfen()
    clone = Board(sfen)
    assert clone.to_sfen() == sfen


def test_from_usi_constructors_for_types() -> None:
    assert Color.from_usi("b") == Color.BLACK
    assert PieceType.from_usi("P") == PieceType.PAWN
    assert Piece.from_usi("P").piece_type == PieceType.PAWN


def test_core_reexports_common_types() -> None:
    assert core_mod.Color is types_mod.Color
    assert core_mod.Square is types_mod.Square
    assert core_mod.Piece is types_mod.Piece
    assert core_mod.PieceType is types_mod.PieceType


def test_color_opponent_alias() -> None:
    assert Color.BLACK.opponent() == Color.WHITE
    assert Color.WHITE.opponent() == Color.BLACK
    assert Color.BLACK.opponent() == Color.BLACK.flip()


def test_square_from_file_rank_is_zero_based() -> None:
    sq = Square.from_usi("7g")
    assert sq.file == 6
    assert sq.rank == 6
    assert Square.from_file_rank(sq.file, sq.rank) == sq
    assert Square.from_file_rank(0, 0).to_usi() == "1a"
    assert Square.from_file_rank(8, 8).to_usi() == "9i"

    with pytest.raises(ValueError, match="0..8"):
        Square.from_file_rank(9, 0)
    with pytest.raises(ValueError, match="0..8"):
        Square.from_file_rank(0, -1)


def test_psfen_roundtrip_startpos() -> None:
    board = Board()
    psfen = board.to_packed_sfen()
    assert isinstance(psfen, (bytes, bytearray))
    assert len(psfen) == 32

    clone = Board()
    clone.set_packed_sfen(psfen)
    assert clone.to_sfen().split()[:3] == board.to_sfen().split()[:3]


def test_parse_usi_position_parts_splits_initial_and_moves() -> None:
    parts = parse_usi_position_parts("position startpos moves 7g7f 3c3d")

    assert parts.initial_sfen == Board().to_sfen()
    assert [move.to_usi() for move in parts.moves] == ["7g7f", "3c3d"]
    assert parts.move_usi == ["7g7f", "3c3d"]
    assert parts.final_sfen == parse_usi_position("position startpos moves 7g7f 3c3d").to_sfen()
    assert parts.to_dict()["move_usi"] == ["7g7f", "3c3d"]


def test_board_declaration_delta_and_mate_smoke_apis() -> None:
    board = Board()

    declaration = board.evaluate_declaration()
    assert declaration["can_declare"] is False
    assert "detail_type" in declaration

    delta = board.push_usi_with_delta("7g7f")
    assert delta["kind"] == "BOARD"
    assert delta["move_usi"] == "7g7f"
    assert delta["from"].to_usi() == "7g"
    assert delta["to"].to_usi() == "7f"
    assert board.to_sfen() == parse_usi_position("position startpos moves 7g7f").to_sfen()

    assert Board().solve_mate_in_one() is None


def test_hcp_roundtrip_startpos() -> None:
    board = Board()
    hcp = board.to_hcp()
    assert isinstance(hcp, (bytes, bytearray))
    assert len(hcp) == 32

    clone = Board()
    clone.set_hcp(hcp)
    assert clone.to_sfen().split()[:3] == board.to_sfen().split()[:3]


def test_board_iter_pieces_startpos() -> None:
    pieces = Board().iter_pieces()

    assert len(pieces) == 40
    assert pieces[0][0].to_usi() == "1a"
    assert pieces[0][1].piece_type == PieceType.LANCE
    assert pieces[-1][0].to_usi() == "9i"
    assert pieces[-1][1].piece_type == PieceType.LANCE
    assert all(piece != Piece(0) for _, piece in pieces)


def test_board_hand_counts_returns_all_usi_keys() -> None:
    board = Board("4k4/9/9/9/9/9/9/9/4K4 b 2PBG 1")

    assert board.hand_counts(Color.BLACK) == {
        "P": 2,
        "L": 0,
        "N": 0,
        "S": 0,
        "B": 1,
        "R": 0,
        "G": 1,
    }
    assert board.hand_counts(Color.WHITE) == {
        "P": 0,
        "L": 0,
        "N": 0,
        "S": 0,
        "B": 0,
        "R": 0,
        "G": 0,
    }


def test_apery_move_conversions() -> None:
    mv = Move.from_usi("7g7f")
    apery = mv.to_apery()
    assert isinstance(apery, AperyMove)
    assert apery.to_move() == mv
    assert apery.to_usi() == "7g7f"

    board = Board()
    mv32 = board.move32_from_move(mv)
    apery32 = board.apery_move32_from_move32(mv32)
    assert isinstance(apery32, AperyMove32)
    assert apery32.to_move() == mv
    assert apery32.to_move32(board) == mv32

    drop = Move.from_usi("P*5e").to_apery()
    assert drop.from_sq == Square.NONE

    drop32 = board.apery_move32_from_move(Move.from_usi("P*5e"))
    assert drop32.from_sq == Square.NONE
    assert drop32.piece_type_before == PieceType.NO_PIECE_TYPE
    assert drop32.piece_type_after == PieceType.PAWN


def test_position_state_roundtrip_and_edit() -> None:
    board = Board()
    state = board.to_position_state()

    assert isinstance(state, PositionState)
    assert state.to_sfen() == board.to_sfen()
    assert state.side_to_move == Color.BLACK
    assert state.ply == 1
    assert len(state.board) == 81
    assert state.hands[0].is_empty()
    assert state.piece_on(Square.from_usi("7g")).piece_type == PieceType.PAWN

    edited = state.copy()
    edited.set_piece(Square.from_usi("7g"), Piece(0))
    edited.set_piece(
        Square.from_usi("7f"),
        Piece.from_color_type(Color.BLACK, PieceType.PAWN),
    )
    edited.side_to_move = Color.WHITE
    edited.ply = 42

    clone = Board()
    clone.set_position_state(edited)
    assert clone.to_sfen() == edited.to_sfen()
    assert clone.turn == Color.WHITE
    assert clone.game_ply == 42
    assert clone.piece_on(Square.from_usi("7f")).piece_type == PieceType.PAWN
    assert clone.piece_on(Square.from_usi("7g")) == Piece(0)


def test_position_state_validate_all_and_board_validate() -> None:
    state = Board().to_position_state()
    state.set_piece(Square.from_usi("5a"), Piece(0))
    state.set_piece(
        Square.from_usi("7f"),
        Piece.from_color_type(Color.BLACK, PieceType.PAWN),
    )

    report = state.validate_all()
    assert not report.is_valid()
    assert not state.is_valid()

    kinds = {issue.kind for issue in report.issues}
    assert kinds >= {"NO_KING", "DOUBLE_PAWN"}

    double_pawn = next(issue for issue in report.issues if issue.kind == "DOUBLE_PAWN")
    assert double_pawn.color == Color.BLACK
    assert double_pawn.file_usi == "7"
    assert double_pawn.piece_type is None
    assert "double pawn" in double_pawn.message

    no_king = next(issue for issue in report.issues if issue.kind == "NO_KING")
    assert no_king.color == Color.WHITE
    assert no_king.square is None

    board = Board()
    board.set_position_state(state)
    with pytest.raises(ValueError, match="double pawn"):
        board.validate()
    assert not board.is_valid()
    assert board.validate_all() == report


def test_position_state_and_validation_objects_compare_false_to_other_types() -> None:
    state = Board().to_position_state()
    report = state.validate_all()
    issue = report.issues[0] if report.issues else None

    assert (state == None) is False  # noqa: E711
    assert (state != None) is True  # noqa: E711
    assert (report == None) is False  # noqa: E711
    assert (report != None) is True  # noqa: E711
    if issue is not None:
        assert (issue == None) is False  # noqa: E711
        assert (issue != None) is True  # noqa: E711


def test_copy_and_reset() -> None:
    board = Board()
    board.apply_move(Move.from_usi("7g7f"))
    board.apply_move(Move.from_usi("3c3d"))

    clone = board.copy()
    assert clone.to_sfen() == board.to_sfen()

    clone.apply_move(Move.from_usi("2g2f"))
    assert clone.to_sfen() != board.to_sfen()

    board.reset()
    assert board.to_sfen() == Board().to_sfen()
    assert board.last_move() is None


def test_legal_moves_nonempty() -> None:
    board = Board()
    moves = board.legal_moves()
    assert moves, "legal_moves should not be empty in startpos"


def test_board_accessors_and_attacks() -> None:
    board = Board()

    # hand / color helpers
    assert board.turn == Color.BLACK
    assert board.game_ply == board.game_ply

    king_sq = Square.from_usi("5i")
    assert board.king_square(Color.BLACK) == king_sq

    pawn_sq = Square.from_usi("7g")
    pawn = board.piece_on(pawn_sq)
    assert pawn.piece_type == PieceType.PAWN
    assert not board.is_square_empty(pawn_sq)

    empties = board.empties()
    assert isinstance(empties, Bitboard)
    assert empties.count() > 0

    blacks = board.pieces_by_color(Color.BLACK)
    assert blacks.count() > 0

    pawns = board.pieces_by_type(PieceType.PAWN)
    assert pawns.count() == 18

    golds = board.golds()
    assert golds.count() > 0

    assert board.hand(Color.BLACK).is_empty()

    occupied = board.pieces()
    attackers = board.attackers_to(king_sq, occupied)
    assert isinstance(attackers, Bitboard)

    attackers_color = board.attackers_to_color(Color.WHITE, king_sq, occupied)
    assert isinstance(attackers_color, Bitboard)

    attackers_current = board.attackers_to_color_current(Color.WHITE, king_sq)
    assert isinstance(attackers_current, Bitboard)

    assert not board.is_attacked_by(king_sq, Color.WHITE)

    pawn_attacks = board.attacks_by(Color.BLACK, PieceType.PAWN)
    assert isinstance(pawn_attacks, Bitboard)
    assert pawn_attacks.count() > 0

    assert board.repetition_state().to_usi() == "rep_none"


def test_move_enhancements() -> None:
    board = Board()
    move32 = board.legal_moves_move32()[0]
    assert move32.from_sq.to_usi() != ""
    assert move32.to_sq.to_usi() != ""
    assert move32.move_type in (
        MoveType.NORMAL,
        MoveType.PROMOTION,
        MoveType.DROP,
    )
    assert move32.to_move().to_usi() == move32.to_usi()
    assert move32.to_csa() is not None
    assert move32.dropped_piece_type is None
    _ = move32.piece_after_move

    mv = Move.from_usi("7g7f")
    assert mv.from_sq.to_usi() == "7g"
    assert mv.to_sq.to_usi() == "7f"
    assert mv.move_type == MoveType.NORMAL
    assert mv.dropped_piece_type is None

    drop16 = Move.from_usi("P*5e")
    assert drop16.move_type == MoveType.DROP
    assert drop16.dropped_piece_type == PieceType.PAWN


def test_str_for_value_types() -> None:
    board = Board()

    piece = board.piece_on(Square.from_usi("7g"))
    assert str(piece) == "P"

    bb = board.pieces_by_type(PieceType.PAWN)
    assert isinstance(str(bb), str)
    assert len(str(bb)) > 0

    hand = board.hand(Color.BLACK)
    assert str(hand).startswith("0x")

    assert str(MoveType.NORMAL) == "normal"
    assert str(board.repetition_state()) == "rep_none"


def test_move_special_constants() -> None:
    assert int(Move.MOVE_NONE) == 0
    assert Move.MOVE_NONE.to_usi() == "none"
    assert Move.MOVE_NULL.to_usi() == "null"
    assert Move.MOVE_RESIGN.to_usi() == "resign"
    assert Move.MOVE_WIN.to_usi() == "win"
    assert not Move.MOVE_END.is_normal()
    assert not Move.MOVE_NONE.is_normal()
    assert not Move.MOVE_NULL.is_normal()
    assert not Move.MOVE_RESIGN.is_normal()
    assert not Move.MOVE_WIN.is_normal()
    assert int(Move.MOVE_END) == (4 << 7) + 4

    assert int(Move.MOVE_NONE) == 0
    assert Move.MOVE_NONE.to_usi() == "none"
    assert Move.MOVE_NULL.to_usi() == "null"
    assert Move.MOVE_RESIGN.to_usi() == "resign"
    assert Move.MOVE_WIN.to_usi() == "win"
    assert not Move.MOVE_END.is_normal()
    assert int(Move.MOVE_END) == (4 << 7) + 4


def test_game_record_from_ki2() -> None:
    ki2 = """
手合割：平手
▲７六歩
まで1手で先手の勝ち
"""
    record = rsshogi.record.Record.from_ki2_str(ki2)
    assert record.move_count == 1
    assert record.moves[0].move.to_usi() == "7g7f"


def test_apply_usi_and_csa() -> None:
    board = Board()
    board.apply_usi("7g7f")
    board.apply_csa("3334FU")
    assert board.game_ply == 3


def test_set_usi_position_with_moves() -> None:
    board = Board()
    board.set_usi_position("position startpos moves 7g7f 3c3d")
    assert board.to_sfen() == "lnsgkgsnl/1r5b1/pppppp1pp/6p2/9/2P6/PP1PPPPPP/1B5R1/LNSGKGSNL b - 3"


def test_parse_and_normalize_usi_position() -> None:
    board = parse_usi_position("startpos moves 7g7f")
    assert isinstance(board, Board)
    assert board.to_sfen() == "lnsgkgsnl/1r5b1/ppppppppp/9/9/2P6/PP1PPPPPP/1B5R1/LNSGKGSNL w - 2"

    assert normalize_usi_position("position startpos") == "startpos"
    assert (
        normalize_usi_position("position startpos moves 7g7f")
        == "lnsgkgsnl/1r5b1/ppppppppp/9/9/2P6/PP1PPPPPP/1B5R1/LNSGKGSNL w - 2"
    )


def test_push_pop_peek_and_push_usi() -> None:
    board = Board()
    assert board.last_move() is None

    pushed = board.push_usi("7g7f")
    assert isinstance(pushed, Move32)
    assert board.last_move().to_usi() == "7g7f"

    pushed2 = board.push_move(Move.from_usi("3c3d"))
    assert pushed2.to_usi() == "3c3d"
    assert board.game_ply == 3

    popped = board.pop()
    assert popped is not None
    assert popped.to_usi() == "3c3d"
    assert board.last_move().to_usi() == "7g7f"

    popped2 = board.pop()
    assert popped2 is not None
    assert popped2.to_usi() == "7g7f"
    assert board.pop() is None


def test_move_recovery_helpers() -> None:
    board = Board()
    mv16 = Move.from_usi("7g7f")

    recovered = board.move32_from_move(mv16)
    assert recovered.to_usi() == "7g7f"
    assert recovered.to_csa() == "7776FU"

    from_csa = board.move_from_csa("7776FU")
    assert from_csa.to_usi() == "7g7f"


def test_move32_to_csa() -> None:
    board = Board()
    mv = Move.from_usi("7g7f")
    mv32 = board.move32_from_move(mv)
    assert mv32.to_csa() == "7776FU"

    board.apply_usi("7g7f 3c3d 8h2b+")
    drop = Move.from_usi("B*3c")
    drop32 = board.move32_from_move(drop)
    assert drop32.to_csa() == "0033KA"


def test_zobrist_hash_and_declare_win_api() -> None:
    board = Board()
    start_hash = board.zobrist_hash()
    assert isinstance(start_hash, int)
    assert not board.can_declare_win()

    board.apply_usi("7g7f")
    assert board.zobrist_hash() != start_hash

    board.undo_move32(board.last_move())
    assert board.zobrist_hash() == start_hash


def test_to_move_utility() -> None:
    mv16 = Move.from_usi("7g7f")
    assert int(to_move_util(mv16)) == int(mv16)

    mv32 = Move32.from_usi("7g7f")
    assert int(to_move_util(mv32)) == int(mv16)
    assert int(to_move_util(int(mv32))) == int(mv16)

    with pytest.raises(ValueError, match="uint16"):
        to_move_util(int(mv32) + 0x1_0000)
    with pytest.raises(ValueError, match="uint16"):
        to_move_util(-1)


def test_psfen_output_buffer_variants() -> None:
    board = Board()

    out = bytearray(32)
    ret = board.to_packed_sfen(out)
    assert ret is None
    assert bytes(out) == board.to_packed_sfen()

    np = pytest.importorskip("numpy")
    arr = np.zeros(32, dtype=np.uint8)
    ret = board.to_packed_sfen(arr)
    assert ret is None
    assert bytes(arr.tolist()) == board.to_packed_sfen()

    packed = np.zeros(1, dtype=[("sfen", np.uint8, 32)])
    ret = board.to_packed_sfen(packed)
    assert ret is None
    assert packed[0]["sfen"].tobytes() == board.to_packed_sfen()


def test_set_psfen_accepts_packed_sfen_ndarray() -> None:
    np = pytest.importorskip("numpy")
    board = Board()
    raw = board.to_packed_sfen()
    packed = np.zeros(1, dtype=[("sfen", np.uint8, 32)])
    packed[0]["sfen"] = np.frombuffer(raw, dtype=np.uint8)

    clone = Board()
    clone.set_packed_sfen(packed)
    assert clone.to_sfen().split()[:3] == board.to_sfen().split()[:3]


def test_hcp_output_variants() -> None:
    board = Board()
    raw = board.to_hcp()
    assert isinstance(raw, (bytes, bytearray))
    assert len(raw) == 32

    out = bytearray(32)
    ret = board.to_hcp(out)
    assert ret is None
    assert bytes(out) == raw

    np = pytest.importorskip("numpy")
    arr = np.zeros(1, dtype=rsshogi.numpy.HuffmanCodedPos)
    ret = board.to_hcp(arr)
    assert ret is None
    assert arr[0]["hcp"].tobytes() == raw

    clone = Board()
    clone.set_hcp(arr)
    assert clone.to_sfen().split()[:3] == board.to_sfen().split()[:3]


def test_is_legal_accepts_move_and_move32() -> None:
    board = Board()
    legal = Move.from_usi("7g7f")
    illegal = Move.from_usi("7g7e")

    assert board.is_legal_move(legal)
    assert not board.is_legal_move(illegal)

    legal32 = board.move32_from_move(legal)
    assert isinstance(legal32, Move32)
    assert board.is_legal_move32(legal32)


def test_to_psv_output_variants() -> None:
    board = Board()
    move = Move.from_usi("7g7f")

    packed = board.to_psv(mv=move, score=123, game_result="BLACK_WIN", game_ply=1)
    assert isinstance(packed, (bytes, bytearray))
    assert len(packed) == 40
    assert int.from_bytes(packed[32:34], "little", signed=True) == 123
    assert int.from_bytes(packed[34:36], "little") == int(move)
    assert int.from_bytes(packed[36:38], "little") == 1
    assert int.from_bytes(packed[38:39], "little", signed=True) == int(rsshogi.record.GameResult.BLACK_WIN)

    out = bytearray(40)
    ret = board.to_psv(
        mv=move,
        score=-50,
        game_result="WHITE_WIN",
        game_ply=7,
        out=out,
    )
    assert ret is None
    assert int.from_bytes(out[32:34], "little", signed=True) == -50
    assert int.from_bytes(out[34:36], "little") == int(move)
    assert int.from_bytes(out[36:38], "little") == 7
    assert int.from_bytes(out[38:39], "little", signed=True) == int(rsshogi.record.GameResult.WHITE_WIN)

    np = pytest.importorskip("numpy")
    arr = np.zeros(1, dtype=rsshogi.numpy.PackedSfenValue)
    ret = board.to_psv(
        mv=move,
        score=10,
        game_result="DRAW_BY_REPETITION",
        game_ply=8,
        out=arr,
    )
    assert ret is None
    assert arr[0]["score"] == 10
    assert arr[0]["move"] == int(move)
    assert arr[0]["game_ply"] == 8
    assert arr[0]["game_result"] == int(rsshogi.record.GameResult.DRAW_BY_REPETITION)


def test_to_hcpe_output_variants() -> None:
    board = Board()
    move = Move.from_usi("7g7f")

    packed = board.to_hcpe(best_move=move, score=123, game_result="BLACK_WIN")
    assert isinstance(packed, (bytes, bytearray))
    assert len(packed) == 38
    assert int.from_bytes(packed[32:34], "little", signed=True) == 123
    assert int.from_bytes(packed[34:36], "little") == int(move.to_apery())
    assert int.from_bytes(packed[36:37], "little", signed=True) == 1

    out = bytearray(38)
    ret = board.to_hcpe(best_move=move, score=-5, game_result="WHITE_WIN", out=out)
    assert ret is None
    assert int.from_bytes(out[32:34], "little", signed=True) == -5
    assert int.from_bytes(out[34:36], "little") == int(move.to_apery())
    assert int.from_bytes(out[36:37], "little", signed=True) == 2

    np = pytest.importorskip("numpy")
    arr = np.zeros(1, dtype=rsshogi.numpy.HuffmanCodedPosAndEval)
    ret = board.to_hcpe(best_move=move, score=7, game_result="DRAW", out=arr)
    assert ret is None
    assert arr[0]["eval"] == 7
    assert arr[0]["bestMove16"] == int(move.to_apery())
    assert arr[0]["gameResult"] == 0


def test_to_hcpe_requires_game_result_and_rejects_out_of_range_move() -> None:
    board = Board()

    with pytest.raises(ValueError, match="game_result is required"):
        board.to_hcpe(best_move="7g7f")

    with pytest.raises(ValueError, match="uint16"):
        board.to_hcpe(best_move=0x1_0000, score=1, game_result="BLACK_WIN")


def test_serialize_sbinpack_requires_eval() -> None:
    kif = """
手合割：平手
手数----指手---------消費時間--
 1 ７六歩(77)
 2 投了
"""
    record = rsshogi.record.Record.from_kif_str(kif)
    with pytest.raises(ValueError, match="MissingEval|missing eval"):
        record.to_sbinpack()
