# 定跡（`rsshogi.book` / 外部定跡）

`rsshogi.book` は定跡（Opening Book）の構築、保存、検索を行うためのモジュールである。
`rsshogi` 独自のバイナリ定跡（`MemoryBook` / `StaticBook`）に加えて、やねうら王形式のテキスト定跡（`YaneuraOuBook`）および SBK 形式のバイナリ定跡（`SbkBook`）の読み取り専用リーダーを提供する。

```python
from rsshogi.book import (
    BookBuilder,
    BookEntry,
    BookKey,
    BookMove,
    MemoryBook,
    SbkBook,
    SbkDiagnostics,
    SbkEntry,
    SbkEval,
    SbkMove,
    SbkOpenProgress,
    StaticBook,
    YaneuraOuBook,
    YaneuraOuBookDiagnostics,
    YaneuraOuBookEntry,
    YaneuraOuBookMove,
    book_builder_from_game_record,
    book_key_after,
    book_key_from_position,
)
```

## 定跡クラスの使い分け

| 区分 | クラス | 特徴と用途 |
|---|---|---|
| ネイティブ定跡（編集用） | `MemoryBook` / `BookBuilder` | メモリ上で棋譜や探索結果から候補手を追加および集約する。 |
| ネイティブ定跡（配布および検索用） | `StaticBook` | バイナリファイル（`.bin`）へ保存し、高速に検索する（仕様は [定跡バイナリ形式](../reference/formats/book.md) を参照）。 |
| 外部定跡（DB2016 形式） | `YaneuraOuBook` | `#YANEURAOU-DB2016 1.00` 形式のテキスト定跡ファイルをインデックス化して検索する。 |
| 外部定跡（SBK 形式） | `SbkBook` | 局面グラフ構造を持つ SBK バイナリ定跡ファイルを検索および遷移する。 |

各定跡フォーマットの内部構造と設計方針については [定跡フォーマットのアーキテクチャ](../reference/formats/book-architecture.md) を参照。

---

## ネイティブ定跡（`MemoryBook` / `StaticBook` / `BookBuilder`）

### `BookKey` とキー計算関数

`BookKey` は定跡検索に使用する局面の Zobrist ハッシュキーを表すイミュータブルなクラスである。
配布版および既定のビルドでは 64 ビットの検索キーを使用し、`hash-128` feature を有効にしたカスタムビルドでは 128 ビットの検索キーを使用する。

| プロパティ / 関数 | 型 / シグネチャ | 説明 |
|---|---|---|
| `key.low` | `int` | キーの下位 64 ビット符号なし整数。 |
| `key.high` | `int` | キーの上位 64 ビット符号なし整数（既定の 64 ビットビルドでは常に `0`）。 |
| `int(key)` | `int` | `(high << 64) \| low` の整数値を返す。 |
| `book_key_from_position(board)` | `(board: Board) -> BookKey` | 現在の盤面から定跡検索用の `BookKey` を計算する。 |
| `book_key_after(board, mv)` | `(board: Board, mv: Move) -> BookKey` | 現在の盤面で `mv` を 1 手進めた後の局面の `BookKey` を計算する（盤面自体は変更しない）。 |

`BookKey` は `==` / `!=` による等値比較と `hash(key)` に対応しており、Python の `dict` のキーや `set` の要素として使用できる。

### `BookMove` と `BookEntry`

```python
BookMove(mv: Move, score: int, depth: int) -> BookMove
```

`BookMove` は定跡内の 1 つの候補手を表し、`BookEntry` は 1 局面に対する候補手のリストを保持する。

| クラス | プロパティ / メソッド | 型 | 説明 |
|---|---|---|---|
| `BookMove` | `mv` | `Move` | 16 ビットの指し手。 |
| `BookMove` | `score` | `int` | 評価値（`int16` 範囲のセンチポーン値）。 |
| `BookMove` | `depth` | `int` | 探索深さ（`uint16` 範囲の非負整数）。 |
| `BookEntry` | `key` | `BookKey` | 局面のハッシュキー。 |
| `BookEntry` | `moves` | `list[BookMove]` | 登録されている候補手のリスト。 |
| `BookEntry` | `len(entry)` | `int` | 候補手の件数。 |

### `MemoryBook`

```python
MemoryBook() -> MemoryBook
```

