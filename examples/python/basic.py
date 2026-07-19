from rsshogi.core import Board


def main() -> None:
    board = Board()
    print(board.to_sfen())
    board.apply_usi("7g7f")
    print(board.to_sfen())


if __name__ == "__main__":
    main()
