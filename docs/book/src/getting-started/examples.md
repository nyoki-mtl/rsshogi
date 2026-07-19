# 例とパターン

実践的なコード例を紹介します。基本的な使い方は [クイックスタート](quickstart.md) をご覧ください。

## 対局のシミュレーション

### ランダム対局

```python
import random
from rsshogi.core import Board

board = Board()

# 最大200手までランダムに対局
for i in range(200):
    moves = board.legal_moves()
    if not moves:
        break

    move = random.choice(moves)
    board.apply_move(move)

    if board.repetition_state().to_usi() != "rep_none":
        break

# 結果を確認
if board.is_mated():
    # is_mated() が True のとき board.turn は詰まされた側。勝者はその相手。
    winner = "先手" if board.turn.is_white() else "後手"
    print(f"{board.game_ply}手で{winner}の勝ち（詰み）")
elif board.is_repetition():
    print(f"千日手: {board.repetition_state().to_usi()}")
else:
    print(f"{board.game_ply}手で終了")
```

### 合法手を探索（1手先読み）

```python
from rsshogi.core import Board

board = Board()

# 各合法手を試して評価
for move in board.legal_moves():
    board.apply_move(move)
    
    # 相手の合法手数を評価値として使用
    score = len(board.legal_moves())
    print(f"{move.to_usi()}: 相手の合法手 {score} 手")
    
    board.undo_move(move)
```

## 棋譜処理

### 複数の棋譜ファイルを処理

```python
from pathlib import Path
from rsshogi.record import Record

kif_dir = Path("kifu")

for kif_file in kif_dir.glob("*.kif"):
    try:
        record = Record.from_kif_file(str(kif_file))
        print(f"{kif_file.name}: {record.move_count}手 - {record.result.name}")
    except Exception as e:
        print(f"{kif_file.name}: エラー - {e}")
```

### 棋譜から特定の局面を抽出

```python
from rsshogi.core import Board
from rsshogi.record import Record

record = Record.from_kif_file("example.kif")
board = Board(sfen=record.init_position_sfen)

# 10手目の局面を取得
for i, move_rec in enumerate(record.moves[:10], 1):
    board.apply_move(move_rec.move)

print(f"10手目の局面: {board.to_sfen()}")
```

### 棋譜のフォーマット変換

```python
from rsshogi.record import Record

# KIF → CSA
record = Record.from_kif_file("input.kif")
record.write_csa("output.csa")

# CSA → KIF
record = Record.from_csa_file("input.csa")
record.write_kif("output.kif")

# 一括変換
from pathlib import Path

for kif_file in Path("kifu").glob("*.kif"):
    record = Record.from_kif_file(str(kif_file))
    csa_file = kif_file.with_suffix(".csa")
    record.write_csa(str(csa_file))
```

## 局面の分析

### 駒の配置を確認

```python
from rsshogi.core import Board
from rsshogi.types import Color, PieceType, Square

board = Board()

# 全てのマスを走査
for file in range(9, 0, -1):  # 9〜1筋
    for rank in range(1, 10):  # 1〜9段
        sq = Square.from_usi(f"{file}{chr(ord('a') + rank - 1)}")
        piece = board.piece_on(sq)
        if piece.piece_type != PieceType.NO_PIECE_TYPE:
            print(f"{file}{rank}: {piece.piece_type.name}")
```

### 特定の駒の位置を取得

```python
from rsshogi.core import Board
from rsshogi.types import Color, PieceType

board = Board()

# 先手の歩の位置をビットボードで取得
pawns = board.pieces_for(PieceType.PAWN, Color.BLACK)
print(f"先手の歩: {pawns.count()}枚")

# 全ての飛車・角の位置
rooks = board.pieces_by_type(PieceType.ROOK)
bishops = board.pieces_by_type(PieceType.BISHOP)
```

### 持ち駒の確認

```python
from rsshogi.core import Board
from rsshogi.types import Color, PieceType

board = Board()

# 指し手を適用して駒を取る
board.apply_usi("7g7f")
board.apply_usi("3c3d")
# ... 駒を取る手を適用

# 持ち駒を確認
for color in [Color.BLACK, Color.WHITE]:
    hand = board.hand(color)
    name = "先手" if color == Color.BLACK else "後手"
    pieces = []
    for pt in [PieceType.ROOK, PieceType.BISHOP, PieceType.GOLD, 
               PieceType.SILVER, PieceType.KNIGHT, PieceType.LANCE, PieceType.PAWN]:
        count = hand.count(pt)
        if count > 0:
            pieces.append(f"{pt.name}:{count}")
    if pieces:
        print(f"{name}の持ち駒: {', '.join(pieces)}")
```

## エラーハンドリング

### 違法手の検出

```python
from rsshogi.core import Board, Move

board = Board()

# 方法1: 例外をキャッチ
try:
    board.apply_usi("9a9b")  # 違法手
except ValueError as e:
    print(f"違法手です: {e}")

# 方法2: 事前にチェック
move = Move.from_usi("7g7f")
if board.is_legal_move(move):
    board.apply_move(move)
else:
    print("この手は指せません")
```

### 最後の手を取り消す

```python
from rsshogi.core import Board

board = Board()

# 方法1: last_move を確認してから undo_move32
last_move = board.last_move()
if last_move:
    board.undo_move32(last_move)
else:
    print("戻せる手がありません")
```

## Move32 と Move の使い分け

```python
from rsshogi.core import Board, Move32, Move

board = Board()

# Move: legal_moves() が返す型（基本はこちら）
moves = board.legal_moves()  # list[Move]
mv = moves[0]
print(type(mv))  # <class 'rsshogi.core.Move'>

# Move32: 32bit 指し手が必要な場合
moves32 = board.legal_moves_move32()  # list[Move32]
move32 = moves32[0]
print(type(move32))  # <class 'rsshogi.core.Move32'>

# 型に対応するメソッドを使う
board.apply_move32(move32)
board.apply_move(mv)

# Move はメモリ効率が良い（16bit vs 32bit）
# 大量の指し手を保存する場合に有利
```

## 実行可能なサンプル

リポジトリの `examples/python/` に実行可能なサンプルがあります。

```bash
# 基本的な盤面操作
python examples/python/basic.py

# apply_move と undo_move
python examples/python/push_pop.py

# 合法手と擬似合法手
python examples/python/board_moves.py
```

## 次のステップ

- [Python API リファレンス](../python/index.md)：詳細な API 仕様
- [FAQ](../reference/faq.md)：よくある質問
