# USI プロトコル

`rsshogi.usi` は USI エンジンプロセスを起動・制御せず、USI プロトコルの
文字列を値オブジェクトへ変換するステートレスな補助 API です。

## USI position の分解

`parse_usi_position_parts()` は USI `position` 文字列を、初期局面 SFEN・手順・
最終局面 SFEN に分解します。

```python
from rsshogi.core import parse_usi_position_parts

parts = parse_usi_position_parts("position startpos moves 7g7f 3c3d")
print(parts.initial_sfen)
print(parts.move_usi)    # ["7g7f", "3c3d"]
print(parts.final_sfen)
```

戻り値 `UsiPositionParts` は以下の属性を持ちます。

| 属性 | 型 | 内容 |
| --- | --- | --- |
| `initial_sfen` | `str` | `startpos` または `sfen ...` が表す初期局面 |
| `moves` | `list[Move]` | 合法性を確認して読み込まれた手順 |
| `move_usi` | `list[str]` | `moves` を USI 文字列に戻したもの |
| `final_sfen` | `str` | 手順を適用した最終局面 |

## USI position と Record

`Record.from_usi_position()` は USI `position` command を棋譜として読み込みます。
`startpos` と `sfen ...` の違いは保持されるため、`to_usi_position()` で同じ
開始局面 token に戻せます。

```python
import rsshogi as rs

record = rs.record.Record.from_usi_position("position startpos moves 7g7f 3c3d")
assert record.to_usi_position() == "position startpos moves 7g7f 3c3d"
```

標準 USI では `position ... moves` に通常手だけを受け付けます。`resign` などの
非標準 token を終端特殊手として扱う場合は、明示的に extended mode を使います。

```python
record = rs.record.Record.from_usi_position(
    "position startpos moves 7g7f resign",
    allow_special_tokens=True,
)
assert record.to_usi_position(include_special_tokens=True).endswith("resign")
```

## info / bestmove の解析

`UsiInfo.parse()` / `parse_info()` は `info` 行から depth, nodes, score, bound,
PV などを取り出します。`UsiBestMove.parse()` / `parse_bestmove()` は
`bestmove` 行を読み込みます。

```python
import rsshogi as rs

info = rs.usi.parse_info(
    "info depth 12 nodes 12345 score cp 37 lowerbound pv 7g7f 3c3d"
)
assert info.depth == 12
assert info.score == 37
assert info.bound is rs.usi.UsiBound.LOWER

best = rs.usi.parse_bestmove("bestmove 7g7f ponder 3c3d")
assert best.bestmove.to_usi() == "7g7f"
```

`move_from_usi()` は `resign`, `win`, `0000`, `none` も `Move` 定数へ変換します。
通常手だけを受け付ける `Move.from_usi()` と使い分けてください。

## go コマンドの組み立て

`UsiGoCommand` は `go` コマンド文字列を組み立てる値オブジェクトです。

```python
import rsshogi as rs

cmd = rs.usi.UsiGoCommand(
    searchmoves=(rs.core.Move.from_usi("7g7f"),),
    btime=1000,
    wtime=1000,
    byoyomi=500,
    depth=10,
)
print(cmd.to_string())
# go searchmoves 7g7f btime 1000 wtime 1000 byoyomi 500 depth 10
```

## Record ヘルパー

`Record.from_usi_main_line()` は初期局面、USI 手順、任意のテレメトリ、
終局結果から合法手を適用しながら `Record` を作ります。

シグネチャ（すべてキーワードで渡せます）:

```python
Record.from_usi_main_line(
    init_position_sfen: str,
    usi_moves: list[str],
    result=None,
    move_times_ms: list[int | None] | None = None,
    evals: list[int | None] | None = None,
    nodes: list[int | None] | None = None,
    depths: list[int | None] | None = None,
    seldepths: list[int | None] | None = None,
    wall_times_ms: list[int | None] | None = None,
    latency_deltas_ms: list[int | None] | None = None,
    metadata=None,
    initial_comment: str | None = None,
) -> Record
```

```python
import rsshogi as rs

record = rs.record.Record.from_usi_main_line(
    "startpos",
    ["7g7f", "3c3d"],
    result=rs.record.GameResult.WHITE_WIN,
    evals=[12, -8],
    nodes=[1000, 2000],
)
```

終局結果だけから標準の終局特殊手を作る場合は
`SpecialMoveEntry.from_result()` を使います。

```python
terminal = rs.record.SpecialMoveEntry.from_result(
    rs.record.GameResult.BLACK_WIN_BY_TIMEOUT,
    time_ms=3000,
)
assert terminal.kind == "TIMEOUT"
```

## Board ヘルパー

`Board.push_usi_with_delta()` は合法手を適用し、差分 DTO を `dict` で返します。
`Board.evaluate_declaration()` は宣言勝ち判定の詳細条件を返します。
`Board.solve_mate_in_one()` は Rust core の 1 手詰めソルバを呼び出します。

```python
from rsshogi.core import Board

board = Board()
delta = board.push_usi_with_delta("7g7f")
assert delta["kind"] == "BOARD"

declaration = board.evaluate_declaration()
print(declaration["can_declare"])

mate = board.solve_mate_in_one()
```

---

## 関連項目

- [Board](board.md) - 盤面管理クラス
- [Record](record.md) - 棋譜レコード（`from_usi_main_line()`）
- [Move](move.md) - 16bit 指し手型
- [クイックスタート](../getting-started/quickstart.md)
