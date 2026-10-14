# SAZ2 自己対局教師データ（`rsshogi.sazpack`）

`rsshogi.sazpack` は SAZ2 形式の自己対局教師データを読み書きするモジュールである。
SAZ2 は各対局の初期局面（32 バイトの `PackedSfen`）、終局結果、終局理由、入玉ルールに加えて、各手番における探索ルート、対局結果、ネットワーク生出力の WDL（勝、分、敗）分布、候補手ごとの Policy 重みと訪問数、詰み手数や残り手数などの探索テレメトリをコンパクトなバイナリとして保持する。

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

---

## データクラス

### `SazWdl`

```python
SazWdl(win: int, draw: int, loss: int) -> SazWdl
```

手番側視点での勝ち（`win`）、引き分け（`draw`）、負け（`loss`）の重みを `uint16`（`0..=65535`）で保持するクラスである。
SAZ2 バイナリへ書き出す際、`win + draw + loss == 65535`（量子化スケール合計）を満たす必要がある。

| プロパティ | 型 | 範囲 | 説明 |
|---|---|---|---|
| `win` | `int` | `0..=65535` | 勝利確率の量子化重み。 |
| `draw` | `int` | `0..=65535` | 引き分け確率の量子化重み。 |
| `loss` | `int` | `0..=65535` | 敗北確率の量子化重み。 |

### `SazPolicyEntry`

```python
SazPolicyEntry(
    mv: Move | Move32 | int | str,
    prior: int,
    raw_prior: int,
    visits: int,
    lower: int,
    upper: int,
) -> SazPolicyEntry
```

探索における 1 つの候補手の Policy 重み、訪問数、および結果区間（Outcome Bound）を保持するクラスである。
コンストラクタの `mv` には `Move`、`Move32`、`int`（`0..=65535`）、または USI 文字列を渡すことができ、`mv` プロパティからは正規化された USI 文字列として取得できる。

| プロパティ | 型 | 範囲 / コード | 説明 |
|---|---|---|---|
| `mv` | `str` | USI 文字列 | 候補手の USI 表記（例：`"7g7f"`）。 |
| `prior` | `int` | `0..=65535` | 探索で使用した Policy 重み（同一局面の全候補手の合計が `65535`）。 |
| `raw_prior` | `int` | `0..=65535` | ニューラルネットワークが出力した生の Policy 重み（同一局面の全候補手の合計が `65535`）。 |
| `visits` | `int` | `0..=4294967295` | 探索終了時点の累積訪問数（`uint32`）。 |
| `lower` | `int` | `0`, `1`, `2` | 結果区間の下限コード（`0`：負け、`1`：引き分け、`2`：勝ち）。 |
| `upper` | `int` | `0`, `1`, `2` | 結果区間の上限コード（`0`：負け、`1`：引き分け、`2`：勝ち）。 |

`lower` と `upper` は `lower <= upper` を満たす必要がある。

### `SazPosition`

```python
SazPosition(
    played: Move | Move32 | int | str,
    root_wdl: SazWdl,
    outcome_wdl: SazWdl,
    raw_wdl: SazWdl,
    raw_mate: int,
    raw_moves_left: int,
    plies_left: int,
    requested_visits: int,
    target_weight_milli: int,
    exploration_flags: int,
    policy: Sequence[SazPolicyEntry],
    mate: int | None = None,
) -> SazPosition
```

対局内の 1 局面分の着手と自己対局教師データを保持するクラスである。

| プロパティ | 型 | 範囲 | 説明 |
|---|---|---|---|
| `played` | `str` | USI 文字列 | 実際に選択された着手の USI 文字列（現在局面で合法手である必要がある）。 |
| `root_wdl` | `SazWdl` | 合計 `65535` | 探索完了時のルート WDL 分布。 |
| `outcome_wdl` | `SazWdl` | 合計 `65535` | 対局結果に基づく WDL 分布。 |
| `raw_wdl` | `SazWdl` | 合計 `65535` | ネットワーク出力の生の WDL 分布。 |
| `raw_mate` | `int` | `0..=65535` | ネットワーク出力の詰み指標（`uint16`）。 |
| `raw_moves_left` | `int` | `0..=65535` | ネットワーク出力の予想残り手数（`uint16`）。 |
| `plies_left` | `int` | `0..=65535` | 実際の終局までの残り手数（`uint16`）。 |
| `requested_visits` | `int` | `0..=4294967295` | 探索時に要求された目標訪問数（`uint32`）。 |
| `target_weight_milli` | `int` | `0..=65535` | 学習ターゲット重みのミリ単位表現（`1000` が標準重み `1.0`）。 |
| `exploration_flags` | `int` | `0..=255` | 探索モードやノイズ付与状態を表すビットフラグ（`uint8`）。 |
| `mate` | `int \| None` | `-32768..=32767` | 探索で判明した詰み手数（`int16`、未判明時は `None`）。 |
| `policy` | `list[SazPolicyEntry]` | 1 件以上 | 合法な候補手ごとの Policy エントリのリスト（同一手の重複は不可）。 |

