# 学習バイナリ（HCP / HCPE / PackedSfen / pack）

rsshogi は棋譜フォーマットとは別に、将棋エンジンの学習でよく使われる固定長/可変長バイナリ形式も扱えます。

- PackedSfen 系: `PackedSfen`, `PackedSfenValue`
- Apery / cshogi 系: `HuffmanCodedPos` (HCP), `HuffmanCodedPosAndEval` (HCPE)
- ScriptCollection 系: `pack`

`hcpe3` は現在の対象外です。
sbinpack v2 と sazpack は独立した仕様ページとして扱います。

## 一覧

| 形式 | サイズ | 系統 | 主な API |
|------|--------|------|----------|
| `PackedSfen` | 32 bytes | 参照実装 | `Board.to_packed_sfen()`, `Board.set_packed_sfen()` |
| `PackedSfenValue` | 40 bytes | 参照実装 | `Board.to_psv()`, `Record.to_psv()` |
| `HuffmanCodedPos` | 32 bytes | Apery / cshogi | `Board.to_hcp()`, `Board.set_hcp()` |
| `HuffmanCodedPosAndEval` | 38 bytes | Apery / cshogi | `Board.to_hcpe()` |
| `pack` | 可変長 | ScriptCollection | `Record.to_pack()`, `Record.from_pack()` |

## HCP

`HuffmanCodedPos` は Apery / cshogi 互換の 32 バイト局面形式です。

```python
from rsshogi.core import Board

board = Board()
hcp = board.to_hcp()
board.set_hcp(hcp)
```

## HCPE

`HuffmanCodedPosAndEval` は HCP に評価値と 16bit 指し手を加えた 38 バイトの固定長エントリです。

```text
hcp[32]
eval        : int16
bestMove16  : uint16
gameResult  : int8
dummy       : uint8
```

`bestMove16` は Apery 16bit 指し手（`AperyMove`）です。rsshogi `Move` とはビットレイアウトが異なります。

```python
hcpe = board.to_hcpe(
    best_move="7g7f",
    score=120,
    game_result="BLACK_WIN",
)
```

## `bestMove16` の扱い

`PackedSfenValue` と `sbinpack Stem` の `bestMove16` は **rsshogi `Move` 形式**で格納されます。
`Board.to_psv()` / `Record.to_psv()` は内部的に `mv.raw()` をそのまま書き出します。
これは cshogi (Apery) の `Move16` とはビットレイアウトが異なります。

HCPE の `bestMove16` は Apery 形式（`AperyMove`）で格納されます。
`to_hcpe()` は `best_move` 引数として `AperyMove` / `AperyMove32` を受け取ります。

必要に応じて次を使い分けます。

- `Move.to_apery() -> AperyMove`
- `AperyMove.to_move() -> Move`
- `Move32.to_apery(board) -> AperyMove32`
- `AperyMove32.to_move32(board) -> Move32`

`AperyMove32 -> Move32` は局面依存です。`AperyMove32` は `PieceType` しか持たず、
`Move32` は色付きの `Piece` を持つため、復元には `Board` が必要です。

## pack

`pack` は ScriptCollection の `pack2hcpe.py` ワークフローで使われる
可変長バイナリです。開始局面ごとに、`AperyMove` と `i16` 評価値の列、終局結果、
終局理由を連結して表現します。

```python
import rsshogi as rs

record = rs.record.Record.from_main_line(
    rs.core.Board().to_sfen(),
    [
        rs.record.MoveEntry("7g7f", engine_info=rs.record.EngineInfo(eval=120)),
        rs.record.MoveEntry("3c3d", engine_info=rs.record.EngineInfo(eval=-80)),
    ],
    rs.record.SpecialMoveEntry("RESIGN", rs.record.GameResult.WHITE_WIN),
)

payload = record.to_pack()
restored = rs.record.Record.from_pack(payload)
```

複数局をまとめる場合は `rsshogi.record.write_pack()` / `decode_pack()` を使います。

## NumPy での読み書き

```python
import numpy as np
from rsshogi.core import Board
from rsshogi.numpy import HuffmanCodedPos, HuffmanCodedPosAndEval

board = Board()

hcp = np.zeros(1, dtype=HuffmanCodedPos)
board.to_hcp(hcp)

hcpe = np.zeros(1, dtype=HuffmanCodedPosAndEval)
board.to_hcpe(best_move="7g7f", score=32, game_result=1, out=hcpe)
```

## 実データでのサイズ比較

実戦譜から生成した `pack` データを各フォーマットへ変換し、データサイズを実測した結果です。
固定長フォーマット（psv / hcpe）は 1 局面あたりのバイト数が決まっているため、
局面数だけからサイズを算出できます（psv は局面数 × 40 byte、hcpe は局面数 × 38 byte）。
一方、手順連結型（pack / [sbinpack](../sbinpack.md)）は実際に変換したバイト数を計測しています。

