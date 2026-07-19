"""Demonstrate structured imports from rsshogi submodules."""

# Core primitives
from rsshogi.core import Board, Move

# Records
from rsshogi.record import GameResult, Record

# Types and constants
from rsshogi.types import Color, PieceType, Square


def main() -> None:
    print("=== Structured Import Demo ===\n")

    # Create a board
    print("1. Board creation and basic operations:")
    board = Board()
    print(f"   Initial position: {board.to_sfen()}")
    print(f"   Side to move: {board.turn}")
    print(f"   Is BLACK? {board.turn == Color.BLACK}")

    # Make moves using structured imports
    print("\n2. Making moves:")
    move = Move.from_usi("7g7f")
    board.apply_move(move)
    print(f"   After 7g7f: {board.to_sfen()}")

    # Check piece types
    print("\n3. Piece inspection:")
    sq = Square.from_usi("7f")
    piece = board.piece_on(sq)
    print(f"   Piece at 7f: {piece}")
    print(f"   Is PAWN? {piece.piece_type == PieceType.PAWN}")

    # Parse KIF
    print("\n4. Parsing KIF:")
    kif_text = """
手合割：平手
手数----指手---------消費時間--
 1 ７六歩(77)
 2 ３四歩(33)
 3 投了
まで2手で後手の勝ち
"""
    record = Record.from_kif_str(kif_text)
    print(f"   Moves in record: {record.move_count}")
    print(f"   Result: {record.result}")
    print(f"   Is WHITE_WIN? {record.result == GameResult.WHITE_WIN}")

    # Export
    print("\n5. Exporting KIF:")
    exported = record.to_kif()
    print(f"   Exported lines: {len(exported.splitlines())}")

    print("\n=== All structured imports working! ===")


if __name__ == "__main__":
    main()
