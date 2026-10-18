# 定跡管理

本章は圧縮配置の Native Book v4 と Python API を説明する。

Native Book v4 と外部定跡の reader を利用できる。形式の詳細は [バイナリ仕様](../reference/formats/book.md) と [定跡アーキテクチャ](../reference/formats/book-architecture.md) を参照。

## Native Book v4

`NativeBook` はファイルの必要な部分を読む reader、`NativeBookEditor` は排他ロックを持つ編集用 writer、`NativeBookBuilder` は新規ファイルを構築する builder である。検証と保存は Rust core が行う。

```python
from rsshogi.core import Board, Move
from rsshogi.book import (
    NativeBook, NativeBookEditor, NativeBookMetadata,
    NativeBound, NativeCandidate, NativeScore,
)

board = Board()
editor = NativeBookEditor.create("example.book", NativeBookMetadata("定跡集", depth_kind=1))
editor.upsert_candidate(
    board,
    NativeCandidate(Move.from_usi("7g7f"), NativeScore.cp(25, NativeBound.Exact),
                    depth=24),
    comment="本線",
)
editor.set_position_comment(board, "初期局面")
editor.save()
editor.close()

book = NativeBook.open("example.book")
book.validate_full()
entry = book.get(board)
record = book.record(board)
```

新規構築では `NativeBookBuilder(path, metadata=None, block_positions=64)` の `block_positions` をキーワードで指定できる。
範囲は 1..=16384、既定と推奨値は 64 である。
保存値は `book.block_positions` で取得し、`editor.compact()` と `salvage_prefix()` でも元の値を維持する。

```python
from rsshogi.book import NativeBookBuilder, NativeBookEntry, NativeBookPosition

builder = NativeBookBuilder("custom-blocks.book", block_positions=256)
builder.push(NativeBookPosition(NativeBookEntry(board, [NativeCandidate(Move.from_usi("7g7f"))])))
builder.finish()
assert NativeBook.open("custom-blocks.book").block_positions == 256
```

評価は `NativeScore.unknown()`、`cp(value, bound)`、`mate(plies, bound)`、`mate_unknown(winning)` で区別する。視点は指す前の手番側で、詰み距離は正なら勝ち、負なら負け。`kind`、`value`、`bound`、`winning` で値を取得できる。

`NativeCandidate(mv, score=None, depth=None, display_order=0)` の depth は任意の `u32` 値であり、`None` は未設定、0 は設定済みの値を表す。
`NativeBookMetadata(title=None, description=None, depth_kind=0)` は文書全体の深さの意味を指定し、0 は未指定、1 は探索深さ、2 は定跡内の後退解析距離である。
その他の `u32` 値も保存、再 open と compaction で保持する。
メタデータは `book.metadata()` または `editor.metadata()` で取得し、`editor.set_metadata(...)` で置換する。

`NativeBookEntry(board, candidates)` は候補手を検証し、raw 指し手順に整列する。表示順は `display_order` が別に保持する。`NativeBookPosition(entry, comment, move_comments)` は所有 snapshot で、`move_comments` は raw 指し手整数 (`int(move)`) から本文への辞書である。候補手ゼロの局面にもコメントを残せる。

`upsert_candidate(..., comment=None)` は既存の候補手コメントを保持し、空文字は削除する。`reorder_candidate` は表示順位を変更する。`save()` は変更がなければ `False` を返し、追記しない。`close()` はロックを解放し、未保存の変更を破棄する。reader は open 時点の snapshot を保持するため、保存後の変更を読むには開き直す。

`open` は完全検証を意味しない。由来不明のファイルは `validate_full()` で検証する。`get()` は検索データ、`record()` はコメント込みの局面を返す。`position_comment(board)` と `move_comment(board, mv)` は指定した本文だけを取得し、同じ局面のほかのコメント本文を読まない。`entries()` と `records()` は所有リストを返すため、大規模ファイルの全件展開では必要な RAM に注意する。Rust 側には streaming iterator がある。`tail()` は未完了 tail の `(offset, end)` または `None`、`io_stats()` は累積の要求 `(read_calls, read_bytes)` を返す。物理ディスクの I/O 量ではない。

I/O 失敗は `OSError`、writer 競合は `BlockingIOError`、不正データは `ValueError`。`NativeBookLogError` は `offset`、`valid_end`、`sequence`、`reason` を持つ `ValueError` である。`NativeBookSaveUncertainError` は保存結果不明を表す `RuntimeError` で、editor の再 open が必要になる。

`recover_tail(path)` は正常な開始ヘッダーを持つ未完了 tail を別ファイルへ保全してから復旧し、保全先を返す。未検証 tail や破損には使わない。`salvage_prefix(path, output)` は原本を変更せず、完全検証した本体とログの prefix を新規ファイルへ救出する。報告には原本長、検証済み末尾、sequence、異常 offset、出力先が含まれる。救出は本体全件の検証と再構築を伴い、時間と別ファイル分の空き容量が必要である。

