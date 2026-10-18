"""Execute the reference manual's Python examples against real bindings."""

import re
from pathlib import Path

import pytest
from rsshogi.core import Board
from rsshogi.record import EngineInfo, GameResult, MoveEntry, Record, SpecialMoveEntry

REFERENCE = Path(__file__).resolve().parents[2] / "docs/book/src/reference"
CASES = [
    ("compat.md", "mv32.to_csa() == '+7776FU'"),
    ("sbinpack.md", "len(pairs) == len(file_pairs) == 1 and meta == b'game-001'"),
    ("sazpack.md", "len(decoded_games) == len(loaded_games) == 1"),
    ("formats/book.md", "entry is not None and len(entry.candidates) > 0"),
    ("formats/csa.md", "len(records_from_file) == len(records_from_str) == 2"),
    ("formats/index.md", "all((csa_text, ki2_text, jkf_text))"),
    ("formats/jkf.md", "len(data['moves']) > 1"),
    ("formats/ki2.md", "mv.to_ki2(board) == '▲７六歩' and bool(output_text)"),
    ("formats/kif.md", "bool(kif_str) and Path('output.kif').is_file()"),
    ("formats/sbk.md", "entry is not None and child is not None"),
    ("formats/training-binaries.md", "len(records) == 2 and psv_arr['score'][0] == 120"),
    ("formats/yaneuraou.md", "entry is not None and len(entry.moves) > 0"),
]


def _varint(value):
    data = bytearray()
    while value >= 128:
        data.append((value & 127) | 128)
        value >>= 7
    data.append(value)
    return bytes(data)


@pytest.fixture
def manual_inputs(tmp_path, monkeypatch):
    """Provide the files and input strings assumed by the examples."""
    monkeypatch.chdir(tmp_path)
    board = Board()
    record = Record.from_main_line(
        board.to_sfen(),
        [MoveEntry("7g7f", engine_info=EngineInfo(eval=100)), MoveEntry("3c3d", engine_info=EngineInfo(eval=-80))],
        SpecialMoveEntry("RESIGN", GameResult.WHITE_WIN),
    )
    kif_text = record.to_kif()
    csa_text = record.to_csa()
    jkf_text = record.to_jkf()
    for name in ("sample.kif", "game.kif"):
        (tmp_path / name).write_text(kif_text, encoding="utf-8")
    (tmp_path / "sample.jkf").write_text(jkf_text, encoding="utf-8")
    (tmp_path / "single_game.csa").write_text(csa_text, encoding="utf-8")
    multi_csa_text = csa_text + "/\n" + csa_text
    (tmp_path / "multi_games.csa").write_text(multi_csa_text, encoding="utf-8")
    (tmp_path / "user_book1.db").write_text(
        f"#YANEURAOU-DB2016 1.00\nsfen {board.to_sfen()}\n7g7f 3c3d 12 8 99\n",
        encoding="utf-8",
    )
    # Root id 10 has a legal edge to implicit child id 20.
    move = b"\x08" + _varint(0x01007677) + b"\x20\x14"
    root = b"\x08\x0a\x4a" + _varint(len(move)) + move
    (tmp_path / "opening.sbk").write_bytes(b"\x1a" + _varint(len(root)) + root + b"\x1a\x02\x08\x14")
    return {
        "kif_text": kif_text,
        "ki2_text": record.to_ki2(),
        "jkf_text": jkf_text,
        "multi_csa_text": multi_csa_text,
    }


@pytest.mark.parametrize(("relative_path", "postcondition"), CASES, ids=[case[0] for case in CASES])
def test_reference_python_example(relative_path, postcondition, manual_inputs):
    """Run the unmodified Markdown code, then check a nonempty result."""
    path = REFERENCE / relative_path
    blocks = re.findall(r"^```python\n(.*?)^```", path.read_text(encoding="utf-8"), re.MULTILINE | re.DOTALL)
    assert len(blocks) == 1
    namespace = {"Path": Path, **manual_inputs}
    exec(compile(blocks[0], str(path), "exec"), namespace)
    assert eval(postcondition, namespace)
