"""SVG options share the Rust renderer and preserve board geometry."""

import xml.etree.ElementTree as ET

import pytest

from rsshogi.core import Board, Move, Move32

NS = {"s": "http://www.w3.org/2000/svg"}
HREF = "{http://www.w3.org/1999/xlink}href"


@pytest.mark.parametrize(
    ("sfen", "moves", "piece"),
    [
        (None, ["7g7f"], "black-pawn"),
        (None, ["7g7f", "3c3d"], "white-pawn"),
        (None, ["7g7f", "3c3d", "8h2b+"], "black-horse"),
        (None, ["7g7f", "3c3d", "8h2b+", "3a2b", "B*4e"], "black-bishop"),
        (None, ["7g7f", "3c3d", "8h2b+", "3a2b", "B*4e", "B*5e"], "white-bishop"),
        ("4k4/9/9/9/9/9/4p4/9/4K4 w - 1", ["5g5h+"], "white-pro-pawn"),
        ("4k4/9/3S5/9/9/9/9/9/4K4 b - 1", ["6c6b+"], "black-pro-silver"),
        ("4k4/9/9/9/9/9/3s5/9/4K4 w - 1", ["6g6h+"], "white-pro-silver"),
    ],
)
def test_destination_only_and_sfen_roundtrip(sfen, moves, piece):
    board = Board(sfen)
    for usi in moves:
        board.apply_usi(usi)
    last = board.last_move()
    original = str(board.to_svg(last, 1.0))
    assert original == str(board.to_svg(last, highlight_squares=True, bold_destination=False))
    bold = str(board.to_svg(last, bold_destination=True))
    assert bold.replace(' font-weight="bold"', "") == original
    root = ET.fromstring(bold)
    weighted = root.findall(".//*[@font-weight]")
    assert len(weighted) == 1
    assert weighted[0].tag == f"{{{NS['s']}}}use"
    assert weighted[0].get(HREF) == f"#{piece}"
    restored = Board(board.to_sfen())
    assert str(restored.to_svg(Move32(last.value), bold_destination=True)) == bold


@pytest.mark.parametrize("highlight", [False, True])
@pytest.mark.parametrize("bold", [False, True])
@pytest.mark.parametrize("background", [None, "#ffffff", "#efefef"])
def test_independent_options_and_paint_order(highlight, bold, background):
    board = Board()
    board.apply_usi("7g7f")
    svg = str(
        board.to_svg(board.last_move(), highlight_squares=highlight, bold_destination=bold, board_background=background)
    )
    root = ET.fromstring(svg)
    rects = root.findall("s:rect", NS)
    assert len(rects) == (background is not None) + 2 * highlight
    assert ("#f6b94d" in svg) == highlight
    assert ("font-weight" in svg) == bold
    assert root.get("viewBox") == "0 0 230 192"
    if background is not None:
        assert rects[0].attrib == {"x": "20.5", "y": "10.5", "width": "180", "height": "180", "fill": background}
        assert list(root).index(rects[-1]) < next(i for i, node in enumerate(root) if node.get("stroke") == "black")


@pytest.mark.parametrize("move_type", [Move, Move32])
def test_special_moves_and_missing_history(move_type):
    board = Board()
    expected = str(board.to_svg())
    for name in ["MOVE_NONE", "MOVE_NULL", "MOVE_RESIGN", "MOVE_WIN", "MOVE_END"]:
        assert str(board.to_svg(getattr(move_type, name), bold_destination=True)) == expected
    board.apply_usi("7g7f")
    assert "font-weight" not in str(board.to_svg(bold_destination=True))
    empty_destination = str(Board().to_svg(Move32.from_usi("7g7f"), bold_destination=True))
    assert "font-weight" not in empty_destination


def test_background_is_an_xml_attribute():
    value = '"/><text>&\'<"'
    root = ET.fromstring(str(Board().to_svg(board_background=value)))
    assert root.find("s:rect", NS).get("fill") == value
    assert not root.findall("s:text[@injected]", NS)


@pytest.mark.parametrize("value", ["7g7f", 123])
def test_last_move_type_is_preserved(value):
    with pytest.raises(TypeError):
        Board().to_svg(value)
