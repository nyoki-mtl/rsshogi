# rsshogi

`rsshogi` は、Python から将棋の局面、指し手、合法性、棋譜、学習データ形式を扱うための
ライブラリです。

> English: `rsshogi` provides Python tools for shogi board handling, move
> legality, game records, policy labels, and dataset formats.

Rust 製の `rsshogi` core を内部で利用していますが、一般的な Python モジュールと同じように
使えるよう構成しています。まずはこの標準ビルドを利用してください。AVX2 対応の x86_64 CPU で
AVX2 最適化版を使いたい場合は
[`rsshogi-avx2`](https://pypi.org/project/rsshogi-avx2/) を利用します。

Python API は [cshogi](https://pypi.org/project/cshogi/) から影響を受けています。

## できること

- SFEN / USI から局面を構築し、指し手を適用して更新する
- 指し手の生成と検証を行い、局面を SFEN として出力する
- KIF / KI2 / CSA / JKF / SFEN などの棋譜形式を読み書きする
- policy label や学習データ向けに指し手や局面を変換する
- データセット向けに、NumPy で扱いやすい packed position 形式を利用する

## 開発用インストール

```bash
python -m pip install maturin
maturin develop -m crates/rsshogi-py/pyproject.toml
```

## AVX2 ビルド

`rsshogi-avx2` は、AVX2 対応 x86_64 CPU 向けに最適化した同じ Python モジュールです。

```bash
python -m pip install rsshogi-avx2
```

## 注意

- `rsshogi` と `rsshogi-avx2` は同時にインストールせず、どちらか一方だけを利用してください。
- 幅広い環境では `rsshogi` が安全な標準選択です。
- 学習向けの policy label 変換は `rsshogi.policy` から利用できます。

## Policy Label

```python
from rsshogi.core import Move
from rsshogi.policy import compact_move_label, move_label
from rsshogi.types import Color

mv = Move.from_usi("7g7f")

label = move_label(mv, Color.BLACK)
compact = compact_move_label(mv, Color.BLACK)

print(label)    # 2187 クラス
print(compact)  # 1496 クラス or None
```

## クイック例

```python
from rsshogi.core import Board

board = Board()
board.apply_usi("7g7f")
print(board.to_sfen())
```

raw-state を Python から直接編集することもできます。

```python
from rsshogi.core import Board
from rsshogi.types import Color, Piece, PieceType, Square

board = Board()
state = board.to_position_state()
state.set_piece(Square.from_usi("7g"), Piece(0))
state.set_piece(Square.from_usi("7f"), Piece.from_color_type(Color.BLACK, PieceType.PAWN))
state.ply = 42

board.set_position_state(state)
report = board.validate_all()
print(board.to_sfen())
print(report.is_valid())
```

USI の `position` 文字列を直接扱うこともできます。

```python
from rsshogi.core import Board, normalize_usi_position, parse_usi_position

board = Board()
board.set_usi_position("position startpos moves 7g7f 3c3d")

board2 = parse_usi_position("startpos moves 7g7f")
print(board2.to_sfen())

print(normalize_usi_position("position startpos"))  # "startpos"
```

## 構造化 import

`rsshogi` は用途別の submodule から import できます。

```python
# 型と定数
from rsshogi.types import Color, PieceType, Square
from rsshogi.core import Move, Move32

# 盤面
from rsshogi.core import Board, PositionState, ValidationIssue, ValidationReport

# 棋譜
from rsshogi.record import Record, RecordMetadata, GameResult

# 棋譜変換
record = Record.from_kif_str(kif_text)
kif_text = record.to_kif()

# 棋譜ファイル I/O
record = Record.from_kif_file("example.kif")
record.write_kif("example_out.kif")

# NumPy dtype
from rsshogi.numpy import (
    PackedSfen,
    PackedSfenValue,
    HuffmanCodedPos,
    HuffmanCodedPosAndEval,
)

# Policy label
from rsshogi.policy import move_label, compact_move_label
```

基本的には submodule から import してください。

```python
from rsshogi.core import Board
from rsshogi.core import Move
from rsshogi.record import Record
```

## ドキュメント

API リファレンスと、各棋譜形式の細かな互換挙動（コメントの扱い、終局行や消費時間の整形など）は
mdBook ドキュメントにまとめています。

- [ドキュメント全体](https://nyoki-mtl.github.io/rsshogi/)
- [Python API（Board / Record など）](https://nyoki-mtl.github.io/rsshogi/python/index.html)
