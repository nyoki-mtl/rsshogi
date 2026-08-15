# クイックスタート

rsshogi の基本的な使い方を紹介します。

> **インストールがまだの場合**
>
> [インストール](installation.md) を参照してください。

## 盤面を作成する

```python
from rsshogi.core import Board

# 平手初期局面
board = Board()

# SFEN から作成
board = Board(sfen="lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1")
```

## 指し手を適用する

```python
board = Board()

# USI 形式で指し手を適用
board.apply_usi("7g7f")
board.apply_usi("3c3d")

# 現在の局面を確認
print(board.to_sfen())
# => lnsgkgsnl/1r5b1/pppppp1pp/6p2/9/2P6/PP1PPPPPP/1B5R1/LNSGKGSNL b - 3
```

## 指し手を取り消す

```python
board = Board()
board.apply_usi("7g7f")
board.apply_usi("3c3d")

# 1手戻す
last_move = board.last_move()
if last_move:
    board.undo_move32(last_move)
```

## 合法手を列挙する

```python
board = Board()

# 全ての合法手を取得
for move in board.legal_moves():
    print(move.to_usi())

# 最初の合法手を指す
moves = board.legal_moves()
if moves:
    board.apply_move(moves[0])
```

## 局面の状態を判定する

```python
board = Board()

# 手番（Color.BLACK / Color.WHITE）
print(board.turn)

# 手数
print(board.game_ply)

# 王手されているか
if board.is_in_check():
    print("王手!")

# 詰みかどうか
if board.is_mated():
    print("詰み!")

# 千日手かどうか
if board.is_repetition():
    print(f"千日手: {board.repetition_state().to_usi()}")
```

## 棋譜を読み込む

```python
from rsshogi.record import Record

# KIF 文字列から
kif_text = """
手合割：平手
先手：先手
後手：後手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 ３四歩(33)
   3 投了
まで2手で後手の勝ち
"""
record = Record.from_kif_str(kif_text)

# ファイルから
record = Record.from_kif_file("example.kif")

# 対局情報を参照
print(record.metadata.black_player)  # 先手
print(record.metadata.white_player)  # 後手

# 指し手を参照
for move_rec in record.moves:
    print(move_rec.move.to_usi())

# 結果を参照
print(record.result.name)  # WHITE_WIN
```

## 棋譜を盤面で再生する

```python
from rsshogi.core import Board
from rsshogi.record import Record

record = Record.from_kif_file("example.kif")

# 初期局面から開始
board = Board(sfen=record.init_position_sfen)

# 指し手を順に適用
for move_rec in record.moves:
    board.apply_move(move_rec.move)
    print(f"{board.game_ply}手目: {move_rec.move.to_usi()}")

print(f"最終局面: {board.to_sfen()}")
```

## 棋譜を書き出す

```python
from rsshogi.record import Record

record = Record.from_kif_file("input.kif")

# 別の形式で書き出し
record.write_csa("output.csa")
record.write_ki2("output.ki2")
record.write_jkf("output.json")

# 文字列として取得
csa_text = record.to_csa()
kif_text = record.to_kif()
```

## 補助型を使う

```python
from rsshogi.core import Board
from rsshogi.types import Color, PieceType, Square

board = Board()

# 手番を Color で取得
color = board.turn
if color == Color.BLACK:
    print("先手の番")

# 特定のマスの駒を確認
sq = Square.from_usi("7g")
piece = board.piece_on(sq)
print(piece.piece_type)  # PieceType.PAWN

# 持ち駒を確認
hand = board.hand(Color.BLACK)
print(f"歩: {hand.count(PieceType.PAWN)}枚")
```

## 次のステップ

- [例とパターン](examples.md)：より多くのコード例
- [Python API リファレンス](../python/index.md)：詳細な API 仕様
- [FAQ](../reference/faq.md)：よくある質問
