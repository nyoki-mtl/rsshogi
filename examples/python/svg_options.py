"""Write white and gray board samples with only the last piece in bold."""

from pathlib import Path

from rsshogi.core import Board, Move32


def main() -> None:
    board = Board()
    for usi in ["7g7f", "3c3d"]:
        board.apply_usi(usi)
    sfen = board.to_sfen()
    last_move_value = board.last_move().value
    restored = Board(sfen)
    output = Path("target/svg_samples")
    output.mkdir(parents=True, exist_ok=True)
    for name, background in [("normal", "#ffffff"), ("gray", "#efefef")]:
        svg = restored.to_svg(
            Move32(last_move_value),
            scale=3.0,
            highlight_squares=False,
            bold_destination=True,
            board_background=background,
        )
        (output / f"{name}.svg").write_text(str(svg), encoding="utf-8")


if __name__ == "__main__":
    main()
