# 補助型

`rsshogi.types` モジュールには、盤面や指し手と密接に関係する補助的な型が含まれています。
これらの型を使用することで、型安全なコードを書くことができます。

```python
from rsshogi.types import Color, PieceType, Piece, Square, Bitboard, Hand, MoveType, RepetitionState
```

---

## Color

```python
rsshogi.types.Color
```

手番（先手/後手）を表す型です。

### クラス定数

| 定数 | 値 | 説明 |
|------|-----|------|
| `Color.BLACK` | 0 | 先手 |
| `Color.WHITE` | 1 | 後手 |

### コンストラクタ

```python
Color(value: int | None = None) -> Color
```

- `value` を省略すると `BLACK` になります。
- 不正な値を渡すと `ValueError` が発生します。

### クラスメソッド

```python
Color.from_usi(usi: str) -> Color
```

文字列から生成します。受理値は `"b"`, `"w"`（USI 標準）および `"black"`, `"white"`（大文字小文字を区別しない）です。

### インスタンスメソッド

| メソッド | 戻り値 | 説明 |
|----------|--------|------|
| `flip()` | `Color` | 手番を反転（BLACK → WHITE） |
| `opponent()` | `Color` | 相手番を返す（`flip()` の別名） |
| `is_black()` | `bool` | 先手かどうか |
| `is_white()` | `bool` | 後手かどうか |

### プロパティ

| プロパティ | 型 | 説明 |
|------------|-----|------|
| `value` | `int` | 内部値（0 or 1） |

### 特殊メソッド

- `int(color)` → 内部値を返す
- `str(color)` → `"black"` or `"white"`
- `==` / `!=` で比較可能（`Color` または `int` と比較）

### 使用例

```python
import rsshogi

board = rsshogi.core.Board()
color = board.turn

if color == rsshogi.types.Color.BLACK:
    print("先手の番")

# 手番を反転
opponent = color.flip()
print(opponent)  # => "white"
```

---

## PieceType

```python
rsshogi.types.PieceType
```

駒の種類を表す型です。成り駒も含みます。

### クラス定数

| 定数 | 値 | 説明 |
|------|-----|------|
| `PieceType.NO_PIECE_TYPE` | 0 | 駒なし |
| `PieceType.PAWN` | 1 | 歩 |
| `PieceType.LANCE` | 2 | 香 |
| `PieceType.KNIGHT` | 3 | 桂 |
| `PieceType.SILVER` | 4 | 銀 |
| `PieceType.BISHOP` | 5 | 角 |
| `PieceType.ROOK` | 6 | 飛 |
| `PieceType.GOLD` | 7 | 金 |
| `PieceType.KING` | 8 | 玉 |
| `PieceType.PRO_PAWN` | 9 | と |
| `PieceType.PRO_LANCE` | 10 | 成香 |
| `PieceType.PRO_KNIGHT` | 11 | 成桂 |
| `PieceType.PRO_SILVER` | 12 | 成銀 |
| `PieceType.HORSE` | 13 | 馬 |
| `PieceType.DRAGON` | 14 | 龍 |
| `PieceType.GOLD_LIKE` | 15 | 金相当の駒（内部用） |

### コンストラクタ

```python
PieceType(value: int | None = None) -> PieceType
```

- `value` を省略すると `NO_PIECE_TYPE` になります。
- 不正な値を渡すと `ValueError` が発生します。

### クラスメソッド

```python
PieceType.from_usi(usi: str) -> PieceType
```

USI 形式の文字列（`"P"`, `"L"`, `"N"`, `"S"`, `"B"`, `"R"`, `"G"`, `"K"` など）から生成します。

### インスタンスメソッド

| メソッド | 戻り値 | 説明 |
|----------|--------|------|
| `is_promotable()` | `bool` | 成れる駒かどうか |
| `is_promoted()` | `bool` | 成り駒かどうか |
| `promote()` | `PieceType` | 成り駒に変換 |
| `demote()` | `PieceType` | 素駒に戻す |
| `to_usi()` | `str` | USI 文字列に変換 |

