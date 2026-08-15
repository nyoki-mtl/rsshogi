# Apery / hcpe 互換

`rsshogi` は通常の `Move` / `Move32` に加えて、`cshogi` / Apery 互換の
`AperyMove` / `AperyMove32` と、`HuffmanCodedPos` / `HuffmanCodedPosAndEval`
を Python API から利用できます。

このページは、既存の `cshogi` 学習データや `hcpe` ファイルを扱うときの入口です。

## 対応している型

```python
from rsshogi.core import Board, Move, Move32, AperyMove, AperyMove32
from rsshogi.numpy import HuffmanCodedPos, HuffmanCodedPosAndEval
```

| 型 | サイズ | 主な用途 |
|----|--------|----------|
| `Move` | 16bit | rsshogi 標準の軽量指し手 |
| `Move32` | 32bit | rsshogi 標準の拡張指し手 |
| `AperyMove` | 16bit | `cshogi` / Apery 互換の move16 |
| `AperyMove32` | 32bit | `cshogi` / Apery 互換の 32bit move |

## `AperyMove`

`AperyMove` は `cshogi.move16(...)` と同じ 16bit 形式です。

- `to`: 下位 7bit
- `from`: 次の 7bit
- 成り: bit 14
- 駒打ち: `from >= 81`

```python
from rsshogi.core import Move

mv = Move.from_usi("7g7f")
amv = mv.to_apery()

print(int(amv))
print(amv.to_usi())
print(amv.to_move().to_usi())
```

## `AperyMove32`

`AperyMove32` は `AperyMove` に加えて、次の `PieceType` 情報を持ちます。

- 移動前の駒種
- 取得駒の駒種

```python
from rsshogi.core import Board, Move

board = Board()
mv = Move.from_usi("7g7f")
amv32 = board.apery_move32_from_move(mv)

print(amv32.to_usi())
print(amv32.piece_type_before)
print(amv32.captured_piece_type)
```

### `AperyMove32.to_move32(board)` が局面依存な理由

`AperyMove32` が保持するのは `PieceType` であり、`rsshogi.Move32` が保持する
「色付きの移動後の駒 (`Piece`)」ではありません。

そのため `AperyMove32 -> Move32` の復元には現在局面が必要です。

```python
restored = amv32.to_move32(board)
print(restored.to_usi())
```

`Move32` から `AperyMove32` へ変換するときも、取得駒種の補完のために `Board` を使います。

```python
move32 = board.legal_moves_move32()[0]
amv32 = move32.to_apery(board)
```

## `Board` から使う API

`Board` には cshogi/Apery 互換の補助 API があります。

```python
board.to_hcp(out=None)
board.set_hcp(hcp, game_ply=1)
board.to_hcpe(best_move=None, score=0, game_result=..., out=None)
board.apery_move32_from_move(move)
board.apery_move32_from_move32(move32)
```

## HCP / HCPE

### `HuffmanCodedPos`

`HuffmanCodedPos` は 32 バイト固定長の局面形式です。

```python
board = Board()
hcp = board.to_hcp()
board.set_hcp(hcp)
```

`out` を渡すと `bytes` を返さず、既存バッファへ直接書き込みます。

```python
import numpy as np
from rsshogi.numpy import HuffmanCodedPos

out = np.zeros(1, dtype=HuffmanCodedPos)
board.to_hcp(out)
board.set_hcp(out)
```

### `HuffmanCodedPosAndEval`

`hcpe` は HCP に評価値と `bestMove16` を足した 38 バイト固定長の学習データです。

```python
hcpe = board.to_hcpe(
    best_move="7g7f",
    score=42,
    game_result="BLACK_WIN",
)
```

`best_move` には次を渡せます。

- `AperyMove`
- `AperyMove32`
- `Move`
- `Move32`
- `int`
- USI 文字列

`game_result` は `Draw` / `BlackWin` / `WhiteWin` に対応する値だけを受け付けます。
未指定の `None` は受け付けません。

## NumPy dtype

NumPy を使う場合は次の dtype が利用できます。

```python
from rsshogi.numpy import HuffmanCodedPos, HuffmanCodedPosAndEval
```

```python
import numpy as np
from rsshogi.core import Board
from rsshogi.numpy import HuffmanCodedPosAndEval

board = Board()
data = np.zeros(1, dtype=HuffmanCodedPosAndEval)
board.to_hcpe(best_move="7g7f", score=120, game_result=1, out=data)

board.set_hcp(data[0]["hcp"])
```

## cshogi との使い分け

- `Move` / `Move32` は rsshogi 標準 API
- `AperyMove` / `AperyMove32` は `hcpe` や `cshogi` 互換データ連携用
- `PackedSfen` / `PackedSfenValue` は参照実装系
- `HuffmanCodedPos` / `HuffmanCodedPosAndEval` は Apery / cshogi 系

通常の盤面操作や合法手生成では `Move` / `Move32` を使い、外部データとの
入出力境界でのみ `AperyMove` 系を使うのが基本です。

---

## 関連項目

- [Board](board.md) - 盤面管理クラス（`to_hcp()` / `to_hcpe()` / `apery_move32_from_move()`）
- [Move](move.md) / [Move32](move32.md) - rsshogi 標準の指し手型
- [NumPy 連携](numpy.md) - `HuffmanCodedPos` / `HuffmanCodedPosAndEval` の dtype
- [cshogi との API 対応表](../reference/compat.md) - 互換性の考え方
