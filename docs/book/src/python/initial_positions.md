# InitialPosition クラス

```python
rsshogi.initial_positions.InitialPosition
```

将棋の初期局面（平手・駒落ち）の SFEN 文字列を定数として提供する列挙型です。

```python
from rsshogi.initial_positions import InitialPosition
```

## 概要

`InitialPosition` は、よく使用される初期局面の SFEN 文字列を定数として提供します。
`Board` のコンストラクタに渡すことで、各種初期局面を簡単に設定できます。

## 定数一覧

### 平手

| 定数 | 説明 |
|------|------|
| `STANDARD` | 平手（標準の初期局面） |
| `EMPTY` | 空の盤面 |

### 駒落ち

| 定数 | 説明 |
|------|------|
| `HANDICAP_LANCE` | 香落ち |
| `HANDICAP_RIGHT_LANCE` | 右香落ち |
| `HANDICAP_BISHOP` | 角落ち |
| `HANDICAP_ROOK` | 飛車落ち |
| `HANDICAP_ROOK_LANCE` | 飛香落ち |
| `HANDICAP_2_PIECES` | 二枚落ち（飛車・角） |
| `HANDICAP_3_PIECES` | 三枚落ち（飛角香） |
| `HANDICAP_4_PIECES` | 四枚落ち（飛角香香） |
| `HANDICAP_5_PIECES` | 五枚落ち（飛角香香桂） |
| `HANDICAP_LEFT_5_PIECES` | 左五枚落ち（飛角香香右桂） |
| `HANDICAP_6_PIECES` | 六枚落ち（飛角香香桂桂） |
| `HANDICAP_8_PIECES` | 八枚落ち（飛角香香桂桂銀銀） |
| `HANDICAP_10_PIECES` | 十枚落ち（飛角香香桂桂銀銀金金） |

## 基本的な使い方

```python
from rsshogi.core import Board
from rsshogi.initial_positions import InitialPosition

# 平手
board = Board()
# または
board = Board(sfen=InitialPosition.STANDARD.value)

# 二枚落ち
board = Board(sfen=InitialPosition.HANDICAP_2_PIECES.value)
print(board.to_sfen())
# => "lnsgkgsnl/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"

# 六枚落ち
board = Board(sfen=InitialPosition.HANDICAP_6_PIECES.value)
```

## 使用例

### 駒落ち対局のシミュレーション

```python
from rsshogi.core import Board
from rsshogi.initial_positions import InitialPosition

# 二枚落ちの局面を作成
board = Board(sfen=InitialPosition.HANDICAP_2_PIECES.value)

# 上手（後手）の手番から始まる
print(board.turn)  # => "white"

# 合法手を確認
for move in board.legal_moves()[:5]:
    print(move.to_usi())
```

### 各駒落ちの局面を表示

```python
from rsshogi.core import Board
from rsshogi.initial_positions import InitialPosition

handicaps = [
    ("香落ち", InitialPosition.HANDICAP_LANCE),
    ("角落ち", InitialPosition.HANDICAP_BISHOP),
    ("飛車落ち", InitialPosition.HANDICAP_ROOK),
    ("二枚落ち", InitialPosition.HANDICAP_2_PIECES),
    ("三枚落ち", InitialPosition.HANDICAP_3_PIECES),
    ("四枚落ち", InitialPosition.HANDICAP_4_PIECES),
    ("五枚落ち", InitialPosition.HANDICAP_5_PIECES),
    ("左五枚落ち", InitialPosition.HANDICAP_LEFT_5_PIECES),
    ("六枚落ち", InitialPosition.HANDICAP_6_PIECES),
]

for name, pos in handicaps:
    board = Board(sfen=pos.value)
    print(f"{name}: {board.to_sfen()}")
```

## SFEN 文字列の取得

```python
from rsshogi.initial_positions import InitialPosition

# .value で SFEN 文字列を取得
sfen = InitialPosition.HANDICAP_2_PIECES.value
print(sfen)
# => "lnsgkgsnl/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
```

## 手合割名と SFEN の相互変換

`rsshogi.initial_positions` には、手合割名と SFEN を相互変換する
ヘルパーも用意されています。

```python
from rsshogi.initial_positions import (
    HANDICAP_TO_SFEN,
    SFEN_TO_HANDICAP,
    handicap_to_sfen,
    sfen_to_handicap,
)

sfen = handicap_to_sfen("三枚落ち")
name = sfen_to_handicap(sfen)

print(sfen == HANDICAP_TO_SFEN["三枚落ち"])  # => True
print(name == SFEN_TO_HANDICAP[sfen])         # => True
```

## 関連項目

- [`Board`](board.md) - 盤面管理クラス
- [クイックスタート](../getting-started/quickstart.md)
