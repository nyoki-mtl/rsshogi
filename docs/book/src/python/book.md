# 定跡管理

本章は圧縮配置の Native Book v4 と Python API を説明する。

1.7.2 でも Python の公開 API は維持する。
`BookDatabasePatch`、DB2016 / SBK / YBB の差分保存、YBB の全件診断と同一キー全行の取得、`NativeBook::validate_full_and_visit()` は Rust 向け API である。
既存の `SbkBook.open()` と Native builder には Rust core の内部改善が適用される。
変更範囲は [1.7.2 の変更内容](../getting-started/release-1.7.2.md)と [1.7.1 の変更内容](../getting-started/release-1.7.1.md)を参照。

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

保存先が既に存在する場合は、上書きせず `FileExistsError`（`OSError` のサブクラス）を送出する。
構築開始後に保存先が作られた場合も、`finish()` は `FileExistsError` を送出する。
不正なブロック局面数や重複局面は `ValueError` となる。

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

`#YANEURAOU-DB2016 1.00` ヘッダーを持つテキスト定跡を読み取り専用で検索する。
既定の `open(path)` は先頭 10,000 局面行までの整列を検査する。
`lookup_sfen()` と `lookup_position()` は全件の整列検査完了を必要とする。
診断が `sorted` でも `complete` が `False` なら `validate_full()` を実行し、未整列なら `lookup_sfen_by_scan()` を使う。
局面索引やファイル全体の候補手をメモリへ展開しない。

| メソッド | 戻り値 | 説明 |
|---|---|---|
| `YaneuraOuBook.open(path)` | `YaneuraOuBook` | 文字列のファイルパスから開く。 |
| `diagnostics()` | `YaneuraOuBookDiagnostics` | 整列状態と検査範囲を返す。 |
| `validate_full()` | `YaneuraOuBookDiagnostics` | 全件を検査し、診断情報を返す。 |
| `lookup_sfen(sfen)` | `YaneuraOuBookEntry \| None` | 正規化 SFEN で検索する。手数は検索キーに含めない。 |
| `lookup_position(board)` | `YaneuraOuBookEntry \| None` | `Board` の局面で検索する。 |
| `lookup_sfen_by_scan(sfen)` | `YaneuraOuBookEntry \| None` | ファイルを走査して検索する。 |

| クラス | プロパティ | 型と意味 |
|---|---|---|
| `YaneuraOuBookEntry` | `sfen` | `str`。手数を `1` に正規化した SFEN。 |
| `YaneuraOuBookEntry` | `min_ply` | `int`。元の手数。省略時は `0`。 |
| `YaneuraOuBookEntry` | `comment` | `str`。局面コメント。 |
| `YaneuraOuBookEntry` | `moves` | `list[YaneuraOuBookMove]`。`len(entry)` で候補手数を取得する。 |
| `YaneuraOuBookMove` | `mv` / `ponder` | `Move`。`to_usi()` で文字列に変換する。応手なしは `Move.MOVE_NONE`。 |
| `YaneuraOuBookMove` | `score` / `depth` / `count` | `int \| None`。評価値、探索深さ、採択回数。 |
| `YaneuraOuBookMove` | `comment` | `str`。候補手コメント。 |
| `YaneuraOuBookDiagnostics` | `kind` | `str`。`sorted`、`unsorted`、`invalid_header`、`unvalidated`。 |
| `YaneuraOuBookDiagnostics` | `checked_rows` | `int \| None`。検査済みの局面行数。 |
| `YaneuraOuBookDiagnostics` | `complete` | `bool \| None`。整列済みの場合に全件検査の完了状態を返す。 |
| `YaneuraOuBookDiagnostics` | `line_number` | `int \| None`。未整列を検出した行番号。 |
| `YaneuraOuBookDiagnostics` | `previous_sfen` / `current_sfen` | `str \| None`。未整列を検出した前後の SFEN。 |

