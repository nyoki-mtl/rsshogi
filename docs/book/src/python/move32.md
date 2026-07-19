# Move32 クラス

```python
rsshogi.core.Move32(value)
```

将棋の指し手を表す 32bit の型です。駒の種類や移動情報を含む完全な指し手表現で、
`Board.legal_moves_move32()` が返す指し手型です。

## 概要

Move32 クラスは以下の機能を提供します：

- USI 形式の文字列への変換・からの生成
- `AperyMove32` との変換
- CSA / KI2 形式への変換（駒情報を含むため可能）
- 駒打ち、成りなどの判定
- 移動元・移動先・駒種の取得

**推奨:** 32bit 指し手が必要な場合は `Board.legal_moves_move32()` を使用してください。

## 基本的な使い方

```python
from rsshogi.core import Board, Move32

# Board から 32bit 指し手を取得するのが標準的な方法
board = Board()
moves = board.legal_moves_move32()
move = moves[0]

print(move.to_usi())       # => "7g7f" など
print(move.is_drop())      # => False
print(move.is_promotion())   # => False

# USI 文字列から生成することも可能
move = Move32.from_usi("7g7f")
print(move.to_usi())  # => "7g7f"
```

---

## クラスメソッド

### `from_usi(usi)`

```python
Move32.from_usi(usi: str) -> Move32
```

USI 文字列から `Move32` インスタンスを生成します。

**引数:**
- **usi**: `str` - USI 形式の指し手文字列（例: `"7g7f"`, `"P*5e"`）

**戻り値:**
- `Move32`: 生成された Move32 インスタンス

**例外:**
- `ValueError`: USI 文字列のパースに失敗した場合

**注意:** `from_usi()` で生成した Move32 には駒情報が含まれないため、
CSA / KI2 形式への変換はできません。完全な Move32 を得るには `Board` 経由で取得してください。

**使用例:**

```python
from rsshogi.core import Move32

move = Move32.from_usi("7g7f")
print(move.to_usi())  # => "7g7f"

# 駒打ち
drop_move = Move32.from_usi("P*5e")
print(drop_move.is_drop())  # => True
```

---

## インスタンスメソッド

### 文字列変換

#### `to_usi()`

```python
move.to_usi() -> str
```

Move32 を USI 形式の文字列に変換します。

**戻り値:**
- `str`: USI 形式の指し手文字列

---

#### `to_csa()`

```python
move.to_csa() -> str | None
```

Move32 を CSA 形式の文字列に変換します。駒情報が必要なため、
`Board.legal_moves_move32()` から取得した Move32 でのみ使用できます。

**戻り値:**
- `str | None`: CSA 形式の文字列（特殊手の場合は `None`）

---

#### `to_ki2(board)`

```python
move.to_ki2(board: Board) -> str | None
```

Move32 を KI2 形式の文字列に変換します。  
曖昧性解消（右/左/直/寄/引 など）のため、局面 `board` が必要です。

**戻り値:**
- `str | None`: KI2 形式の文字列（変換できない場合は `None`）

---

### 特殊手定数（クラス属性）

- `Move32.MOVE_NONE`
- `Move32.MOVE_NULL`
- `Move32.MOVE_RESIGN`
- `Move32.MOVE_WIN`
- `Move32.MOVE_END`

```python
from rsshogi.core import Move32

assert Move32.MOVE_NONE.to_usi() == "none"
assert Move32.MOVE_NULL.to_usi() == "null"
```

---

### 指し手の判定

#### `is_drop()`

```python
move.is_drop() -> bool
```

この指し手が駒打ちかどうかを判定します。

**戻り値:**
- `bool`: 駒打ちの場合 `True`

---

#### `is_promotion()`

```python
move.is_promotion() -> bool
```

この指し手が成りかどうかを判定します。

**戻り値:**
- `bool`: 成りの場合 `True`

---

#### `is_normal()`

```python
move.is_normal() -> bool
```

この指し手が通常の指し手（特殊手定数ではない）かどうかを判定します。

**戻り値:**
- `bool`: 有効な手の場合 `True`

---

### 移動情報の取得

#### `from_sq` / `to_sq`

```python
move.from_sq -> Square
move.to_sq -> Square
```

移動元・移動先のマスを取得します。

**戻り値:**
- `Square`: マス

---

#### `dropped_piece_type`

```python
move.dropped_piece_type -> PieceType | None
```

駒打ちの場合、打った駒種を返します。駒打ちでない場合は `None`。

---

#### `piece_after_move`

```python
move.piece_after_move -> Piece
```

移動後の駒を取得します。成りの場合は成り後の駒、
通常移動の場合は移動する駒、駒打ちの場合は打つ駒を返します。

