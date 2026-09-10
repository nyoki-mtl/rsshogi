# 学習バイナリ（HCP / HCPE / PackedSfen / pack）

rsshogi は棋譜フォーマットとは別に、将棋エンジンの学習でよく使われる固定長/可変長バイナリ形式も扱えます。

- PackedSfen 系: `PackedSfen`, `PackedSfenValue`
- Apery / cshogi 系: `HuffmanCodedPos` (HCP), `HuffmanCodedPosAndEval` (HCPE)
- ScriptCollection 系: `pack`

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
HCP と PackedSfen は、どちらも 32 バイトですが、ビット配置と駒の符号表が異なる別形式です。
一方のデータを他方の decoder で読み込むことはできません。
どちらも盤上に先後の玉が 1 枚ずつ存在する局面を対象にします。
片玉、玉なし、標準枚数を超える駒を含む局面は表現できません。

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

`pack` は開始局面、手ごとの `AperyMove` と `i16` 評価値、終局結果、終局理由を連結する可変長バイナリです。

```text
start_tag
  1: 平手開始局面
  0: HCP[32] + game_ply(u16 LE)
ply*
  apery_move(u16 LE) + eval(i16 LE)
terminal
  result_marker(u16 LE) + end_reason(u8)
```

`result_marker` は `0x0000`、`0x0081`、`0x0102` のいずれかで、順に引き分け、先手勝ち、後手勝ちを表します。
`end_reason` は投了、千日手、最大手数、中断、時間切れ、反則、入玉、トライなどの終了理由を表し、結果と矛盾する組合せは decode 時に拒否されます。

### PACK の `AperyMove` レイアウト

PACK の指し手は rsshogi `Move` ではなく `AperyMove` の raw `u16` です。

| ビット | 内容 |
|---|---|
| 0..6 | 着手先の盤上 index。 |
| 7..13 | 移動元の盤上 index、または駒打ちの source 値。 |
| 14 | 成りフラグ。 |
| 15 | 0。 |

盤上の移動元は `0..=80` です。
source 値 `81..=87` は歩、香、桂、銀、金、角、飛の駒打ちを表します。
駒打ちに成りフラグを立てる表現、範囲外の座標、同一升への移動は不正です。
PACK の reader は不正な `AperyMove` と局面上で非合法な手を拒否します。

### 評価値

PACK に保存する評価値は `i16` のセンチポーン値です。
`Record` からの書き出しでは各手の評価値が必須で、`-32000..=32000` の外側はその範囲へ clamp して保存します。
decode 時は保存済みの `i16` をそのまま `Record` の評価値として復元します。

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

## サイズの性質

`psv` と `hcpe` は局面ごとの固定長レコードです。
サイズはそれぞれ局面数 × 40 byte、局面数 × 38 byte で決まります。

`pack` と [sbinpack](../sbinpack.md) は開始局面と指し手列を保存する可変長形式です。
同じ対局でも、局面数、評価値差分、指し手インデックス、chunk の分け方、metadata の有無でサイズが変わります。
圧縮率を比較する場合は、同じデータと条件で生成したファイル全体のサイズを測定してください。

局面を独立してシャッフルまたはランダムアクセスする用途には固定長形式が向きます。
連続局面を順に再生できる用途には、手順連結型が向きます。

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
