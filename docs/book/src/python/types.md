# 基本型（`rsshogi.types`）

`rsshogi.types` モジュールは、手番、駒種、色付き駒、マス目、ビットボード、持ち駒、指し手種別、千日手状態を表す 8 つのクラスを提供する。
このうち `Color`、`PieceType`、`Piece`、`Square` の 4 クラスは `rsshogi.core` からも同じ型としてインポートできる。

```python
from rsshogi.types import (
    Bitboard,
    Color,
    Hand,
    MoveType,
    Piece,
    PieceType,
    RepetitionState,
    Square,
)
```

## Color

```python
rsshogi.types.Color(value: int | None = None) -> Color
```

手番（先手または後手）を表す型である。
`value` を省略または `None` を渡すと `Color.BLACK`（`0`）になる。
`0` と `1` 以外の整数を渡すと `ValueError` を送出する。

### 定数

| 定数 | 内部値 (`value`) | `str(color)` | 説明 |
|---|---|---|---|
| `Color.BLACK` | `0` | `"black"` | 先手（下手）を表す。 |
| `Color.WHITE` | `1` | `"white"` | 後手（上手）を表す。 |

### メソッドとプロパティ

| メソッド / プロパティ | シグネチャ | 説明 |
|---|---|---|
| `from_usi` | `Color.from_usi(usi: str) -> Color` | `"b"`、`"w"`、`"black"`、`"white"`（大文字小文字不問）から生成する。それ以外は `ValueError` を送出する。 |
| `flip` | `color.flip() -> Color` | 手番を反転した `Color` を返す（`BLACK` なら `WHITE`）。 |
| `opponent` | `color.opponent() -> Color` | 相手の手番を返す（`flip()` と同義）。 |
| `is_black` | `color.is_black() -> bool` | 先手であれば `True` を返す。 |
| `is_white` | `color.is_white() -> bool` | 後手であれば `True` を返す。 |
| `value` | `int` | 内部整数値（`0` または `1`）を返す。 |

### 特殊メソッド

- `int(color)`：内部値（`0` または `1`）を返す。
- `str(color)`：`"black"` または `"white"` を返す。
- `repr(color)`：`"Color(black)"` または `"Color(white)"` を返す。
- `hash(color)`：ハッシュ値を返す。
- `color == other` / `color != other`：`Color` または `int` と等価比較を行う。

```python
from rsshogi.types import Color

c = Color.from_usi("b")
assert c == Color.BLACK
assert c.opponent() == Color.WHITE
assert str(c) == "black"
```

## PieceType

```python
rsshogi.types.PieceType(value: int | None = None) -> PieceType
```

手番を持たない駒の種類を表す型である。
`value` を省略または `None` を渡すと `PieceType.NO_PIECE_TYPE`（`0`）になる。
`0..=14` 以外の整数を渡すと `ValueError` を送出する。

### 定数

| 定数 | 値 | `to_usi()` / `str()` | 説明 |
|---|---|---|---|
| `PieceType.NO_PIECE_TYPE` | `0` | `""` | 駒なしを表す。 |
| `PieceType.PAWN` | `1` | `"P"` | 歩を表す。 |
| `PieceType.LANCE` | `2` | `"L"` | 香を表す。 |
| `PieceType.KNIGHT` | `3` | `"N"` | 桂を表す。 |
| `PieceType.SILVER` | `4` | `"S"` | 銀を表す。 |
| `PieceType.BISHOP` | `5` | `"B"` | 角を表す。 |
| `PieceType.ROOK` | `6` | `"R"` | 飛を表す。 |
| `PieceType.GOLD` | `7` | `"G"` | 金を表す。 |
| `PieceType.KING` | `8` | `"K"` | 玉を表す。 |
| `PieceType.PRO_PAWN` | `9` | `"+P"` | と金を表す。 |
| `PieceType.PRO_LANCE` | `10` | `"+L"` | 成香を表す。 |
| `PieceType.PRO_KNIGHT` | `11` | `"+N"` | 成桂を表す。 |
| `PieceType.PRO_SILVER` | `12` | `"+S"` | 成銀を表す。 |
| `PieceType.HORSE` | `13` | `"+B"` | 馬を表す。 |
| `PieceType.DRAGON` | `14` | `"+R"` | 龍を表す。 |
| `PieceType.GOLD_LIKE` | `15` | `"G"` | 金相当の駒を表す内部検索用定数である。 |

