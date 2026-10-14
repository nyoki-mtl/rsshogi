# 初期局面と手合割（`rsshogi.initial_positions`）

`rsshogi.initial_positions` モジュールは、平手、空盤面、および 13 種類の駒落ち初期局面の SFEN 定数と、日本語の手合割名と SFEN 文字列を相互変換する辞書や関数を提供する。

```python
from rsshogi.initial_positions import (
    HANDICAP_TO_SFEN,
    SFEN_TO_HANDICAP,
    InitialPosition,
    handicap_to_sfen,
    sfen_to_handicap,
)
```

## InitialPosition

```python
rsshogi.initial_positions.InitialPosition
```

標準の `enum.Enum` を継承した列挙型であり、各メンバの `.value` に初期局面の SFEN 文字列を保持する。
駒落ち局面（`HANDICAP_*`）はいずれも上手（後手、`w`）の手番から始まる。

### 定数一覧

| 列挙メンバ | 手合割名 | SFEN 文字列 (`member.value`) |
|---|---|---|
| `InitialPosition.STANDARD` | 平手 | `"lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"` |
| `InitialPosition.EMPTY` | （空盤面） | `"9/9/9/9/9/9/9/9/9 b - 1"` |
| `InitialPosition.HANDICAP_LANCE` | 香落ち | `"lnsgkgsn1/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"` |
| `InitialPosition.HANDICAP_RIGHT_LANCE` | 右香落ち | `"1nsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"` |
| `InitialPosition.HANDICAP_BISHOP` | 角落ち | `"lnsgkgsnl/1r7/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"` |
| `InitialPosition.HANDICAP_ROOK` | 飛車落ち | `"lnsgkgsnl/7b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"` |
| `InitialPosition.HANDICAP_ROOK_LANCE` | 飛香落ち | `"lnsgkgsn1/7b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"` |
| `InitialPosition.HANDICAP_2_PIECES` | 二枚落ち | `"lnsgkgsnl/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"` |
| `InitialPosition.HANDICAP_3_PIECES` | 三枚落ち | `"lnsgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"` |
| `InitialPosition.HANDICAP_4_PIECES` | 四枚落ち | `"1nsgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"` |
| `InitialPosition.HANDICAP_5_PIECES` | 五枚落ち | `"2sgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"` |
| `InitialPosition.HANDICAP_LEFT_5_PIECES` | 左五枚落ち | `"1nsgkgs2/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"` |
| `InitialPosition.HANDICAP_6_PIECES` | 六枚落ち | `"2sgkgs2/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"` |
| `InitialPosition.HANDICAP_8_PIECES` | 八枚落ち | `"3gkg3/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"` |
| `InitialPosition.HANDICAP_10_PIECES` | 十枚落ち | `"4k4/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"` |

```python
from rsshogi.core import Board
from rsshogi.initial_positions import InitialPosition
from rsshogi.types import Color

# 二枚落ちの初期局面で Board を初期化
board = Board(sfen=InitialPosition.HANDICAP_2_PIECES.value)
assert board.turn == Color.WHITE
```

## 手合割名と SFEN の相互変換

`InitialPosition.EMPTY` を除く 14 種類の標準手合割（`"平手"` から `"十枚落ち"` まで）について、読み取り専用マッピング（`MappingProxyType`）と変換関数が用意されている。

| 定数 / 関数 | 型 / シグネチャ | 説明 |
|---|---|---|
| `HANDICAP_TO_SFEN` | `Mapping[str, str]` | 日本語の手合割名（全 14 項目）をキー、SFEN 文字列を値とする読み取り専用辞書である。 |
| `SFEN_TO_HANDICAP` | `Mapping[str, str]` | SFEN 文字列をキー、日本語の手合割名を値とする読み取り専用辞書である。 |
| `handicap_to_sfen` | `handicap_to_sfen(handicap: str) -> str` | 手合割名に対応する SFEN 文字列を返す。未登録の名前を渡すと `KeyError` を送出する。 |
| `sfen_to_handicap` | `sfen_to_handicap(sfen: str) -> str \| None` | SFEN 文字列に対応する手合割名を返す。該当しない場合は `None` を返す。 |

```python
from rsshogi.initial_positions import (
    HANDICAP_TO_SFEN,
    SFEN_TO_HANDICAP,
    handicap_to_sfen,
    sfen_to_handicap,
)

sfen = handicap_to_sfen("飛香落ち")
assert sfen == HANDICAP_TO_SFEN["飛香落ち"]
assert sfen_to_handicap(sfen) == "飛香落ち"
assert SFEN_TO_HANDICAP[sfen] == "飛香落ち"
assert sfen_to_handicap("9/9/9/9/9/9/9/9/9 b - 1") is None
```

## 関連項目

- [`Board`](board.md)
- [`Record`](record.md)