### `SazGame`

```python
SazGame(
    stem: str | bytes | bytearray,
    game_result: GameResult | int | str,
    termination_reason: int,
    entering_king_rule: int,
    positions: Sequence[SazPosition],
) -> SazGame
```

1 対局分の SAZ2 データを保持するクラスである。
コンストラクタの `stem` には初期局面の SFEN 文字列、または 32 バイトの `PackedSfen`（`bytes` / `bytearray`）を指定できる。

| プロパティ | 型 | 説明 |
|---|---|---|
| `stem_packed_sfen` | `bytes` | 初期局面の 32 バイト `PackedSfen`。 |
| `game_result` | `GameResult` | 対局結果（[棋譜管理の `GameResult`](record.md#gameresult-と-gameresultinfo) を参照）。 |
| `termination_reason` | `int` | SAZ2 の終局理由コード（下表参照）。 |
| `entering_king_rule` | `int` | SAZ2 の入玉ルールコード（下表参照）。 |
| `positions` | `list[SazPosition]` | 初期局面から着手順に並んだ `SazPosition` のリスト。 |

#### `termination_reason` と `entering_king_rule` のコード一覧

| `termination_reason` | 意味 | `entering_king_rule` | 意味 |
|---:|---|---:|---|
| `0` | 合法手なし（詰みなど） | `0` | 入玉ルールなし |
| `1` | 千日手引き分け | `1` | 24 点法 |
| `2` | 連続王手の千日手による勝ち | `2` | 24 点法（駒落ち対応） |
| `3` | 連続王手の千日手による負け | `3` | 27 点法 |
| `4` | 優等局面による終了 | `4` | 27 点法（駒落ち対応） |
| `5` | 劣等局面による終了 | `5` | トライルール |
| `6` | 入玉宣言勝ち | `6` | 未設定 |
| `7` | 最大手数到達 | - | - |

---

## シリアライズとデシリアライズ関数

`write_sazpack` および `write_sazpack_file` は、書き出し時に初期局面のデコード、各手の合法性、候補手の重複有無、WDL 分布および Policy 重みの合計値（`65535`）をすべて検証し、違反がある場合は `ValueError` を送出する。

| 関数 | シグネチャ | 説明 |
|---|---|---|
| `write_sazpack(games)` | `(games: Sequence[SazGame]) -> bytes` | `SazGame` のシーケンスを SAZ2 バイナリへ変換して `bytes` を返す。 |
| `write_sazpack_file(path, games)` | `(path: str \| PathLike, games: Sequence[SazGame]) -> None` | `SazGame` のシーケンスを SAZ2 バイナリファイルへ書き出す。 |
| `decode_sazpack(data)` | `(data: bytes \| bytearray) -> list[SazGame]` | SAZ2 バイナリから `list[SazGame]` を復元する。 |
| `decode_sazpack_file(path)` | `(path: str \| PathLike) -> list[SazGame]` | SAZ2 バイナリファイルから `list[SazGame]` を復元する。 |

```python
from rsshogi.record import GameResult
from rsshogi.sazpack import (
    SazGame,
    SazPolicyEntry,
    SazPosition,
    SazWdl,
    decode_sazpack,
    write_sazpack,
)

wdl = SazWdl(win=40000, draw=5535, loss=20000)
policy = [
    SazPolicyEntry("7g7f", prior=45000, raw_prior=42000, visits=120, lower=0, upper=2),
    SazPolicyEntry("2g2f", prior=20535, raw_prior=23535, visits=80, lower=0, upper=2),
]
position = SazPosition(
    played="7g7f",
    root_wdl=wdl,
    outcome_wdl=SazWdl(65535, 0, 0),
    raw_wdl=wdl,
    raw_mate=0,
    raw_moves_left=80,
    plies_left=75,
    requested_visits=200,
    target_weight_milli=1000,
    exploration_flags=1,
    policy=policy,
    mate=None,
)
game = SazGame(
    stem="lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
    game_result=GameResult.BLACK_WIN,
    termination_reason=0,
    entering_king_rule=3,
    positions=[position],
)

blob = write_sazpack([game])
restored = decode_sazpack(blob)
assert restored[0].positions[0].played == "7g7f"
assert len(restored[0].positions[0].policy) == 2
```

## 関連項目

- [Policy ラベル変換（`rsshogi.policy`）](policy.md)
- [盤面管理（`Board`）](board.md)
- [棋譜管理（`Record` / `GameResult`）](record.md)
- [棋譜フォーマット一覧](../reference/formats/index.md)