旧 `MemoryBook`、`StaticBook`、`BookBuilder` とハッシュキー API は 1.5.0 の開発版で削除した。旧 v2 単独から局面を復元できないため、元の棋譜や外部形式から再生成する。

## やねうら王形式定跡（`YaneuraOuBook`）

`#YANEURAOU-DB2016 1.00` ヘッダを持つテキスト定跡ファイルを読み取り専用で検索する。
`YaneuraOuBook.open(path)` はファイル全体を展開せず、局面ブロックのオフセット索引を構築したうえで、検索時に該当ブロックのみを遅延デコードする。

### `YaneuraOuBook` のプロパティとメソッド

| プロパティ / メソッド | シグネチャ / 型 | 説明 |
|---|---|---|
| `YaneuraOuBook.open(path)` | `(path: str \| PathLike) -> YaneuraOuBook` | DB2016 形式の定跡ファイルを開き、インデックスを構築する（クラスメソッド）。 |
| `diagnostics` | `YaneuraOuBookDiagnostics` | ファイルオープン時に検出された SFEN 整列状態の診断情報。 |
| `lookup_sfen(sfen)` | `(sfen: str) -> YaneuraOuBookEntry \| None` | SFEN 文字列で局面を検索する。持ち駒の並び順は正規化され、末尾の手数は無視される。 |
| `lookup_position(board)` | `(board: Board) -> YaneuraOuBookEntry \| None` | `Board` の現在局面で定跡を検索する。 |
| `len(book)` | `int` | インデックス化されたユニークな局面数を返す。 |

### `YaneuraOuBookEntry` / `YaneuraOuBookMove` / `YaneuraOuBookDiagnostics`

| クラス | プロパティ | 型 | 説明 |
|---|---|---|---|
| `YaneuraOuBookEntry` | `sfen` | `str` | 手数を除いた正規化済み SFEN 文字列（例：`"lnsgkgsnl/... b -"`）。 |
| `YaneuraOuBookEntry` | `ply` | `int \| None` | 定跡ファイル内の `sfen` 行に記録されていた手数（省略されていた場合は `None`）。 |
| `YaneuraOuBookEntry` | `moves` | `list[YaneuraOuBookMove]` | 候補手のリスト（`len(entry)` で件数を取得可能）。 |
| `YaneuraOuBookMove` | `best_move_usi` | `str` | 候補手の USI 文字列。 |
| `YaneuraOuBookMove` | `ponder_move_usi` | `str \| None` | 予想応手（ponder）の USI 文字列（`"none"` などの場合は `None`）。 |
| `YaneuraOuBookMove` | `eval` | `int` | 評価値（センチポーン）。 |
| `YaneuraOuBookMove` | `depth` | `int` | 探索深さ。 |
| `YaneuraOuBookMove` | `count` | `int \| None` | 採択回数（記録されていない場合は `None`）。 |
| `YaneuraOuBookDiagnostics` | `kind` | `str` | すべての局面行が昇順なら `"CLEAN"`、未整列箇所があれば `"UNSORTED_SFEN_ORDER"`。 |
| `YaneuraOuBookDiagnostics` | `checked_rows` | `int` | オープン時に検査した局面行数。 |
| `YaneuraOuBookDiagnostics` | `complete` | `bool` | 全局面行の検査が完了していれば `True`。 |
| `YaneuraOuBookDiagnostics` | `line_number` | `int \| None` | 最初に未整列が検出された行番号（整列済みの場合は `None`）。 |
| `YaneuraOuBookDiagnostics` | `previous_sfen` | `str \| None` | 未整列検出箇所の直前の SFEN（整列済みの場合は `None`）。 |
| `YaneuraOuBookDiagnostics` | `current_sfen` | `str \| None` | 未整列検出箇所の当該 SFEN（整列済みの場合は `None`）。 |

```python
from rsshogi.book import YaneuraOuBook
from rsshogi.core import Board

book = YaneuraOuBook.open("user_book1.db")
print(book.diagnostics.kind, len(book))

board = Board()
entry = book.lookup_position(board)
if entry is not None:
    for move in entry.moves:
        print(move.best_move_usi, move.ponder_move_usi, move.eval, move.depth, move.count)
```

---

## SBK 形式定跡（`SbkBook`）

`SbkBook` は局面状態（state）と有向辺（move）のグラフ構造を持つ SBK バイナリ定跡ファイルを読み取り専用で扱うクラスである。
局面または SFEN からの検索に加えて、各候補手が持つ遷移先インデックス（`target_state_index`）や `child_entry(parent, move_index)` を使って定跡グラフを高速に辿ることができる。

### `SbkBook` のプロパティとメソッド

