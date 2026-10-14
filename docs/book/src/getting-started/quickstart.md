# クイックスタート

`rsshogi` の基本的な操作（局面の生成、指し手の適用と取り消し、合法手生成、状態判定、棋譜の入出力）を順に示す。
導入手順は [インストール](installation.md) を参照。

## 1. 局面を作成する

平手初期局面、または任意の SFEN 文字列から局面を生成する。

### Python

```python
from rsshogi.core import Board

# 平手初期局面で初期化
board = Board()

# SFEN 文字列を指定して初期化
board = Board(sfen="lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1")
```

### Rust

Rust では `board::hirate_position()` または `board::position_from_sfen()` を使う。
指し手を適用して履歴（`StateStack`）を管理する場合は、開始前に `init_stack()` を呼ぶ。

```rust
use rsshogi::board;

let mut pos = board::hirate_position();
pos.init_stack();

let from_sfen = board::position_from_sfen(
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## 2. 指し手を適用して取り消す

USI 形式の指し手文字列（`"7g7f"`、`"8h2b+"`、`"P*5e"` など）や、`Move` / `Move32` オブジェクトを適用して局面を進める。

### Python

```python
from rsshogi.core import Board

board = Board()

# USI 文字列で指し手を適用
board.apply_usi("7g7f")
board.apply_usi("3c3d")
print(board.to_sfen())
# => lnsgkgsnl/1r5b1/pppppp1pp/6p2/9/2P6/PP1PPPPPP/1B5R1/LNSGKGSNL b - 3

# 直前の指し手（Move32）を取得して 1 手戻す
last_move = board.last_move()
if last_move is not None:
    board.undo_move32(last_move)
```

### Rust

```rust
use rsshogi::board;

let mut pos = board::hirate_position();
pos.init_stack();

let mv1 = pos.move_from_usi("7g7f").unwrap();
let mv2 = pos.move_from_usi("3c3d").unwrap();
pos.apply_move32(mv1);
pos.apply_move32(mv2);

// 直前の手を逆順に取り消す
pos.undo_move32(mv2);
pos.undo_move32(mv1);
```

## 3. 合法手を列挙する

現在の局面におけるすべての合法手を取得する。
Python の `board.legal_moves()` は不成を含む全合法手（Rust の `LegalAll` 相当）を `list[Move]` として返す。

### Python

```python
from rsshogi.core import Board

board = Board()

# 合法手をすべて取得（平手初期局面では 30 手）
moves = board.legal_moves()
print(len(moves))  # 30

for move in moves[:5]:
    print(move.to_usi())

# 最初の合法手を適用
if moves:
    board.apply_move(moves[0])
```

### Rust

Rust ではヒープ確保を行わない `MoveList`（または `Move32List`）へ手生成を行う。
棋譜処理などすべての合法手が必要な場面では `generate_legal_all`（または `generate_moves::<LegalAll>`）、探索向けに明らかに損な不成を省く場面では `generate_moves::<Legal>` を使う。

```rust
use rsshogi::board::{self, MoveList};
use rsshogi::movegen::generate_legal_all;

let pos = board::hirate_position();
let mut list = MoveList::new();
generate_legal_all(&pos, &mut list);
assert_eq!(list.len(), 30);
```

## 4. 局面の状態を判定する

手番、手数、王手、詰み、千日手、1 手詰めを判定できる。

```python
from rsshogi.core import Board
from rsshogi.types import Color

board = Board()

# 手番（Color.BLACK または Color.WHITE）と SFEN の手数欄
print(board.turn == Color.BLACK)  # True
print(board.game_ply)            # 1

# 王手・詰み・千日手の判定
if board.is_in_check():
    print("王手がかかっている")
if board.is_mated():
    print("詰み")
if board.is_repetition():
    print(f"千日手状態: {board.repetition_state().to_usi()}")

# 1 手詰め探索（詰みがあれば Move32、なければ None）
mate_move = board.solve_mate_in_one()
```

## 5. マス目と持ち駒を参照する

`Square`、`PieceType`、`Color` を用いて盤上の駒や持ち駒を参照する。

```python
from rsshogi.core import Board
from rsshogi.types import Color, PieceType, Square

board = Board()

# 7七（USI 表記 "7g"）の駒を参照
sq = Square.from_usi("7g")
piece = board.piece_on(sq)
print(piece.piece_type == PieceType.PAWN)  # True

# 先手の持ち駒の歩の枚数を取得
hand = board.hand(Color.BLACK)
print(hand.count(PieceType.PAWN))  # 0
```

## 6. 棋譜を読み書きして再生する

`Record` クラスは KIF、KI2、CSA、JKF などの棋譜フォーマットを統一されたデータモデルで扱う。

```python
from rsshogi.core import Board
from rsshogi.record import Record

kif_text = """\
手合割：平手
先手：先手太郎
後手：後手次郎
手数----指手---------消費時間--
   1 ７六歩(77)
   2 ３四歩(33)
   3 投了
まで2手で後手の勝ち
"""

record = Record.from_kif_str(kif_text)
print(record.metadata.black_player)  # 先手太郎
print(record.result.name)            # WHITE_WIN

# 初期局面から本譜の指し手を順に再生
board = Board(sfen=record.init_position_sfen)
for entry in record.moves:
    board.apply_move(entry.move)
print(board.to_sfen())

# 別の棋譜フォーマットへ変換
csa_text = record.to_csa()
jkf_json = record.to_jkf()
```

ファイルから直接読み込む場合は `Record.from_kif_file("game.kif")`、ファイルへ書き出す場合は `record.write_csa("game.csa")` を使う。
`.kif` ファイルの既定文字コードは Shift_JIS、`.kifu` ファイルの既定文字コードは UTF-8 であり、`encoding="utf-8"` 引数で明示的に指定することもできる。

## 次のステップ

- [実践レシピ集](examples.md)：対局シミュレーション、探索、一括変換、エラー処理のパターン。
- [Python API リファレンス](../python/index.md)：全クラスと関数の詳細な仕様。
- [データモデルとフォーマット仕様](../reference/formats/index.md)：各棋譜・定跡・学習バイナリのフォーマット仕様。
