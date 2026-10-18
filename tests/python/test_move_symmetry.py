"""Check that the bindings' flip and mirror operations agree with the flipped board."""

from rsshogi.core import Board, Move, Move32

POSITIONS = [
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
    "l4S2l/4g1gs1/5p1p1/pr2N1pkp/4Gn3/PP3PPPP/2GPP4/1K7/L3r+s2L w BS2N5Pb 1",
    "6n1l/2+S1k4/2lp4p/1np1B2b1/3PP4/1N1S3rP/1P2+pPP+p1/1p1G5/3KG2r1 b GSN2L4Pgs2p 1",
    "l6nl/5+P1gk/2np1S3/p1p4Pp/3P2Sp1/1PPb2P1P/P5GS1/R8/LN4bKL w RGgsn5p 1",
    "R8/2K1S1SSk/4B4/9/9/9/9/9/1L1L1L3 b RBGSNLP3g3n17p 1",
    "1n2g1k1p/2r2p2P/l+B1pp1p1n/p6P1/LN3g2L/3PG1Pp1/S3GS+s2/1P5R1/+b1KS3N1 b L3P3p 155",
]

# 平手は飛角があるため左右非対称。鏡像の対応を見るには対称な局面を使う。
FILE_SYMMETRIC_SFEN = "lnsgkgsnl/9/ppppppppp/9/9/9/PPPPPPPPP/9/LNSGKGSNL b - 1"


def _usi_set(moves):
    return sorted(mv.to_usi() for mv in moves)


def test_flipping_every_legal_move_matches_the_flipped_position():
    for sfen in POSITIONS:
        board = Board(sfen)
        flipped = Board(board.to_sfen_flipped())
        legal = board.legal_moves_move32()
        assert _usi_set(mv.flip_side() for mv in legal) == _usi_set(flipped.legal_moves_move32())
        for mv in legal:
            assert mv.flip_side().flip_side() == mv
            assert mv.flip_side().piece_after_move.color == mv.piece_after_move.color.flip()
            assert mv.flip_side().piece_after_move.piece_type == mv.piece_after_move.piece_type


def test_mirroring_permutes_the_legal_moves_of_a_symmetric_position():
    board = Board(FILE_SYMMETRIC_SFEN)
    legal = board.legal_moves_move32()
    mirrored = []
    for mv in legal:
        moved = mv.mirror_file()
        assert moved.mirror_file() == mv
        assert moved.piece_after_move == mv.piece_after_move
        mirrored.append(moved)
    assert _usi_set(mirrored) == _usi_set(legal)


def test_move_flip_side_and_mirror_file_keep_the_move_kind():
    normal = Move.from_usi("7g7f")
    assert normal.flip_side().to_usi() == "3c3d"
    assert normal.mirror_file().to_usi() == "3g3f"
    assert normal.flip_side().flip_side() == normal
    assert normal.mirror_file().mirror_file() == normal

    promotion = Move.from_usi("8h2b+")
    assert promotion.flip_side().to_usi() == "2b8h+"
    assert promotion.mirror_file().to_usi() == "2h8b+"
    assert promotion.flip_side().is_promotion()
    assert promotion.mirror_file().is_promotion()

    drop = Move.from_usi("P*7f")
    assert drop.flip_side().to_usi() == "P*3d"
    assert drop.mirror_file().to_usi() == "P*3f"
    assert drop.flip_side().is_drop()
    assert drop.flip_side().dropped_piece_type == drop.dropped_piece_type


def test_move32_without_piece_info_flips_only_the_squares():
    mv = Move32.from_usi("7g7f")
    assert mv.flip_side().to_usi() == "3c3d"
    assert mv.mirror_file().to_usi() == "3g3f"
    assert mv.flip_side().flip_side() == mv


def test_non_normal_moves_pass_through_unchanged():
    sentinels = [Move.MOVE_NONE, Move.MOVE_NULL, Move.MOVE_RESIGN, Move.MOVE_WIN, Move.MOVE_END]
    sentinels += [Move32.MOVE_NONE, Move32.MOVE_NULL, Move32.MOVE_RESIGN, Move32.MOVE_WIN, Move32.MOVE_END]
    for sentinel in sentinels:
        assert sentinel.flip_side() == sentinel
        assert sentinel.mirror_file() == sentinel


def test_to_packed_sfen_flipped_matches_the_flipped_board():
    for sfen in POSITIONS:
        board = Board(sfen)
        expected = Board(board.to_sfen_flipped()).to_packed_sfen()
        assert board.to_packed_sfen_flipped() == expected
        out = bytearray(32)
        assert board.to_packed_sfen_flipped(out) is None
        assert bytes(out) == expected