| プロパティ / メソッド | シグネチャ / 型 | 説明 |
|---|---|---|
| `SbkBook.open(path)` | `(path: str \| PathLike) -> SbkBook` | SBK 定跡ファイルを開く（クラスメソッド）。 |
| `SbkBook.open_with_control(path, progress_callback=None, should_cancel=None, poll_interval_records=4096)` | 進捗とキャンセル制御付きのオープン | 進捗通知コールバックとキャンセル判定コールバックを指定して SBK ファイルを開く（クラスメソッド）。 |
| `author` | `str` | ファイルヘッダに記録された作成者名。 |
| `description` | `str` | ファイルヘッダに記録された説明文。 |
| `diagnostics` | `SbkDiagnostics` | 総局面数、総候補手数、範囲外遷移先数の診断情報。 |
| `lookup_sfen(sfen)` | `(sfen: str) -> SbkEntry \| None` | SFEN 文字列から該当局面の `SbkEntry` を検索する（末尾の手数は無視される）。 |
| `lookup_position(board)` | `(board: Board) -> SbkEntry \| None` | `Board` の現在局面から `SbkEntry` を検索する。 |
| `lookup_state_id(state_id)` | `(state_id: int) -> SbkEntry \| None` | 64 ビットの `state_id`（定跡キー）から `SbkEntry` を検索する。 |
| `lookup_state_index(state_index)` | `(state_index: int) -> SbkEntry \| None` | 0 始まりの状態番号 `state_index` から `SbkEntry` を直接取得する。 |
| `child_entry(parent, move_index)` | `(parent: SbkEntry, move_index: int) -> SbkEntry \| None` | `parent.moves[move_index]` の遷移先にあたる局面の `SbkEntry` を返す。`move_index` が範囲外の場合は `IndexError` を送出する。 |
| `len(book)` | `int` | 登録されている局面状態数を返す。 |

### `SbkEntry` / `SbkMove` / `SbkEval` / `SbkDiagnostics` / `SbkOpenProgress`

| クラス | プロパティ | 型 | 説明 |
|---|---|---|---|
| `SbkEntry` | `state_index` | `int` | ファイル内の 0 始まりの状態インデックス。 |
| `SbkEntry` | `state_id` | `int` | 局面の 64 ビット識別子。 |
| `SbkEntry` | `moves` | `list[SbkMove]` | 候補手のリスト（`len(entry)` で件数を取得可能）。 |
| `SbkMove` | `move_usi` | `str` | 候補手の USI 文字列。 |
| `SbkMove` | `eval` | `SbkEval` | 評価値オブジェクト。 |
| `SbkMove` | `depth` | `int` | 探索深さ。 |
| `SbkMove` | `count` | `int` | 採択回数。 |
| `SbkMove` | `target_state_index` | `int` | この手を指した後の遷移先 `state_index`。 |
| `SbkMove` | `target_state_id` | `int \| None` | 遷移先局面の `state_id`（遷移先が範囲外の場合は `None`）。 |
| `SbkEval` | `raw` | `int` | SBK ファイル内の生の評価値整数。 |
| `SbkEval` | `kind` | `str` | `"NO_VALUE"`、`"CP"`、`"WIN"`（詰みまたは必勝）、`"LOSS"`（被詰または必敗）のいずれか。 |
| `SbkEval` | `cp` | `int \| None` | `kind == "CP"` の場合のセンチポーン評価値（それ以外は `None`）。 |
| `SbkEval` | `mate_plies` | `int \| None` | `kind` が `"WIN"` または `"LOSS"` の場合の詰み手数（それ以外は `None`）。 |
| `SbkEval` | `display_text` | `str` | 表示用に整形された評価値文字列（`str(eval)` と同一）。 |
| `SbkDiagnostics` | `total_states` | `int` | ファイル内の総状態数。 |
| `SbkDiagnostics` | `total_moves` | `int` | ファイル内の総候補手数。 |
| `SbkDiagnostics` | `out_of_range_move_targets` | `int` | `target_state_index` が有効範囲外を指していた候補手の件数。 |
| `SbkOpenProgress` | `phase` | `str` | 読み込みフェーズ（`"HEADER"`、`"STATES"`、`"MOVES"`、`"COMPLETE"`）。 |
| `SbkOpenProgress` | `completed_units` | `int` | 現在フェーズで処理済みのレコード数。 |
| `SbkOpenProgress` | `total_units` | `int` | 現在フェーズの総レコード数。 |
| `SbkOpenProgress` | `fraction` | `float` | 全体の進捗率（`0.0` から `1.0`）。 |

`SbkBook.open_with_control` の `should_cancel` コールバックが `True` を返した場合、読み込みは中断され `InterruptedError` が送出される。
また、`poll_interval_records` に `0` を渡すと `ValueError` を送出する。

```python
from rsshogi.book import SbkBook, SbkOpenProgress
from rsshogi.core import Board


def on_progress(p: SbkOpenProgress) -> None:
    print(f"{p.phase}: {p.fraction * 100:.1f}%")


book = SbkBook.open_with_control("opening.sbk", progress_callback=on_progress)
board = Board()
entry = book.lookup_position(board)
if entry is not None and len(entry) > 0:
    first_move = entry.moves[0]
    print(first_move.move_usi, first_move.eval.display_text)
    next_entry = book.child_entry(entry, 0)
```

## 関連項目

- [盤面管理](board.md)
- [指し手](move.md)
- [棋譜管理](record.md)
