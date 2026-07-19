from rsshogi.core import Board, Move


def main() -> None:
    m16 = Move.from_usi("7g7f")
    print(m16.to_usi(), int(m16))
    board = Board()
    board.apply_move(m16)
    print(board.to_sfen())


if __name__ == "__main__":
    main()