### メソッドとプロパティ

| メソッド / プロパティ | シグネチャ | 説明 |
|---|---|---|
| `from_usi` | `PieceType.from_usi(usi: str) -> PieceType` | USI 駒種文字列（`"P"`、`"p"`、`"+P"`、`"+r"` など大文字小文字両対応）から生成する。不正な文字列は `ValueError` を送出する。 |
| `to_usi` | `pt.to_usi() -> str` | 大文字の USI 駒種文字列（`"P"`、`"+B"` など）を返す。 |
| `is_promotable` | `pt.is_promotable() -> bool` | 成ることができる生駒（歩、香、桂、銀、角、飛）であれば `True` を返す。 |
| `is_promoted` | `pt.is_promoted() -> bool` | 成り駒（と、成香、成桂、成銀、馬、龍）であれば `True` を返す。 |
| `promote` | `pt.promote() -> PieceType` | 成り駒に変換する（成れない駒種はそのまま返す）。 |
| `demote` | `pt.demote() -> PieceType` | 成り駒を生駒に戻す（生駒はそのまま返す）。 |
| `value` | `int` | 内部整数値を返す。 |

### 特殊メソッド

- `int(pt)`：内部整数値を返す。
- `str(pt)`：`pt.to_usi()` と同じ文字列を返す。
- `repr(pt)`：`PieceType(P)` のような文字列を返す。
- `hash(pt)`：ハッシュ値を返す。
- `pt == other` / `pt != other`：`PieceType` または `int` と等価比較を行う。

```python
from rsshogi.types import PieceType

pt = PieceType.from_usi("b")
assert pt == PieceType.BISHOP
assert pt.is_promotable()
assert pt.promote() == PieceType.HORSE
assert pt.promote().demote() == PieceType.BISHOP
```

## Piece

```python
rsshogi.types.Piece(value: int | None = None) -> Piece
```

手番（所有者）と駒種を組み合わせた色付き駒を表す型である。
`value` を省略または `None` を渡すと駒なし（`value == 0`、`piece_type == PieceType.NO_PIECE_TYPE`）を生成する。
無効な内部整数値（`15`、`16`、負数、`31` 超など）を渡すと `ValueError` を送出する。

### 内部整数値と USI 表記

| 所有者 | 生駒（歩..玉） | 成り駒（と..龍） |
|---|---|---|
| 駒なし | `0`（`str(piece) == ""`） | - |
| 先手（`Color.BLACK`） | `1..=8`（`"P"`, `"L"`, `"N"`, `"S"`, `"B"`, `"R"`, `"G"`, `"K"`） | `9..=14`（`"+P"`, `"+L"`, `"+N"`, `"+S"`, `"+B"`, `"+R"`） |
| 後手（`Color.WHITE`） | `17..=24`（`"p"`, `"l"`, `"n"`, `"s"`, `"b"`, `"r"`, `"g"`, `"k"`） | `25..=30`（`"+p"`, `"+l"`, `"+n"`, `"+s"`, `"+b"`, `"+r"`） |

### メソッドとプロパティ

| メソッド / プロパティ | シグネチャ | 説明 |
|---|---|---|
| `from_color_type` | `Piece.from_color_type(color: Color, piece_type: PieceType) -> Piece` | 手番と駒種から色付き駒を生成する。 |
| `from_usi` | `Piece.from_usi(usi: str) -> Piece` | USI 駒文字列から生成する。大文字（`"P"`、`"+P"`）は先手駒、小文字（`"p"`、`"+p"`）は後手駒になる。不正な文字列は `ValueError` を送出する。 |
| `color` | `Color` | 駒の所有者の手番を返す。 |
| `piece_type` | `PieceType` | 駒種（成り状態を含む）を返す。 |
| `is_promoted` | `piece.is_promoted() -> bool` | 成り駒であれば `True` を返す。 |
| `promote` | `piece.promote() -> Piece` | 同じ手番のまま成り駒に変換する。 |
| `unpromoted_piece` | `piece.unpromoted_piece() -> Piece` | 同じ手番のまま生駒に戻した `Piece` を返す。 |
| `base_piece_type` | `piece.base_piece_type() -> PieceType` | 生駒に戻した `PieceType` を返す。 |
| `value` | `int` | 内部整数値を返す。 |

