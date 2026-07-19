# Move クラス

```python
rsshogi.core.Move(value)
```

将棋の指し手を表す 16bit の軽量型です。メモリ効率を重視した指し手表現で、
大量の指し手を保存する場合に有用です。

## 概要

Move クラスは以下の機能を提供します：

- USI 形式の文字列への変換・からの生成
- `AperyMove` との相互変換
- 駒打ち、成りなどの判定
- KI2 形式への変換（`Board` を渡す必要あり）
- メモリ効率の良い表現（Move32 の半分のサイズ）

**用途:** 通常は `Move` を使用してください。大量の棋譜データや探索木でもそのまま使えます。
`Move32` は CSA 変換や駒情報の直接取得が必要な場合に使用します。

## 基本的な使い方

```python
from rsshogi.core import Board, Move

# USI 文字列から生成
move = Move.from_usi("7g7f")
print(move.to_usi())  # => "7g7f"

# Board で使用
board = Board()
board.apply_move(move)  # Move で指せる
```

---

## クラスメソッド

### `from_usi(usi)`

```python
Move.from_usi(usi: str) -> Move
```

USI 文字列から `Move` インスタンスを生成します。

**引数:**
- **usi**: `str` - USI 形式の指し手文字列（例: `"7g7f"`, `"P*5e"`）

**戻り値:**
- `Move`: 生成された Move インスタンス

**例外:**
- `ValueError`: USI 文字列のパースに失敗した場合

**使用例:**

```python
from rsshogi.core import Move

move = Move.from_usi("7g7f")
print(move.to_usi())  # => "7g7f"

# 駒打ち
drop_move = Move.from_usi("P*5e")
print(drop_move.is_drop())  # => True
```

---

## インスタンスメソッド

### 文字列変換

#### `to_usi()`

```python
mv.to_usi() -> str
```

Move を USI 形式の文字列に変換します。

**戻り値:**
- `str`: USI 形式の指し手文字列

---

#### `to_ki2(board)`

```python
mv.to_ki2(board: Board) -> str | None
```

Move を現在局面に基づいて補完し、KI2 形式の文字列に変換します。
KI2 の左右・直・寄・引・上などの表記判定には局面情報が必要なため、`board` を渡します。

**戻り値:**
- `str | None`: KI2 形式の指し手。変換できない場合は `None`

```python
from rsshogi.core import Board, Move

board = Board()
move = Move.from_usi("7g7f")
print(move.to_ki2(board))  # => "▲７六歩"
```

---

#### `to_apery()`

```python
mv.to_apery() -> AperyMove
```

`cshogi` / Apery 互換の 16bit 指し手へ変換します。

```python
from rsshogi.core import Move

mv = Move.from_usi("7g7f")
amv = mv.to_apery()

print(int(amv))
print(amv.to_move().to_usi())
```

---

### 特殊手定数（クラス属性）

- `Move.MOVE_NONE`
- `Move.MOVE_NULL`
- `Move.MOVE_RESIGN`
- `Move.MOVE_WIN`
- `Move.MOVE_END`

```python
from rsshogi.core import Move

assert Move.MOVE_NONE.to_usi() == "none"
assert Move.MOVE_RESIGN.to_usi() == "resign"
```

---

### 指し手の判定

#### `is_drop()`

```python
mv.is_drop() -> bool
```

この指し手が駒打ちかどうかを判定します。

**戻り値:**
- `bool`: 駒打ちの場合 `True`

---

#### `is_promotion()`

```python
mv.is_promotion() -> bool
```

この指し手が成りかどうかを判定します。

**戻り値:**
- `bool`: 成りの場合 `True`

---

#### `is_normal()`

```python
mv.is_normal() -> bool
```

この指し手が通常の指し手（特殊手定数ではない）かどうかを判定します。

**戻り値:**
- `bool`: 有効な手の場合 `True`

---

### 移動情報の取得

#### `from_sq` / `to_sq`

```python
mv.from_sq -> Square
mv.to_sq -> Square
```

移動元・移動先のマスを取得します。

---

#### `dropped_piece_type`

```python
mv.dropped_piece_type -> PieceType | None
```

駒打ちの場合、打った駒種を返します。駒打ちでない場合は `None`。

---

#### `move_type`

```python
mv.move_type -> MoveType
```

指し手の種類を取得します。

**戻り値:**
- `MoveType`: `MoveType.NORMAL`、`MoveType.PROMOTION`、`MoveType.DROP` のいずれか

---

## 特殊メソッド

