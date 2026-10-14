# USI プロトコル（`rsshogi.usi` / USI 局面解析）

`rsshogi.usi` は USI（Universal Shogi Interface）プロトコルの文字列と構造化オブジェクトを相互変換するステートレスなモジュールである。
また、`rsshogi.core` には USI の `position` コマンド文字列を検証、分解、正規化する関数群が用意されている。

```python
from rsshogi.core import (
    UsiPositionParts,
    normalize_usi_position,
    parse_usi_position,
    parse_usi_position_parts,
)
from rsshogi.usi import (
    MAX_PLY,
    VALUE_MATE,
    VALUE_MATE_IN_MAX_PLY,
    UsiBestMove,
    UsiBound,
    UsiGoCommand,
    UsiInfo,
    UsiScore,
    move_from_usi,
    parse_bestmove,
    parse_info,
    parse_score,
)
```

---

## 定数と指し手変換（`rsshogi.usi`）

| 定数 / 関数 | 型 / 値 | 説明 |
|---|---|---|
| `MAX_PLY` | `int`（`246`） | 詰みスコア計算の基準となる最大探索手数。 |
| `VALUE_MATE` | `int`（`32000`） | 0 手詰みに対する内部スコア基準値。 |
| `VALUE_MATE_IN_MAX_PLY` | `int`（`31754`） | `VALUE_MATE - MAX_PLY`（詰みスコアと判定する閾値）。 |
| `move_from_usi(usi)` | `(usi: str) -> Move` | USI 指し手文字列を `Move` へ変換する。通常手と `"0000"` に加えて、`"resign"`（`Move.WIN` ではなく `Move.RESIGN`）、`"win"`（`Move.WIN`）、`"none"`（`Move.NONE`）も受理する。 |
| `parse_score(text)` | `(text: str) -> UsiScore` | `"cp 120"` や `"mate +5"` などの USI スコア文字列を解析する。 |
| `parse_info(line)` | `(line: str) -> UsiInfo` | USI エンジンの `info ...` 出力行を解析する。 |
| `parse_bestmove(line)` | `(line: str) -> UsiBestMove` | USI エンジンの `bestmove ...` 出力行を解析する。 |

> [!NOTE]
> `rsshogi.core.Move.from_usi(usi)` は通常の指し手と `"0000"`（パス手）のみを受理し、`"resign"` や `"win"` には `ValueError` を送出する。エンジンの `bestmove` トークンを扱う場合は `rsshogi.usi.move_from_usi(usi)` または `parse_bestmove(line)` を使用する。

---

## スコアと探索情報の解析（`UsiScore` / `UsiBound` / `UsiInfo`）

### `UsiBound`

探索評価値が真の値のどの境界であるかを表す列挙型（`IntEnum`）である。

| 定数 | 値 (`value`) | `to_usi()` の戻り値 | 説明 |
|---|---:|---|---|
| `UsiBound.NONE` | `0` | `""` | 境界指定なし。 |
| `UsiBound.UPPER` | `1` | `"upperbound"` | 上界（実際の評価値はこの値以下）。 |
| `UsiBound.LOWER` | `2` | `"lowerbound"` | 下界（実際の評価値はこの値以上）。 |
| `UsiBound.EXACT` | `3` | `""` | 正確な評価値。 |

### `UsiScore`

USI の `score cp <x>` および `score mate <y>` を表すイミュータブルなデータクラスである。

| メソッド / プロパティ | シグネチャ / 型 | 説明 |
|---|---|---|
| `UsiScore.cp(value)` | `(value: int) -> UsiScore` | センチポーン評価値（`kind="cp"`）を生成する（クラスメソッド）。 |
| `UsiScore.mate(plies)` | `(plies: int) -> UsiScore` | 手数付きの詰みスコア（`kind="mate"`）を生成する（クラスメソッド）。 |
| `UsiScore.mate_plus()` | `() -> UsiScore` | 手数不明の詰み（`score mate +`、`kind="mate_plus"`）を生成する（クラスメソッド）。 |
| `UsiScore.mate_minus()` | `() -> UsiScore` | 手数不明の被詰（`score mate -`、`kind="mate_minus"`）を生成する（クラスメソッド）。 |
| `UsiScore.parse(text)` | `(text: str) -> UsiScore` | `"cp 85"`、`"score mate -3"`、`"mate +"` などの文字列から `UsiScore` を生成する（クラスメソッド）。 |
| `UsiScore.from_raw(raw_score)` | `(raw_score: int) -> UsiScore` | 内部整数スコアから `UsiScore` へ変換する。絶対値が `VALUE_MATE_IN_MAX_PLY`（`31754`）以上の場合は詰みスコア（`VALUE_MATE - abs(raw_score)` 手詰み）に変換される（クラスメソッド）。 |
| `kind` | `str` | `"cp"`、`"mate"`、`"mate_plus"`、`"mate_minus"` のいずれか。 |
| `value` | `int \| None` | `"cp"` または `"mate"` の場合の整数値（`"mate_plus"` / `"mate_minus"` では `None`）。 |
| `to_usi_tokens()` | `tuple[str, str]` | `("cp", "85")` や `("mate", "+")` の 2 要素タプルを返す。 |
| `to_usi()` / `str(score)` | `str` | `"cp 85"` や `"mate -3"` 形式の文字列を返す。 |
| `to_raw()` | `int` | `VALUE_MATE`（`32000`）基準の内部整数スコアへ変換する。 |

