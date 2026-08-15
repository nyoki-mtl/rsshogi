# SAZ2 自己対局教師データ

`rsshogi.sazpack` は SAZ2 形式の自己対局教師データを読み書きするモジュールです。

```python
from rsshogi.sazpack import (
    SazGame,
    SazPolicyEntry,
    SazPosition,
    SazWdl,
    decode_sazpack,
    decode_sazpack_file,
    write_sazpack,
    write_sazpack_file,
)
```

SAZ2 は初期局面、対局結果、各手の WDL 分布、policy、探索メタデータをまとめて保持します。

## `SazWdl`

```python
SazWdl(win: int, draw: int, loss: int)
```

`win`、`draw`、`loss` はそれぞれ非負の `int` です。

| プロパティ | 型 | 説明 |
|------|------|------|
| `win` | `int` | 勝利の重み |
| `draw` | `int` | 引き分けの重み |
| `loss` | `int` | 敗北の重み |

`write_sazpack()` は WDL 分布の合計が形式が要求する値と一致することを検証します。

## `SazPolicyEntry`

```python
SazPolicyEntry(mv, prior, raw_prior, visits_before, visits_after, lower, upper)
```

`mv` は `Move`、`Move32`、`int`、または USI 文字列を受け付けます。

| プロパティ | 型 | 説明 |
|------|------|------|
| `mv` | `str` | USI 形式の候補手 |
| `prior` | `int` | 掃索後の policy 重み |
| `raw_prior` | `int` | ネットワーク出力の policy 重み |
| `visits_before` / `visits_after` | `int` | 対応するスナップショットの訪問数 |
| `lower` / `upper` | `int` | 結果区間の下限と上限 |

`write_sazpack()` は各局面の `prior` と `raw_prior` を別々に検証し、`visits_after >= visits_before` を要求します。

## `SazPosition`

```python
SazPosition(
    played,
    root_wdl,
    outcome_wdl,
    raw_wdl,
    raw_mate,
    raw_moves_left,
    plies_left,
    requested_visits,
    target_weight_milli,
    exploration_flags,
    policy,
    mate=None,
)
```

1 局面分の着手と自己対局データを保持します。

| プロパティ | 型 | 説明 |
|------|------|------|
| `played` | `str` | USI 形式の実際の着手 |
| `root_wdl` | `SazWdl` | 根の WDL 分布 |
| `outcome_wdl` | `SazWdl` | 対局結果の WDL 分布 |
| `raw_wdl` | `SazWdl` | ネットワーク出力の WDL 分布 |
| `raw_mate` / `raw_moves_left` | `int` | ネットワーク出力の詰みと残り手数 |
| `plies_left` | `int` | 対局結果に基づく残り手数 |
| `requested_visits` | `int` | 要求した訪問数 |
| `target_weight_milli` | `int` | 対象重みの 1/1000 単位表現 |
| `exploration_flags` | `int` | 探索フラグ |
| `mate` | `int | None` | 詰み情報 |
| `policy` | `list[SazPolicyEntry]` | policy 候補手 |

## `SazGame`

```python
SazGame(stem, game_result, termination_reason, entering_king_rule, positions)
```

`stem` は SFEN 文字列または 32 バイトの Packed SFEN を受け付けます。

| プロパティ | 型 | 説明 |
|------|------|------|
| `stem_packed_sfen` | `bytes` | 32 バイトの初期局面 |
| `game_result` | `GameResult` | 対局結果 |
| `termination_reason` | `int` | SAZ2 の終局理由コード |
| `entering_king_rule` | `int` | SAZ2 の入玉ルールコード |
| `positions` | `list[SazPosition]` | 着手順の局面データ |

## 読み書き

| 関数 | 説明 |
|------|------|
| `write_sazpack(games)` | `Sequence[SazGame]` を SAZ2 バイナリにシリアライズして `bytes` を返す |
| `decode_sazpack(data)` | `bytes | bytearray` から `list[SazGame]` を復元する |
| `write_sazpack_file(path, games)` | SAZ2 バイナリをファイルへ書き込む |
| `decode_sazpack_file(path)` | SAZ2 バイナリファイルから復元する |

```python
from rsshogi.record import GameResult
from rsshogi.sazpack import SazGame, SazPolicyEntry, SazPosition, SazWdl, decode_sazpack, write_sazpack

wdl = SazWdl(65535, 0, 0)
policy = [SazPolicyEntry("7g7f", 65535, 65535, 0, 1, 0, 2)]
position = SazPosition("7g7f", wdl, wdl, wdl, 0, 0, 1, 1, 1000, 0, policy)
game = SazGame("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1", GameResult.BLACK_WIN, 7, 0, [position])

data = write_sazpack([game])
decoded = decode_sazpack(data)
assert decoded[0].positions[0].played == "7g7f"
```

## 関連項目

- [Policy ラベル](policy.md) - policy 候補手のラベル変換。
- [Board](board.md) - SFEN と Packed SFEN の相互変換。
- [GameResult](game_result.md) - 対局結果。
