from rsshogi.core import Board


def main() -> None:
    board = Board()
    board.apply_usi("7g7f")
    board.apply_usi("3c3d")
    print("after moves:", board.to_sfen())
    last_move = board.last_move()
    if last_move:
        board.undo_move(last_move)
        print("undone:", last_move.to_usi())
    print("after undo:", board.to_sfen())


if __name__ == "__main__":
    main()
