# 実践レシピ集

局面の操作、簡易探索、棋譜の一括処理、ビットボードを用いた局面分析、およびエラー処理の実践的なコードパターンを示す。
基本的な API の使い方は [クイックスタート](quickstart.md) を参照。

## 対局のシミュレーションと終局判定

ランダムに合法手を選びながら対局を進め、詰みまたは千日手で終了する例を示す。

```python
import random
from rsshogi.core import Board

board = Board()

for _ in range(200):
    moves = board.legal_moves()
    if not moves:
        break
    board.apply_move(random.choice(moves))
    if board.is_repetition():
        break

if board.is_mated():
    # is_mated() が True の時点では、詰まされた側が手番（board.turn）を持っている
    winner = "先手" if board.turn.is_white() else "後手"
    print(f"{board.game_ply - 1} 手で{winner}の勝ち（詰み）")
elif board.is_repetition():
    print(f"千日手成立: {board.repetition_state().to_usi()}")
else:
    print(f"{board.game_ply - 1} 手で打ち切り")
```

- **`game_ply` と経過手数の関係**：`board.game_ply` は SFEN 末尾の手数番号を表し、平手初期局面では `1` から始まる。そのため、初期局面から実際に指された手数は `board.game_ply - 1` となる。
- **`is_repetition()` と `repetition_state()` の違い**：`is_repetition()` は同一局面（盤面、手番、双方の持ち駒がすべて一致）が 4 回出現した千日手成立時のみ `True` を返す。一方、`repetition_state()` は同一盤面で片方の持ち駒だけが増減した優等局面（`SUPERIOR`）や劣等局面（`INFERIOR`）も区別して返すため、探索木の枝刈りには `repetition_state()`、対局の終局判定には `is_repetition()` を使う。

## 1 手先読み探索（apply と undo の対称操作）

探索では `apply_move`（または `apply_move32`）で局面を進め、評価後に `undo_move`（または `undo_move32`）で元の局面へ戻す。
新しい `Board` インスタンスを毎手コピーするよりも、単一の `Board` に対して `apply` と `undo` を繰り返す方が高速に動作する。

```python
from rsshogi.core import Board

board = Board()

for move in board.legal_moves():
    board.apply_move(move)
    reply_count = len(board.legal_moves())
    print(f"{move.to_usi()}: 相手の応手 {reply_count} 通り")
    board.undo_move(move)
```

## `Move`（16-bit）と `Move32`（32-bit）の使い分け

`rsshogi` には 2 種類の主要な指し手型がある。

| 型 | 保持する情報 | 主な用途 |
|---|---|---|
| `Move` | 移動元、移動先、成りフラグ、駒打ちフラグ（16 bit）。 | 合法手リスト（`legal_moves()`）、定跡、棋譜の指し手、Policy ラベル変換。 |
| `Move32` | `Move` の全情報 + 移動後の色付き駒（32 bit）。 | `to_csa()` のように駒名を必要とする出力、`last_move()` の履歴参照、高速な `apply_move32` / `undo_move32`。 |

```python
from rsshogi.core import Board

board = Board()

# Move（16 bit）から Move32（32 bit）への昇格には現在の盤面情報を利用する
mv16 = board.legal_moves()[0]
mv32 = board.move32_from_move(mv16)

print(mv16.to_usi())  # "7g7f" など
print(mv32.to_csa())  # "+7776FU" など（Move32 は駒種を持つため盤面なしで CSA 出力可能）

# Move32 から Move への変換は上位ビットを落とすだけで盤面を必要としない
assert mv32.to_move() == mv16
```

## 棋譜の一括変換と局面抽出

ディレクトリ内の KIF ファイルを走査し、CSA 形式への一括変換や特定手数目の局面抽出を行う例を示す。

```python
from pathlib import Path
from rsshogi.core import Board
from rsshogi.record import Record

kif_dir = Path("kifu")

for kif_path in kif_dir.glob("*.kif"):
    try:
        # UTF-8 の .kif を読む場合は encoding="utf-8" を指定する
        record = Record.from_kif_file(str(kif_path))
    except ValueError as err:
        print(f"{kif_path.name}: 解析失敗 ({err})")
        continue

    # CSA ファイルへ書き出し
    csa_path = kif_path.with_suffix(".csa")
    record.write_csa(str(csa_path))

    # 本譜の 20 手目終了時点の SFEN を抽出
    board = Board(sfen=record.init_position_sfen)
    for entry in record.moves[:20]:
        board.apply_move(entry.move)
    print(f"{kif_path.name} (20 手目): {board.to_sfen()}")
```

なお、単純に棋譜全体の各局面 SFEN リストだけが必要な場合は、`record.replay_positions()` メソッドで初期局面から終局までの SFEN リストを一括取得できる。

## ビットボードと持ち駒による局面分析

盤上の全マスをループで調べる代わりに、`Bitboard` や `iter_pieces()`、`hand_counts()` を使うと簡潔に局面を集計できる。

```python
from rsshogi.core import Board
from rsshogi.types import Color, PieceType

board = Board()
board.apply_usi("7g7f")
board.apply_usi("3c3d")
board.apply_usi("8h2b+")

# 盤上のすべての駒を (Square, Piece) のリストとして取得
for sq, piece in board.iter_pieces():
    if piece.piece_type == PieceType.HORSE:
        print(f"馬の位置: {sq.to_usi()}")  # "2b"

# ビットボードで先手の歩の枚数と位置を取得
black_pawns = board.pieces_for(PieceType.PAWN, Color.BLACK)
print(f"先手の盤上の歩: {black_pawns.count()} 枚")

# 持ち駒を 7 駒種すべての辞書 {"P": ..., "L": ..., "N": ..., "S": ..., "B": ..., "R": ..., "G": ...} で取得
counts = board.hand_counts(Color.BLACK)
print(f"先手の持ち駒の角: {counts['B']} 枚")  # 1 枚
```

## 違法手の事前検査と例外処理

外部入力の指し手を適用する際は、`is_legal_move()` で事前判定するか、`apply_usi()` が送出する `ValueError` を捕捉する。

```python
from rsshogi.core import Board, Move

board = Board()

# 方法 1：Move オブジェクトに変換して合法性を事前検査する
candidate = Move.from_usi("7g7f")
if board.is_legal_move(candidate):
    board.apply_move(candidate)

# 方法 2：apply_usi で構文エラーや非合法手をまとめて ValueError として捕捉する
try:
    board.apply_usi("9a9b")
except ValueError as err:
    print(f"適用できない指し手: {err}")
```

## リポジトリ同梱のサンプルスクリプト

リポジトリの `examples/` ディレクトリには、そのまま実行できるサンプルコードが含まれている。

- **Rust サンプル**：`cargo run --example sfen_roundtrip`、`cargo run --example sbinpack_size_breakdown --features records`、`cargo run --example sazpack_measure --features records`。
- **Python サンプル**：`examples/python/basic.py`、`examples/python/push_pop.py`、`examples/python/board_moves.py`、`examples/python/move_roundtrip.py`、`examples/python/sequence.py`。