### プロパティ

| プロパティ | 型 | 説明 |
|------------|-----|------|
| `value` | `int` | 内部値（0–15） |

### 特殊メソッド

- `int(piece_type)` → 内部値を返す
- `==` / `!=` で比較可能（`PieceType` または `int` と比較）

### 使用例

```python
import rsshogi

pt = rsshogi.types.PieceType.PAWN
print(pt.is_promotable())  # => True
print(pt.promote())        # => PieceType(PRO_PAWN)

# 成り駒判定
pro_pawn = rsshogi.types.PieceType.PRO_PAWN
print(pro_pawn.is_promoted())  # => True
print(pro_pawn.demote())       # => PieceType(PAWN)
```

---

## Piece

```python
rsshogi.types.Piece
```

色付きの駒を表す型です。

### コンストラクタ

```python
Piece(value: int | None = None) -> Piece
```

- `value` を省略すると駒なし（`NO_PIECE_TYPE` 相当）になります。
- 不正な値を渡すと `ValueError` が発生します。

### クラスメソッド

```python
Piece.from_color_type(color: Color, piece_type: PieceType) -> Piece
Piece.from_usi(usi: str) -> Piece
```

### インスタンスメソッド

| メソッド | 戻り値 | 説明 |
|----------|--------|------|
| `is_promoted()` | `bool` | 成り駒かどうか |
| `promote()` | `Piece` | 成り駒に変換 |
| `unpromoted_piece()` | `Piece` | 素駒に戻す |
| `base_piece_type()` | `PieceType` | 素駒の駒種 |

### プロパティ

| プロパティ | 型 | 説明 |
|------------|-----|------|
| `color` | `Color` | 駒の所有者 |
| `piece_type` | `PieceType` | 駒種 |
| `value` | `int` | 内部値 |

### 特殊メソッド

- `int(piece)` → 内部値を返す
- `==` / `!=` で比較可能（`Piece` または `int` と比較）

### 使用例

```python
import rsshogi

board = rsshogi.core.Board()
sq = rsshogi.types.Square.from_usi("7g")
piece = board.piece_on(sq)

print(piece.color)       # => Color(BLACK)
print(piece.piece_type)  # => PieceType(PAWN)

# 駒を作成
black_rook = rsshogi.types.Piece.from_color_type(
    rsshogi.types.Color.BLACK,
    rsshogi.types.PieceType.ROOK
)
```

---

## Square

```python
rsshogi.types.Square
```

盤上のマス目を表す型です。

### コンストラクタ

```python
Square(value: int | None = None) -> Square
```

- `value` を省略すると `NONE` になります。
- 不正な値を渡すと `ValueError` が発生します。

### クラス定数

| 定数 | 説明 |
|------|------|
| `Square.NONE` | 存在しないマス |

### クラスメソッド

```python
Square.from_usi(s: str) -> Square                      # "7g" など
Square.from_index(idx: int) -> Square                  # 0-80
Square.from_file_rank(file: int, rank: int) -> Square  # 0-based (0-8, 0-8)
```

`from_file_rank` は `Square.file` / `Square.rank` と往復できることを優先した API です（`file=0, rank=0` は `1a`、`file=6, rank=6` は `7g`）。人間向けの座標を直接書く場合は `Square.from_usi("7g")` を使ってください。

### インスタンスメソッド

| メソッド | 戻り値 | 説明 |
|----------|--------|------|
| `to_usi()` | `str` | USI 文字列（`"7g"`） |
| `is_valid()` | `bool` | 有効なマスかどうか |
| `is_none()` | `bool` | NONE かどうか |
| `mirror_file()` | `Square` | 左右反転 |
| `flip_side()` | `Square` | 先後反転（180度回転） |

### プロパティ