**計測データセット**: 125,082 局 / 13,344,569 局面（平均 106.7 手/局、本手順のみ）

| フォーマット | サイズ | 1 局面あたり | pack 比 | 表現方式 |
|------|--------|------------|--------|----------|
| `sbinpack` | 46.30 MB | 3.47 byte | 0.80x | 手順連結（インデックス + 差分、可変長） |
| `pack` | 58.13 MB | 4.36 byte | 1.00x | 手順連結（16bit 指し手 + i16 評価値、固定 4 byte/手） |
| `hcpe` | 507.09 MB | 38.00 byte | 8.72x | 局面ごとに HCP(32B) + 評価/指し手/結果 |
| `psv` | 533.78 MB | 40.00 byte | 9.18x | 局面ごとに PackedSfen(32B) + 評価/指し手/手数/結果 |

### 考察

- 手順連結型（pack / sbinpack）は局面ごとの盤面を持たず、初期局面 + 指し手列で表現するため、
  局面独立型（psv / hcpe）の **1/9〜1/10** のサイズに収まります。
- sbinpack が最もコンパクトです（pack 比 0.80x、約 20% 削減）。
  指し手を「合法手リスト中のインデックス」の ULEB128、評価値を前局面からの ZigZag 差分で持つため、
  pack の「16bit 指し手 + i16 評価値 = 4 byte/手 固定」よりさらに小さくなります。
- psv / hcpe は 1 局面ごとに完全な圧縮盤面（32 byte）を持つ固定長のため、
  サイズは大きい一方で、シャッフルやランダムアクセスを伴う学習データとして扱いやすい利点があります。

### sbinpack の圧縮率の内訳

「sbinpack が pack の 0.80x 止まり」の理由を、1 手あたりのバイト内訳で分解すると以下になります。

| 要素 | pack | sbinpack | 備考 |
|------|------|----------|------|
| 指し手 | 2.00 byte（生 16bit） | **1.04 byte**（合法手インデックス ULEB128） | 約 96% の手がインデックス < 128 で 1 byte に収まる |
| 評価値 | 2.00 byte（i16 固定） | **2.05 byte**（前局面からの差分 ZigZag + ULEB128） | 差分化が効かず、むしろ微増 |
| 合計 | 4.00 byte/手 | 3.09 byte/手 | |

- 削減はほぼ「指し手のインデックス化」だけで稼いでいます（2.00 → 1.04 byte/手）。
  pack 自体が「盤面を持たず指し手列で表現する」既に密な形式のため、絞れる余地が元々小さいことが、
  圧縮率が思ったほど伸びない主因です。
- 評価値の差分化はこのデータでは効いていません（2.05 byte/手 ≧ 生 i16 の 2.00 byte/手）。
  評価値差分のバイト長分布は 1 byte: 7.8% / 2 byte: 79.4% / 3 byte: 12.8% で、
  3 byte に膨らむケースが固定長 i16 より不利に働いています。
- 原因は、**評価値が手番側視点（side-to-move）の cp で格納され、1 手ごとに符号が反転する**ためです。
  連続局面の評価値が `+200 → -180 → +210 ...` のように交互に符号反転すると、差分は元の評価値の
  約 2 倍の大きさになり、可変長符号化で 2〜3 byte に膨らみます。差分コーディングは「隣接値が近い」
  ことを前提とするため、符号が飛ぶと逆効果になります。
- sbinpack v2 では、評価値を stem 側視点に正規化してから差分を取ります。これにより、
  このデータでは評価値差分が **2.05 → 1.45 byte/手**（-29%）まで下がり、sbinpack 全体は
  46.30 MB → **38.38 MB（pack 比 約 0.66x、実測）** へ縮みました。
  （movetext だけなら ~0.62x 相当ですが、局ごとの stem 固定費 40 byte を含めた実ファイル比は 0.66x です。）

> 計測は本手順のみ（分岐を含めず）を対象とし、sbinpack は単一チャンク構成で算出しています。
> psv / hcpe のサイズは固定長エントリ数からの算出値です。

## 用途の違い

- `PackedSfen` / `PackedSfenValue`: 既存の学習データとの互換
- `HuffmanCodedPos` / `HuffmanCodedPosAndEval`: Apery / cshogi 系学習データとの互換
- `pack`: ScriptCollection / `pack2hcpe.py` 系ワークフローとの互換

通常の棋譜処理には `Record` を使い、固定長の学習バイナリを扱うときだけこれらの形式を使うのが基本です。

## 関連項目

- [NumPy 連携](../../python/numpy.md)
- [Apery / hcpe 互換](../../python/apery.md)
- [sbinpack 仕様](../sbinpack.md)
- [sazpack 仕様](../sazpack.md)
