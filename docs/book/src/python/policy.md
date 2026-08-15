# Policy ラベル

policy 学習向けの move label 変換を提供するモジュールです。

## インポート

```python
from rsshogi.policy import (
    MOVE_LABEL_COUNT,
    COMPACT_MOVE_LABEL_COUNT,
    MOVE_LABEL_TO_COMPACT,
    COMPACT_TO_MOVE_LABEL,
    move_label,
    compact_move_label,
    move_label_to_compact,
    compact_move_label_to_move_label,
    is_structurally_valid_move_label,
)
```

## 概要

- `move_label(move, turn)` は 2187 クラスの label を返します
- `compact_move_label(move, turn)` は 1496 クラスへ圧縮します
- `MOVE_LABEL_TO_COMPACT` / `COMPACT_TO_MOVE_LABEL` でテーブル変換できます
- ここでの `compact` は「局面で合法」ではなく「構造的に現れうる」ラベルだけを残す意味です

`move` 引数には次を渡せます。

- `Move`
- `Move32`
- `int`
- USI 文字列

`turn` は `Color` を渡します。

## 基本例

```python
from rsshogi.core import Move
from rsshogi.policy import compact_move_label, move_label
from rsshogi.types import Color

mv = Move.from_usi("7g7f")

label = move_label(mv, Color.BLACK)
compact = compact_move_label(mv, Color.BLACK)

assert compact is not None
print(label)
print(compact)
```

## 白番の正規化

後手番の手は 180 度回転してからラベル化されます。
そのため、対称な手は同じラベルになります。

```python
from rsshogi.policy import move_label
from rsshogi.types import Color

assert move_label("7g7f", Color.BLACK) == move_label("3c3d", Color.WHITE)
```

## 構造的に無効なラベル

たとえば一段目への歩打ちや最上段への桂不成は、どの局面でも構造的に現れません。
このような手は 2187 クラスの `move_label` には写りますが、`compact_move_label` では `None` になります。

```python
from rsshogi.policy import compact_move_label, is_structurally_valid_move_label, move_label
from rsshogi.types import Color

label = move_label("P*5a", Color.BLACK)

assert not is_structurally_valid_move_label(label)
assert compact_move_label("P*5a", Color.BLACK) is None
```

## テーブル変換

```python
from rsshogi.policy import (
    COMPACT_TO_MOVE_LABEL,
    MOVE_LABEL_TO_COMPACT,
    compact_move_label,
    move_label,
)
from rsshogi.types import Color

raw = move_label("7g7f", Color.BLACK)
compact = compact_move_label("7g7f", Color.BLACK)

assert compact is not None
assert MOVE_LABEL_TO_COMPACT[raw] == compact
assert COMPACT_TO_MOVE_LABEL[compact] == raw
```

## 関連項目

- [Move](move.md) / [Move32](move32.md) - 指し手型
- [補助型](types.md#color) - `Color`
- [cshogi との API 対応表](../reference/compat.md) - 互換性の考え方
- Rust core の対応 API: `rsshogi::labels::policy`