### 特殊メソッド

- `int(piece)`：内部整数値を返す。
- `str(piece)`：USI 形式の駒文字列（`"P"`、`"+p"` など、駒なしは `""`）を返す。
- `repr(piece)`：`Piece(+p)` のような文字列を返す。
- `hash(piece)`：ハッシュ値を返す。
- `piece == other` / `piece != other`：`Piece` または `int` と等価比較を行う。

```python
from rsshogi.types import Color, Piece, PieceType

white_horse = Piece.from_usi("+b")
assert white_horse.color == Color.WHITE
assert white_horse.piece_type == PieceType.HORSE
assert white_horse.base_piece_type() == PieceType.BISHOP
assert white_horse.unpromoted_piece() == Piece.from_color_type(Color.WHITE, PieceType.BISHOP)
```

## Square

```python
rsshogi.types.Square(value: int | None = None) -> Square
```

盤上の 81 マス（`0..=80`）または盤外を表す `Square.NONE`（`-1`）を保持する型である。
`value` を省略または `None` を渡すと `Square.NONE` になる。
`-1..=80` 以外の整数を渡すと `ValueError` を送出する。

### 座標系とインデックス

`Square` の内部インデックスは `file * 9 + rank` で定義される。
`file`（筋）と `rank`（段）はいずれも **0 始まり（`0..=8`）** である。
- `file = 0` が 1 筋、`file = 6` が 7 筋、`file = 8` が 9 筋に対応する。
- `rank = 0` が一（`a`）段、`rank = 6` が七（`g`）段、`rank = 8` が九（`i`）段に対応する。
- したがって `1a` は `index = 0`（`file = 0, rank = 0`）、`7g` は `index = 60`（`file = 6, rank = 6`）、`9i` は `index = 80`（`file = 8, rank = 8`）となる。

将棋の一般的な筋と段の表記を直接指定したい場合は `Square.from_usi("7g")` を使うと直感的である。

### 定数、メソッド、プロパティ

| 定数 / メソッド / プロパティ | シグネチャ | 説明 |
|---|---|---|
| `Square.NONE` | `Square` | 存在しないマス（`value == -1`）を表すクラス定数である。 |
| `from_usi` | `Square.from_usi(usi: str) -> Square` | `"7g"` や `"5e"` などの USI 座標文字列から生成する。不正な文字列は `ValueError` を送出する。 |
| `from_index` | `Square.from_index(idx: int) -> Square` | `0..=80` のインデックスから盤上マスを生成する。範囲外は `ValueError`（負数は `OverflowError`）を送出する。 |
| `from_file_rank` | `Square.from_file_rank(file: int, rank: int) -> Square` | 0 始まりの筋（`0..=8`）と段（`0..=8`）から生成する。範囲外は `ValueError` を送出する。 |
| `to_usi` | `sq.to_usi() -> str` | USI 文字列（`"7g"` など、`Square.NONE` は `"none"`）を返す。 |
| `is_valid` | `sq.is_valid() -> bool` | 有効な値（`Square.NONE` を含む）であれば `True` を返す。盤上判定には `not sq.is_none()` を使用する。 |
| `is_none` | `sq.is_none() -> bool` | `Square.NONE` であれば `True` を返す。 |
| `mirror_file` | `sq.mirror_file() -> Square` | 筋を左右反転したマスを返す（例：`7g` から `3g`）。 |
| `flip_side` | `sq.flip_side() -> Square` | 盤面を 180 度回転したマスを返す（例：`7g` から `3c`）。 |
| `value` | `int` | 内部整数値（`0..=80`、または `-1`）を返す。 |
| `file` | `int` | 0 始まりの筋（`0..=8`）を返す。 |
| `rank` | `int` | 0 始まりの段（`0..=8`）を返す。 |

### 特殊メソッド

- `int(sq)`：内部整数値を返す。
- `str(sq)`：`sq.to_usi()` と同じ文字列を返す。
- `repr(sq)`：`Square(7g)` のような文字列を返す。
- `hash(sq)`：ハッシュ値を返す。
- `sq == other` / `sq != other`：`Square` または `int` と等価比較を行う。