### `UsiInfo`

```python
UsiInfo.parse(text: str) -> UsiInfo
parse_info(line: str) -> UsiInfo
```

USI エンジンが出力する `info` 行の各項目を保持するイミュータブルなデータクラスである。
`score` トークンが含まれる場合、`score` プロパティには `to_raw()` で変換された内部整数スコアが入り、`raw_score` プロパティには `UsiScore` オブジェクトが入る。

| プロパティ | 型 | 説明 |
|---|---|---|
| `depth` | `int \| None` | 基本探索深さ（`depth`）。 |
| `seldepth` | `int \| None` | 選択探索深さ（`seldepth`）。 |
| `time` | `int \| None` | 探索経過時間（ミリ秒、`time`）。 |
| `nodes` | `int \| None` | 探索ノード数（`nodes`）。 |
| `nps` | `int \| None` | 1 秒あたりの探索ノード数（`nps`）。 |
| `hashfull` | `int \| None` | 置換表の使用率（千分率、`hashfull`）。 |
| `multipv` | `int \| None` | MultiPV の順位番号（`multipv`）。 |
| `score` | `int \| None` | 内部整数表現の評価値（`raw_score.to_raw()`）。 |
| `raw_score` | `UsiScore \| None` | 構造化された評価値オブジェクト。 |
| `bound` | `UsiBound` | `lowerbound` / `upperbound` の区分（指定がない場合は `UsiBound.NONE`）。 |
| `pv` | `tuple[Move, ...]` | 読み筋（Principal Variation）の指し手タプル。 |
| `text` | `str \| None` | `info string ...` の文字列メッセージ。 |

```python
from rsshogi.usi import UsiBound, parse_info

info = parse_info(
    "info depth 18 seldepth 26 time 450 nodes 890000 nps 1977777 hashfull 310 "
    "multipv 1 score cp 145 lowerbound pv 7g7f 3c3d 2g2f"
)
assert info.depth == 18
assert info.score == 145
assert info.raw_score is not None and info.raw_score.to_usi() == "cp 145"
assert info.bound is UsiBound.LOWER
assert [m.to_usi() for m in info.pv] == ["7g7f", "3c3d", "2g2f"]
```

---

## `UsiBestMove` と `UsiGoCommand`

### `UsiBestMove`

```python
UsiBestMove(bestmove: Move, ponder: Move | None = None) -> UsiBestMove
UsiBestMove.parse(text: str) -> UsiBestMove
parse_bestmove(line: str) -> UsiBestMove
```

エンジンの `bestmove` 応答行を表すイミュータブルなデータクラスである。

| プロパティ / メソッド | 型 | 説明 |
|---|---|---|
| `bestmove` | `Move` | 最善手（投了時は `Move.RESIGN`、入玉宣言時は `Move.WIN`）。 |
| `ponder` | `Move \| None` | 予想応手（指定がない場合は `None`）。 |
| `to_string()` / `str(bm)` | `str` | `"bestmove 7g7f ponder 3c3d"` 形式の USI 文字列を返す。 |

### `UsiGoCommand`

```python
UsiGoCommand(
    *,
    searchmoves: tuple[Move, ...] = (),
    ponder: bool = False,
    btime: int | None = None,
    wtime: int | None = None,
    byoyomi: int | None = None,
    binc: int | None = None,
    winc: int | None = None,
    nodes: int | None = None,
    depth: int | None = None,
    mate: int | None = None,
    infinite: bool = False,
) -> UsiGoCommand
```

エンジンへ送信する `go` コマンドを組み立てるイミュータブルなデータクラスである。
`mate` に負の整数（例：`-1`）を指定すると `go mate infinite` として出力され、正の整数を指定すると詰み探索の制限時間（ミリ秒）として出力される。