| プロパティ | 型 | 説明 |
|------------|-----|------|
| `value` | `int` | 内部値（0–80、NONE は -1） |
| `file` | `int` | 筋（0-8、0 = 1筋） |
| `rank` | `int` | 段（0-8、0 = 1段） |

### 特殊メソッド

- `int(square)` → 内部値を返す
- `==` / `!=` で比較可能（`Square` または `int` と比較）

### 使用例

```python
import rsshogi

# USI 文字列から作成
sq = rsshogi.types.Square.from_usi("5e")
print(sq.to_usi())  # => "5e"
print(sq.file)    # => 4（0-based: 5筋 → 4）
print(sq.rank)    # => 4（0-based: 5段 → 4）

# インデックスから作成
sq2 = rsshogi.types.Square.from_index(40)  # 中央
print(sq2.to_usi())

# 反転（180度回転）
print(sq.flip_side().to_usi())  # => 先後反転後のマス
```

---

## Bitboard

```python
rsshogi.types.Bitboard
```

81ビットのビットボードを表す型です。盤面上の駒の位置や攻撃範囲を効率的に表現します。

### コンストラクタ

```python
Bitboard() -> Bitboard
```

空のビットボード（`EMPTY` と同じ）を生成します。

### クラス定数

| 定数 | 説明 |
|------|------|
| `Bitboard.EMPTY` | 空のビットボード |
| `Bitboard.ALL` | 全マスがセットされたビットボード |

### クラスメソッド

```python
Bitboard.from_parts(low: int, high: int) -> Bitboard
Bitboard.from_square(square: Square) -> Bitboard
```

### インスタンスメソッド

| メソッド | 戻り値 | 説明 |
|----------|--------|------|
| `is_empty()` | `bool` | 空かどうか |
| `count()` | `int` | セットされたビット数 |
| `set(square)` | `None` | マスをセット |
| `clear(square)` | `None` | マスをクリア |
| `test(square)` | `bool` | マスがセットされているか |
| `flip_side()` | `Bitboard` | 先後反転 |
| `mirror_file()` | `Bitboard` | 左右反転 |
| `pop_lsb()` | `Square | None` | 最下位ビットを取り出し |

### プロパティ

| プロパティ | 型 | 説明 |
|------------|-----|------|
| `packed` | `int` | 128bit 整数表現 |
| `parts` | `tuple[int, int]` | 内部表現（low, high） |

### 特殊メソッド

- `int(bitboard)` → 128bit 整数表現を返す（`packed` と同値）

### 使用例

```python
import rsshogi

board = rsshogi.core.Board()

# 空きマスを取得
empties = board.empties()
print(empties.count())  # => 61（初期局面の空きマス数）

# 特定駒のビットボード
pawns = board.pieces_by_type(rsshogi.types.PieceType.PAWN)
print(pawns.count())  # => 18（先後合計）

# 攻撃範囲
sq = rsshogi.types.Square.from_usi("5e")
attackers = board.attackers_to(sq, board.pieces())
print(attackers.count())

# ビットボードをイテレート
bb = board.pieces_by_color(rsshogi.types.Color.BLACK)
while not bb.is_empty():
    sq = bb.pop_lsb()
    print(sq.to_usi())
```

---

## Hand

```python
rsshogi.types.Hand
```

持ち駒を表す型です。

### コンストラクタ

```python
Hand() -> Hand
```

空の持ち駒（`ZERO` と同じ）を生成します。

### クラス定数

| 定数 | 説明 |
|------|------|
| `Hand.ZERO` | 空の持ち駒 |

### インスタンスメソッド

| メソッド | 戻り値 | 説明 |
|----------|--------|------|
| `count(piece_type)` | `int` | 指定駒種の枚数 |
| `add(piece_type, amount=1)` | `None` | 駒を追加 |
| `sub(piece_type, amount=1)` | `None` | 駒を減らす |
| `is_empty()` | `bool` | 持ち駒がないか |

### プロパティ