メモリ上で候補手を追加および検索できる可変の定跡クラスである。

| メソッド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `insert_move(key, book_move)` | `key: BookKey`, `book_move: BookMove` | `None` | 指定した局面キーに候補手を追加する。 |
| `get(key)` | `key: BookKey` | `BookEntry \| None` | 指定した局面キーの `BookEntry` を返す。未登録の場合は `None` を返す。 |
| `contains(key)` | `key: BookKey` | `bool` | 指定した局面キーが登録されていれば `True` を返す。 |
| `len(book)` | なし | `int` | 登録されている局面数を返す。 |

### `StaticBook`

バイナリ形式でシリアライズ可能な読み取り専用の定跡クラスである。

| メソッド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `StaticBook.from_file(path)` | `path: str \| PathLike` | `StaticBook` | バイナリ定跡ファイルから `StaticBook` を読み込む（クラスメソッド）。 |
| `StaticBook.from_bytes(data)` | `data: bytes` | `StaticBook` | バイト列から `StaticBook` を復元する（クラスメソッド）。 |
| `StaticBook.from_memory_book(book)` | `book: MemoryBook` | `StaticBook` | `MemoryBook` から `StaticBook` を構築する（クラスメソッド）。 |
| `write_file(path)` | `path: str \| PathLike` | `None` | 定跡データをバイナリファイルへ書き出す。 |
| `to_bytes()` | なし | `bytes` | 定跡データをバイト列へシリアライズする。 |
| `get(key)` | `key: BookKey` | `BookEntry \| None` | 指定した局面キーの `BookEntry` を返す。未登録の場合は `None` を返す。 |
| `contains(key)` | `key: BookKey` | `bool` | 指定した局面キーが登録されていれば `True` を返す。 |
| `len(book)` | なし | `int` | 登録されている局面数を返す。 |

### `BookBuilder` と `book_builder_from_game_record`

```python
BookBuilder() -> BookBuilder
book_builder_from_game_record(
    record: Record,
    default_score: int = 0,
    default_depth: int = 0,
) -> MemoryBook
```

`BookBuilder` は複数の棋譜（本手順および変化手順）や個別の局面から定跡を構築するビルダークラスである。
`into_memory()` または `build_static()` を呼び出すと、ビルダー内部のデータが取り出されてビルダー自身は空の状態に戻る。

| メソッド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `insert(key, mv, score, depth)` | `key: BookKey`, `mv: Move`, `score: int`, `depth: int` | `None` | 指定した `BookKey` に対して候補手を追加する。 |
| `insert_for_position(board, mv, score, depth)` | `board: Board`, `mv: Move`, `score: int`, `depth: int` | `None` | `board` から自動的にキーを計算して候補手を追加する。 |
| `extend_from_game_record(record, default_score=0, default_depth=0)` | `record: Record`, `default_score: int = 0`, `default_depth: int = 0` | `None` | 棋譜の本手順と変化手順を走査し、各手の `engine_info`（未設定時はデフォルト値）を用いて定跡へ取り込む。 |
| `into_memory()` | なし | `MemoryBook` | 蓄積した定跡データを `MemoryBook` として取り出す。 |
| `build_static()` | なし | `StaticBook` | 蓄積した定跡データを `StaticBook` として取り出す。 |

```python
from rsshogi.book import BookBuilder, StaticBook, book_key_from_position
from rsshogi.core import Board, Move
from rsshogi.record import Record

builder = BookBuilder()
record = Record.from_usi_main_line("startpos", ["7g7f", "3c3d"], evals=[30, -20])
builder.extend_from_game_record(record, default_score=0, default_depth=12)

board = Board()
builder.insert_for_position(board, Move.from_usi("2g2f"), score=25, depth=14)

static_book = builder.build_static()
entry = static_book.get(book_key_from_position(board))
if entry is not None:
    for bm in entry.moves:
        print(bm.mv.to_usi(), bm.score, bm.depth)
```

---

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

- [定跡バイナリ形式](../reference/formats/book.md)
- [定跡フォーマットのアーキテクチャ](../reference/formats/book-architecture.md)
- [盤面管理（`Board`）](board.md)
- [指し手クラス（`Move`）](move.md)
- [棋譜管理（`Record`）](record.md)