| メソッド | 戻り値 | 説明 |
|---|---|---|
| `to_string()` / `str(cmd)` | `str` | 設定されたフィールドから `"go ..."` コマンド文字列を生成する。 |

```python
from rsshogi.core import Move
from rsshogi.usi import UsiGoCommand, parse_bestmove

cmd = UsiGoCommand(
    searchmoves=(Move.from_usi("7g7f"), Move.from_usi("2g2f")),
    btime=60000,
    wtime=60000,
    byoyomi=10000,
    depth=20,
)
print(cmd.to_string())
# => "go searchmoves 7g7f 2g2f btime 60000 wtime 60000 byoyomi 10000 depth 20"

bm = parse_bestmove("bestmove 7g7f ponder 3c3d")
print(bm.bestmove.to_usi(), bm.ponder.to_usi() if bm.ponder else None)
# => 7g7f 3c3d
```

---

## USI `position` 文字列の解析と正規化（`rsshogi.core`）

USI の `position` コマンド文字列（`"position startpos moves 7g7f 3c3d"` や `"sfen ... moves ..."`）を検証し、初期局面、合法手リスト、最終局面に分解する関数群が `rsshogi.core` に用意されている。
手順中に非合法手や不正なトークンが含まれている場合は `ValueError` を送出する。

| 関数 / クラス | シグネチャ | 説明 |
|---|---|---|
| `parse_usi_position(position)` | `(position: str) -> tuple[str, list[Move]]` | USI `position` 文字列を解析し、`(初期局面の SFEN, 合法手の Move リスト)` のタプルを返す。 |
| `parse_usi_position_parts(position)` | `(position: str) -> UsiPositionParts` | USI `position` 文字列を解析し、初期局面、手順、最終局面をまとめた `UsiPositionParts` を返す。 |
| `normalize_usi_position(position)` | `(position: str) -> str` | USI `position` 文字列を検証したうえで、正規化された `"position startpos ..."` または `"position sfen ..."` 文字列へ変換する。 |

### `UsiPositionParts`

`parse_usi_position_parts(position)` が返すオブジェクトは以下のプロパティとメソッドを持つ。

| プロパティ / メソッド | 型 | 説明 |
|---|---|---|
| `initial_sfen` | `str` | 開始局面の正規化された SFEN 文字列。 |
| `moves` | `list[Move]` | 合法性を検証済みの 16 ビット `Move` のリスト。 |
| `move_usi` | `list[str]` | 各指し手の USI 文字列のリスト。 |
| `final_sfen` | `str` | すべての指し手を適用した後の最終局面の SFEN 文字列。 |
| `to_dict()` | `dict[str, Any]` | `initial_sfen`、`moves`、`move_usi`、`final_sfen` の 4 キーを持つ辞書を返す。 |

```python
from rsshogi.core import normalize_usi_position, parse_usi_position_parts

parts = parse_usi_position_parts("startpos moves 7g7f 3c3d")
print(parts.initial_sfen)
# => "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"
print(parts.move_usi)
# => ['7g7f', '3c3d']
print(parts.final_sfen)
# => "lnsgkgsnl/1r5b1/pppppp1pp/6p2/9/2P6/PP1PPPPPP/1B5R1/LNSGKGSNL b - 3"

normalized = normalize_usi_position("startpos moves 7g7f")
assert normalized == "position startpos moves 7g7f"
```

### `Board` および `Record` との連携

- **盤面への直接適用**：[`Board.set_usi_position(position)`](board.md#局面の初期化と入出力) を使うと、`Board` インスタンスに初期局面と手順を直接反映できる。
- **棋譜オブジェクトとの相互変換**：[`Record.from_usi_position(position, *, allow_special_tokens=False)`](record.md#読み込みクラスメソッド) と [`Record.to_usi_position(*, include_special_tokens=False)`](record.md#書き出しインスタンスメソッド) を使うと、開始局面が `startpos` であったか `sfen ...` であったかを保持したまま `Record` と相互変換できる。
- **探索テレメトリ付き棋譜の生成**：対局実行ループで収集した USI 手順と評価値、ノード数、消費時間から `Record` を作る場合は [`Record.from_usi_main_line(...)`](record.md#recordfrom_usi_main_line-のシグネチャ) を使用する。

## 関連項目

- [盤面管理（`Board`）](board.md)
- [指し手クラス（`Move`）](move.md)
- [棋譜管理（`Record`）](record.md)