- `int(mv)` → 内部値（16bit 整数）を返す
- `str(mv)` → USI 形式の文字列を返す（`to_usi()` と同値）

## プロパティ

### `value`

```python
mv.value -> int
```

Move の内部表現（16bit 整数）にアクセスします。

---

## 使用例

### Board と組み合わせる

```python
import rsshogi

board = rsshogi.core.Board()

# Move で指す
move = rsshogi.core.Move.from_usi("7g7f")
board.apply_move(move)

print(board.to_sfen())
```

---

### 大量の指し手を保存

Move はメモリ効率が良いため、大量の指し手を保存する場合に有利です。

```python
from rsshogi.core import Move

# 棋譜データなど大量の指し手を扱う場合
game_moves = [
    Move.from_usi("7g7f"),
    Move.from_usi("3c3d"),
    Move.from_usi("2g2f"),
    # ... 数百手
]

# メモリ使用量は Move32 の半分
print(f"保存した指し手数: {len(game_moves)}")
```

---

### 内部値による再構築

```python
from rsshogi.core import Move

move = Move.from_usi("7g7f")

# 内部値を取得
internal_value = int(move)

# 内部値から再構築
reconstructed = Move(internal_value)
print(reconstructed.to_usi())  # => "7g7f"
```

---

## Move と Move32 の比較

| 特性 | Move (16bit) | Move32 (32bit) |
|------|----------------|--------------|
| サイズ | 2バイト | 4バイト |
| 移動元・移動先 | ✓ | ✓ |
| 成り / 駒打ちフラグ | ✓ | ✓ |
| 移動後の駒情報 | × | ✓ |
| USI 変換 | ✓ | ✓ |
| CSA 変換 | × | ✓ |
| KI2 変換 | ✓（board 必要） | ✓（board 必要） |
| メモリ効率 | 高い | 標準 |

### Move の制限

Move には駒情報が含まれないため、以下の操作はできません：

- CSA 形式への変換
- 駒種の直接取得（盤面から取得する必要がある）

`to_ki2(board)` は `Board` の文脈から駒情報を補完して KI2 文字列を生成するため、Move でも利用できます。

### 使い分け

**Move を推奨する場合:**
- 一般的な用途（`Board.legal_moves()` の戻り値）
- USI 形式のみで十分
- 大量の棋譜データや探索木で指し手を保存

**Move32 を検討する場合:**
- CSA 形式への変換が必要
- 駒情報を直接取得したい

### 型に対応するメソッド

`Board.apply_move()` は Move を、`Board.apply_move32()` は Move32 を受け付けます。

```python
import rsshogi

board = rsshogi.core.Board()

# Move は apply_move で適用する
mv = rsshogi.core.Move.from_usi("7g7f")
board.apply_move(mv)  # OK

# Move32 は apply_move32 で適用する
move32 = board.legal_moves_move32()[0]
board.apply_move32(move32)  # OK
```

---

## 他エンジンとの命名対応

rsshogi では 16bit 指し手を **基本型** として `Move` と命名しています。
主要な将棋エンジンや cshogi では同じ 16bit 型を `Move16` と呼ぶため、混同に注意してください。

| エンジン/ライブラリ | 16bit 型 | 32bit 型 |
|---------------------|----------|----------|
| **rsshogi** | `Move` | `Move32` |
| **参照実装** | `Move16` | `Move` |
| **cshogi** (Apery 系) | `Move16` | `Move` |

- rsshogi の `Move` は 参照実装の `Move16` と同一のビットレイアウトです。
- cshogi の `Move16` とはビットレイアウトが異なります（駒打ち判定方法・成りフラグ位置が違う）。
  cshogi との間で指し手の整数値を直接比較してはならず、相互変換には USI 文字列を経由してください。

---

## 技術詳細: ビット構造

Move は **互換**の指し手エンコーディングを採用しています。

```text
bit位置:  15  14 | 13 ... 7 | 6 ... 0
          ─────────────────────────────
          │ P │ D │移動元(7)│移動先(7)│
          ─────────────────────────────
          P: 成りフラグ (bit 15)
          D: 駒打ちフラグ (bit 14)
```

- **bit 0-6**: 移動先の座標
- **bit 7-13**: 移動元の座標または打つ駒種
- **bit 14**: 駒打ちフラグ
- **bit 15**: 成りフラグ

---

## 関連項目

- [`Board`](board.md) - 盤面管理クラス
- [`Move32`](move32.md) - 32bit 指し手型
- [クイックスタート](../getting-started/quickstart.md)