**注意:** `from_usi()` で生成した Move32（部分 Move32）では駒情報が未設定のため、
意味のない値が返ります。正しい駒情報が必要な場合は `Board.legal_moves_move32()` または
`Board.move32_from_move()` で取得した Move32 を使用してください。

---

#### `move_type`

```python
move.move_type -> MoveType
```

指し手の種類を取得します。

**戻り値:**
- `MoveType`: `MoveType.NORMAL`、`MoveType.PROMOTION`、`MoveType.DROP` のいずれか

---

#### `to_move()`

```python
move.to_move() -> Move
```

`Move` に変換します。駒情報は失われます。

---

#### `to_apery(board)`

```python
move.to_apery(board: Board) -> AperyMove32
```

`cshogi` / Apery 互換の 32bit 指し手に変換します。
取得駒種の補完が必要なため、現在局面 `board` を渡します。

```python
from rsshogi.core import Board

board = Board()
move32 = board.legal_moves_move32()[0]
amv32 = move32.to_apery(board)

print(amv32.to_usi())
print(amv32.captured_piece_type)
```

---

## 特殊メソッド

- `int(move)` → 内部値（32bit 整数）を返す
- `str(move)` → USI 形式の文字列を返す（`to_usi()` と同値）

## プロパティ

### `value`

```python
move.value -> int
```

Move32 の内部表現（32bit 整数）にアクセスします。

---

## 使用例

### Board と組み合わせる

```python
import rsshogi

board = rsshogi.core.Board()

# 合法手を取得して適用（32bit 指し手）
move = board.legal_moves_move32()[0]
board.apply_move32(move)

# CSA 形式で出力（駒情報を含むため可能）
for legal_move in board.legal_moves_move32():
    csa = legal_move.to_csa()
    print(f"{legal_move.to_usi()} => {csa}")
```

---

### 駒打ちと成りの判定

```python
from rsshogi.core import Move32

# 通常の手
move1 = Move32.from_usi("7g7f")
print(f"駒打ち: {move1.is_drop()}, 成り: {move1.is_promotion()}")
# => 駒打ち: False, 成り: False

# 駒打ち
move2 = Move32.from_usi("P*5e")
print(f"駒打ち: {move2.is_drop()}, 成り: {move2.is_promotion()}")
# => 駒打ち: True, 成り: False

# 成り
move3 = Move32.from_usi("2b2a+")
print(f"駒打ち: {move3.is_drop()}, 成り: {move3.is_promotion()}")
# => 駒打ち: False, 成り: True
```

---

### 内部値による再構築

```python
from rsshogi.core import Move32

move = Move32.from_usi("7g7f")

# 内部値を取得
internal_value = int(move)

# 内部値から再構築
reconstructed = Move32(internal_value)
print(reconstructed.to_usi())  # => "7g7f"
```

---

## Move32 と Move の比較

`Move32` は 32bit、`Move` は 16bit の指し手型です。

| 特性 | Move32 (32bit) | Move (16bit) |
|------|--------------|----------------|
| サイズ | 4バイト | 2バイト |
| 移動元・移動先 | ✓ | ✓ |
| 成り / 駒打ちフラグ | ✓ | ✓ |
| 移動後の駒情報 | ✓ | × |
| USI 変換 | ✓ | ✓ |
| CSA 変換 | ✓ | × |
| KI2 変換 | ✓（board 必要） | ✓（board 必要） |
| メモリ効率 | 標準 | 高い |

**使い分け:**

- **Move（推奨）**: 通常の用途。`Board.legal_moves()` の戻り値。
- **Move32**: CSA 変換や駒情報の直接取得が必要な場合。`Board.legal_moves_move32()` を使用。

---

## 技術詳細: ビット構造

Move32 は **互換**の指し手エンコーディングを採用しています。

```text
bit位置:  31 ... 21 | 20 ... 16 | 15  14 | 13 ... 7 | 6 ... 0
          ──────────────────────────────────────────────────────
          │  未使用  │移動後駒(5)│ P │ D │移動元(7) │移動先(7)│
          ──────────────────────────────────────────────────────
          P: 成りフラグ (bit 15)
          D: 駒打ちフラグ (bit 14)
```

- **bit 0-6**: 移動先の座標
- **bit 7-13**: 移動元の座標または打つ駒種
- **bit 14**: 駒打ちフラグ
- **bit 15**: 成りフラグ
- **bit 16-20**: 移動後の駒（`Piece`、手番情報を含む）

---

## 関連項目

- [`Board`](board.md) - 盤面管理クラス
- [`Move`](move.md) - 16bit 指し手型
- [クイックスタート](../getting-started/quickstart.md)