```python
from rsshogi.book import YaneuraOuBook
from rsshogi.core import Board

book = YaneuraOuBook.open("user_book1.db")
book.validate_full()
print(book.diagnostics().kind)

board = Board()
entry = book.lookup_position(board)
if entry is not None:
    print(entry.min_ply, len(entry))
    for move in entry.moves:
        print(move.mv.to_usi(), move.ponder.to_usi(), move.score, move.depth, move.count)
```

## SBK 形式定跡（`SbkBook`）

SBK の局面状態と候補手を読み取り専用で扱う。
`state_id` はファイル内の識別子、`state_index` は物理的な格納順を表す。
`next_state_id` を物理インデックスとして扱わず、遷移先の取得には `child_entry()` を使う。

| メソッドまたはプロパティ | 戻り値 | 説明 |
|---|---|---|
| `SbkBook.open(path)` | `SbkBook` | 文字列のファイルパスから開く。 |
| `SbkBook.open_with_control(path, callback)` | `SbkBook` | 進捗付きで開く。callback は必須。 |
| `author` / `description` | `str \| None` | 作成者と説明。 |
| `diagnostics()` | `SbkDiagnostics` | 重複局面と未解決遷移先の件数を返す。 |
| `lookup_sfen(sfen)` / `lookup_position(board)` | `SbkEntry \| None` | 局面で検索する。 |
| `lookup_state_id(state_id)` | `SbkEntry \| None` | ファイル内の識別子で検索する。 |
| `lookup_state_index(state_index)` | `SbkEntry \| None` | 0 始まりの物理インデックスで検索する。 |
| `child_entry(parent, move_index)` | `SbkEntry \| None` | 候補手の遷移先を取得する。範囲外の候補手番号や未解決の遷移先は `None`。 |
| `len(book)` | `int` | 局面状態数。 |

| クラス | プロパティ | 型と意味 |
|---|---|---|
| `SbkEntry` | `state_index` / `state_id` | `int` / `int \| None`。物理インデックスと識別子。 |
| `SbkEntry` | `sfen` / `comment` | `str` / `str \| None`。局面とコメント。 |
| `SbkEntry` | `games` / `won_black` / `won_white` | `int \| None`。対局数、先手勝数、後手勝数。 |
| `SbkEntry` | `moves` / `evals` | `list[SbkMove]` / `list[SbkEval]`。候補手と局面評価の履歴。 |
| `SbkMove` | `mv` / `raw_move` | `Move` / `int`。変換済みの指し手とファイル内の生値。 |
| `SbkMove` | `evaluation` / `weight` / `next_state_id` | `int \| None`。評価値、重み、遷移先識別子。 |
| `SbkEval` | `evaluation_value` / `depth` / `sel_depth` / `nodes` | `int \| None`。評価値、深さ、選択深さ、ノード数。 |
| `SbkEval` | `variation` / `engine_name` | `str \| None`。読み筋とエンジン名。 |
| `SbkDiagnostics` | `duplicate_positions` / `unresolved_next_states` | `int`。重複局面数と未解決遷移先数。 |
| `SbkOpenProgress` | `indexed_states` / `total_states` | `int`。索引作成済み状態数と全状態数。 |

callback は `SbkOpenProgress` を受け取る。
`None` または真値を返すと継続し、`False` を返すとキャンセルする。
キャンセルと読み込みエラーは `ValueError`、callback 内の例外は元の例外として送出される。

```python
from rsshogi.book import SbkBook, SbkOpenProgress
from rsshogi.core import Board


def on_progress(progress: SbkOpenProgress) -> None:
    print(f"{progress.indexed_states}/{progress.total_states}")


book = SbkBook.open_with_control("opening.sbk", on_progress)
board = Board()
entry = book.lookup_position(board)
if entry is not None and len(entry) > 0:
    first_move = entry.moves[0]
    print(first_move.mv.to_usi(), first_move.evaluation, first_move.next_state_id)
    next_entry = book.child_entry(entry, 0)
```

## 関連項目

- [盤面管理](board.md)
- [指し手](move.md)
- [棋譜管理](record.md)
