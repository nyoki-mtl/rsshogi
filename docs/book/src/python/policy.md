# Policy ラベル（`rsshogi.policy`）

`rsshogi.policy` モジュールは、方策ネットワーク（Policy Network）の分類出力層で用いる指し手ラベル変換を提供する。
移動先 81 マス × 移動方向、成り、駒打ち 27 通りの 2187 クラス表現と、将棋盤の構造上出現しえない手を除いた 1496 クラスの圧縮表現を相互に変換できる。

```python
from rsshogi.policy import (
    COMPACT_MOVE_LABEL_COUNT,
    COMPACT_TO_MOVE_LABEL,
    MOVE_LABEL_COUNT,
    MOVE_LABEL_TO_COMPACT,
    compact_move_label,
    compact_move_label_to_move_label,
    is_structurally_valid_move_label,
    move_label,
    move_label_to_compact,
)
```

## 定数一覧

| 定数 | 型 | 値 / 要素数 | 説明 |
|---|---|---|---|
| `MOVE_LABEL_COUNT` | `int` | `2187` | 非圧縮 policy ラベルの総クラス数（81 マス × 27 通りの方向と駒打ち）である。 |
| `COMPACT_MOVE_LABEL_COUNT` | `int` | `1496` | 構造的に出現しうる policy ラベルの総クラス数である。 |
| `MOVE_LABEL_TO_COMPACT` | `tuple[int \| None, ...]` | 長さ `2187` | 2187 クラスのラベル `raw` をインデックスとし、対応する圧縮ラベル（`0..1496`）または構造的に無効な場合の `None` を格納するタプルである。 |
| `COMPACT_TO_MOVE_LABEL` | `tuple[int, ...]` | 長さ `1496` | 1496 クラスの圧縮ラベル `compact` をインデックスとし、対応する 2187 クラスのラベル（`0..2187`）を格納するタプルである。 |

## 関数一覧

| 関数 | シグネチャ | 説明 |
|---|---|---|
| `move_label` | `move_label(move: Move \| Move32 \| int \| str, turn: Color) -> int` | 指し手と手番から `0..2187` の policy ラベルを返す。 |
| `compact_move_label` | `compact_move_label(move: Move \| Move32 \| int \| str, turn: Color) -> int \| None` | 指し手と手番から `0..1496` の圧縮ラベルを返す。構造的に無効な手の場合は `None` を返す。 |
| `move_label_to_compact` | `move_label_to_compact(label: int) -> int \| None` | `0..2187` のラベルを `0..1496` の圧縮ラベルに変換する。構造的に無効なラベルでは `None` を返す。 |
| `compact_move_label_to_move_label` | `compact_move_label_to_move_label(label: int) -> int` | `0..1496` の圧縮ラベルを `0..2187` のラベルへ展開する。 |
| `is_structurally_valid_move_label` | `is_structurally_valid_move_label(label: int) -> bool` | `0..2187` のラベルが構造的に出現しうる手であれば `True` を返す。 |

### 引数と例外条件

- **`move` 引数**：`Move`、`Move32`、`0..=65535` の `int`（16 ビット `Move` の内部値）、または USI 文字列（`"7g7f"`、`"P*5e"` など）を直接渡せる。
  - 上記以外の型を渡した場合は `TypeError` を送出する。
  - `int` が `0..=65535` の範囲外である場合、または USI 文字列がパースできない場合は `ValueError` を送出する。
  - `Move.MOVE_NONE` などの特殊手や、8 方向および桂馬跳び以外の不正な移動ベクトルを持ち policy ラベルへエンコードできない指し手を渡した場合は `ValueError` を送出する。
- **`turn` 引数**：[`Color`](types.md#color) インスタンス（`Color.BLACK` または `Color.WHITE`）を渡す。後手番（`Color.WHITE`）を指定すると、移動元と移動先のマスを盤面 180 度回転して先手視点に正規化してからラベルを計算する。
- **`label` 引数**：
  - `move_label_to_compact(label)` と `is_structurally_valid_move_label(label)` に `0..2187` の範囲外の整数を渡すと `ValueError`（負数や `u16` 範囲外は `OverflowError`）を送出する。
  - `compact_move_label_to_move_label(label)` に `0..1496` の範囲外の整数を渡すと `ValueError` を送出する。

## 使用例

### 基本的なラベル変換と後手番の視点正規化

```python
from rsshogi.core import Move
from rsshogi.policy import compact_move_label, move_label
from rsshogi.types import Color

mv = Move.from_usi("7g7f")
raw = move_label(mv, Color.BLACK)
compact = compact_move_label(mv, Color.BLACK)
assert compact is not None

# USI 文字列を直接渡すこともでき、先手の 7g7f と後手の 3c3d は同じラベルになる
assert move_label("7g7f", Color.BLACK) == raw
assert move_label("3c3d", Color.WHITE) == raw
assert compact_move_label("3c3d", Color.WHITE) == compact
```

### 構造的に無効なラベルの判定

一段目への歩や香や桂の駒打ち、盤端から外れる移動方向、桂馬の 1 段目および 2 段目への不成などは、いかなる合法局面でも出現しない。
これらの手は 2187 クラスの `move_label()` では数値に変換されるが、`is_structurally_valid_move_label()` は `False`、`compact_move_label()` と `move_label_to_compact()` は `None` を返す。

```python
from rsshogi.policy import (
    COMPACT_TO_MOVE_LABEL,
    MOVE_LABEL_TO_COMPACT,
    compact_move_label,
    compact_move_label_to_move_label,
    is_structurally_valid_move_label,
    move_label,
    move_label_to_compact,
)
from rsshogi.types import Color

# 一段目への歩打ち（P*5a）は構造的に出現しない
invalid_raw = move_label("P*5a", Color.BLACK)
assert not is_structurally_valid_move_label(invalid_raw)
assert compact_move_label("P*5a", Color.BLACK) is None
assert move_label_to_compact(invalid_raw) is None
assert MOVE_LABEL_TO_COMPACT[invalid_raw] is None

# 有効な手の往復変換
valid_raw = move_label("7g7f", Color.BLACK)
valid_compact = move_label_to_compact(valid_raw)
assert valid_compact is not None
assert compact_move_label_to_move_label(valid_compact) == valid_raw
assert COMPACT_TO_MOVE_LABEL[valid_compact] == valid_raw
```

## 関連項目

- [指し手クラス（`Move` / `Move32`）](move.md)
- [基本型（`Color`）](types.md#color)
- [SAZ2 自己対局教師データ](sazpack.md)