| プロパティ | 型 | 説明 |
|------------|-----|------|
| `value` | `int` | 内部値 |

### 特殊メソッド

- `int(hand)` → 内部値を返す

### 使用例

```python
import rsshogi

board = rsshogi.core.Board()

# 先手の持ち駒を取得
hand = board.hand(rsshogi.types.Color.BLACK)

# 各駒種の枚数を確認
for pt in [
    rsshogi.types.PieceType.PAWN,
    rsshogi.types.PieceType.LANCE,
    rsshogi.types.PieceType.KNIGHT,
    rsshogi.types.PieceType.SILVER,
    rsshogi.types.PieceType.GOLD,
    rsshogi.types.PieceType.BISHOP,
    rsshogi.types.PieceType.ROOK,
]:
    count = hand.count(pt)
    if count > 0:
        print(f"{pt}: {count}枚")
```

---

## MoveType

```python
rsshogi.types.MoveType
```

指し手の種類を表す型です。

### コンストラクタ

```python
MoveType() -> MoveType
```

`NORMAL` を生成します。

### クラス定数

| 定数 | 説明 |
|------|------|
| `MoveType.NORMAL` | 通常の移動 |
| `MoveType.PROMOTION` | 成る手 |
| `MoveType.DROP` | 駒打ち |

### インスタンスメソッド

| メソッド | 戻り値 | 説明 |
|----------|--------|------|
| `is_normal()` | `bool` | 通常の移動か |
| `is_promotion()` | `bool` | 成る手か |
| `is_drop()` | `bool` | 駒打ちか |

### 特殊メソッド

- `==` / `!=` で比較可能（`MoveType` 同士）

### 使用例

```python
import rsshogi

board = rsshogi.core.Board()

for move in board.legal_moves():
    mt = move.move_type
    if mt == rsshogi.types.MoveType.DROP:
        print(f"{move.to_usi()} は駒打ち")
    elif mt == rsshogi.types.MoveType.PROMOTION:
        print(f"{move.to_usi()} は成る手")
```

---

## RepetitionState

```python
rsshogi.types.RepetitionState
```

千日手の状態を表す型です。

### コンストラクタ

```python
RepetitionState() -> RepetitionState
```

`NONE`（千日手ではない）を生成します。

### クラス定数

| 定数 | USI文字列 | 説明 |
|------|-----------|------|
| `RepetitionState.NONE` | `"rep_none"` | 千日手ではない |
| `RepetitionState.WIN` | `"rep_win"` | 連続王手の千日手で勝ち |
| `RepetitionState.LOSE` | `"rep_lose"` | 連続王手の千日手で負け |
| `RepetitionState.DRAW` | `"rep_draw"` | 通常の千日手（引き分け） |
| `RepetitionState.SUPERIOR` | `"rep_sup"` | 優等局面 |
| `RepetitionState.INFERIOR` | `"rep_inf"` | 劣等局面 |

### インスタンスメソッド

| メソッド | 戻り値 | 説明 |
|----------|--------|------|
| `to_usi()` | `str` | USI 拡張文字列 |

### 特殊メソッド

- `==` / `!=` で比較可能（`RepetitionState` 同士）

### 使用例

```python
import rsshogi

board = rsshogi.core.Board()

# 千日手状態を取得
state = board.repetition_state()

if state == rsshogi.types.RepetitionState.DRAW:
    print("千日手引き分け")
elif state == rsshogi.types.RepetitionState.WIN:
    print("連続王手の千日手で勝ち")
elif state == rsshogi.types.RepetitionState.LOSE:
    print("連続王手の千日手で負け")

# USI 文字列として取得
print(state.to_usi())  # => "rep_none" など
```

---

## 関連項目

- [`Board`](board.md) - 盤面管理クラス
- [`Move32`](move32.md) - 32bit 指し手型
- [`Move`](move.md) - 16bit 指し手型
- [`GameResult`](game_result.md) - 終局結果