```python
from rsshogi.types import Square

sq = Square.from_usi("7g")
assert sq.file == 6 and sq.rank == 6
assert Square.from_file_rank(sq.file, sq.rank) == sq
assert sq.mirror_file().to_usi() == "3g"
assert sq.flip_side().to_usi() == "3c"
```

## Bitboard

```python
rsshogi.types.Bitboard() -> Bitboard
```

盤上の 81 マスの部分集合を保持するビットボード型である。
内部的には 2 つの 64 ビット整数（`low` に 1 筋から 7 筋の 63 マス、`high` に 8 筋と 9 筋の 18 マス）で表現される。
引数なしコンストラクタ `Bitboard()` は空のビットボードを生成する。

### 定数、メソッド、プロパティ

| 定数 / メソッド / プロパティ | シグネチャ | 説明 |
|---|---|---|
| `Bitboard.EMPTY` | `Bitboard` | 全マスが 0 の空ビットボード定数である。 |
| `Bitboard.ALL` | `Bitboard` | 全 81 マスが 1 にセットされたビットボード定数である。 |
| `from_parts` | `Bitboard.from_parts(low: int, high: int) -> Bitboard` | 下位 64 ビット（bit `0..=62` 有効）と上位 64 ビット（bit `0..=17` 有効）から生成する。 |
| `from_square` | `Bitboard.from_square(square: Square) -> Bitboard` | 指定した 1 マスのみがセットされたビットボードを生成する。 |
| `is_empty` | `bb.is_empty() -> bool` | セットされたマスが 0 個であれば `True` を返す。 |
| `count` | `bb.count() -> int` | セットされているマスの個数（popcount）を返す。 |
| `set` | `bb.set(square: Square) -> None` | 指定したマスのビットを 1 に変更する（破壊的変更）。 |
| `clear` | `bb.clear(square: Square) -> None` | 指定したマスのビットを 0 に変更する（破壊的変更）。 |
| `test` | `bb.test(square: Square) -> bool` | 指定したマスのビットが 1 であれば `True` を返す。 |
| `pop_lsb` | `bb.pop_lsb() -> Square \| None` | 最小インデックスの 1 ビットを取り除き、その `Square` を返す。空の場合は `None` を返す。 |
| `flip_side` | `bb.flip_side() -> Bitboard` | 180 度回転した新しい `Bitboard` を返す。 |
| `mirror_file` | `bb.mirror_file() -> Bitboard` | 左右反転した新しい `Bitboard` を返す。 |
| `packed` | `int` | `(high << 63) \| low` に相当する 81 ビットパック整数を返す。 |
| `parts` | `tuple[int, int]` | 内部表現の `(low, high)` タプルを返す。 |

### 特殊メソッド

- `int(bb)`：`bb.packed` と同じ整数値を返す。
- `str(bb)`：9×9 の盤面視覚化文字列を返す。
- `repr(bb)`：`Bitboard(0x...)` 形式の 16 進文字列を返す。

```python
from rsshogi.types import Bitboard, Square

bb = Bitboard()
bb.set(Square.from_usi("7g"))
bb.set(Square.from_usi("3c"))
assert bb.count() == 2
assert bb.test(Square.from_usi("7g"))

squares = []
while not bb.is_empty():
    sq = bb.pop_lsb()
    if sq is not None:
        squares.append(sq.to_usi())
assert squares == ["3c", "7g"]
```

## Hand

```python
rsshogi.types.Hand() -> Hand
```

片方の手番の持ち駒枚数を 32 ビット整数にパックして保持する型である。
引数なしコンストラクタ `Hand()` は持ち駒なし（`Hand.ZERO`）を生成する。

### 対象となる駒種と例外

持ち駒として扱える `PieceType` は生駒 7 種（`PAWN`、`LANCE`、`KNIGHT`、`SILVER`、`GOLD`、`BISHOP`、`ROOK`）のみである。
`count()`、`add()`、`sub()` に `PieceType.KING`、成り駒（`PRO_PAWN`..`DRAGON`）、`NO_PIECE_TYPE`、`GOLD_LIKE` を渡すと `ValueError` を送出する。

### 定数、メソッド、プロパティ

