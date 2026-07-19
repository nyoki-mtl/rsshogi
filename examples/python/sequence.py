from rsshogi.core import Board


def main() -> None:
    board = Board()
    for mv in ["7g7f", "3c3d", "2g2f", "8c8d"]:
        board.apply_usi(mv)
    print(board.to_sfen())


if __name__ == "__main__":
    main()
