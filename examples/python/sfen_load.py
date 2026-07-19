from rsshogi.core import Board


def main() -> None:
    sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"
    board = Board(sfen=sfen)
    print(board.to_sfen())


if __name__ == "__main__":
    main()