| 定数 / メソッド / プロパティ | シグネチャ | 説明 |
|---|---|---|
| `Hand.ZERO` | `Hand` | すべての持ち駒が 0 枚の定数である。 |
| `count` | `hand.count(piece_type: PieceType) -> int` | 指定した駒種の所持枚数を返す。 |
| `add` | `hand.add(piece_type: PieceType, amount: int \| None = 1) -> None` | 指定した駒種を `amount` 枚増やす（破壊的変更）。 |
| `sub` | `hand.sub(piece_type: PieceType, amount: int \| None = 1) -> None` | 指定した駒種を `amount` 枚減らす（破壊的変更）。 |
| `is_empty` | `hand.is_empty() -> bool` | 持ち駒が 1 枚もなければ `True` を返す。 |
| `value` | `int` | 内部の 32 ビットパック整数値を返す。 |

### 特殊メソッド

- `int(hand)`：`hand.value` と同じ内部整数値を返す。
- `str(hand)`：`0x...` 形式の 16 進文字列を返す。
- `repr(hand)`：`Hand(0x...)` 形式の文字列を返す。

```python
from rsshogi.types import Hand, PieceType

hand = Hand()
hand.add(PieceType.PAWN, 2)
hand.add(PieceType.ROOK, 1)
assert hand.count(PieceType.PAWN) == 2
assert not hand.is_empty()
hand.sub(PieceType.PAWN, 1)
assert hand.count(PieceType.PAWN) == 1
```

## MoveType

```python
rsshogi.types.MoveType() -> MoveType
```

指し手の種類（通常移動、成り、駒打ち）を表す列挙型クラスである。
引数なしコンストラクタ `MoveType()` は `MoveType.NORMAL` を返す。

### 定数とメソッド

| 定数 | `str(mt)` | `is_normal()` | `is_promotion()` | `is_drop()` | 説明 |
|---|---|---|---|---|---|
| `MoveType.NORMAL` | `"normal"` | `True` | `False` | `False` | 盤上の成らない移動を表す。 |
| `MoveType.PROMOTION` | `"promotion"` | `False` | `True` | `False` | 盤上の成る移動を表す。 |
| `MoveType.DROP` | `"drop"` | `False` | `False` | `True` | 持ち駒を打つ手を表す。 |

### 特殊メソッド

- `str(mt)`：`"normal"`、`"promotion"`、`"drop"` のいずれかを返す。
- `repr(mt)`：`"MoveType.NORMAL"`、`"MoveType.PROMOTION"`、`"MoveType.DROP"` を返す。
- `hash(mt)`：ハッシュ値を返す。
- `mt == other` / `mt != other`：`MoveType` 同士の等価比較を行う（他の型との比較は `TypeError`）。

## RepetitionState

```python
rsshogi.types.RepetitionState() -> RepetitionState
```

`Board.repetition_state()` が返す千日手および優等局面や劣等局面の判定結果型である。
引数なしコンストラクタ `RepetitionState()` は `RepetitionState.NONE` を返す。

### 定数とメソッド

| 定数 | `to_usi()` / `str(state)` | 説明 |
|---|---|---|
| `RepetitionState.NONE` | `"rep_none"` | 千日手や同一局面の反復ではない通常の状態を表す。 |
| `RepetitionState.WIN` | `"rep_win"` | 相手による連続王手の千日手が成立し、現在手番側の反則勝ちとなる状態を表す。 |
| `RepetitionState.LOSE` | `"rep_lose"` | 自分による連続王手の千日手が成立し、現在手番側の反則負けとなる状態を表す。 |
| `RepetitionState.DRAW` | `"rep_draw"` | 通常の千日手（引き分け）を表す。 |
| `RepetitionState.SUPERIOR` | `"rep_sup"` | 盤面配置が同一で持ち駒が一方的に増えた優等局面を表す。 |
| `RepetitionState.INFERIOR` | `"rep_inf"` | 盤面配置が同一で持ち駒が一方的に減った劣等局面を表す。 |

### 特殊メソッド

- `state.to_usi()` / `str(state)`：上表の USI 拡張文字列を返す。
- `repr(state)`：`RepetitionState(rep_draw)` のような文字列を返す。
- `hash(state)`：ハッシュ値を返す。
- `state == other` / `state != other`：`RepetitionState` 同士の等価比較を行う（他の型との比較は `TypeError`）。

## 関連項目

- [`Board`](board.md)
- [指し手クラス（`Move` / `Move32`）](move.md)
