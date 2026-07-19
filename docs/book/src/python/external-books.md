# 外部定跡（DB2016 / SBK）

DB2016 や SBK といった、他エンジン向けの定跡ファイルを **読み取り専用**で
参照するためのクラスです。rsshogi 独自の定跡バイナリ（[`StaticBook` / `MemoryBook`](book.md)）
とは別系統の API で、外部ファイルをそのまま開いて検索します。

```python
import rsshogi as rs

yo_book = rs.book.YaneuraOuBook.open("book.db")   # DB2016
sbk_book = rs.book.SbkBook.open("book.sbk")        # SBK
```

| クラス | 対象ファイル | 用途 |
|--------|-------------|------|
| `YaneuraOuBook` | DB2016 `.db` | 大規模 DB2016 定跡をメモリに全展開せず参照 |
| `SbkBook` | SBK `.sbk` | ShogiHome 系 SBK 定跡の参照・ツリー探索 |

---

## DB2016

### YaneuraOuBook

| メソッド | 戻り値 | 説明 |
|----------|--------|------|
| `YaneuraOuBook.open(path)` | `YaneuraOuBook` | `.db` ファイルを開く（クラスメソッド） |
| `diagnostics()` | `YaneuraOuBookDiagnostics` | ソート状態など参照可否の診断を返す |
| `validate_full()` | `YaneuraOuBookDiagnostics` | ファイル全体のソートを検証する |
| `lookup_sfen(sfen)` | `YaneuraOuBookEntry \| None` | SFEN 文字列で定跡を検索 |
| `lookup_position(board)` | `YaneuraOuBookEntry \| None` | `Board` から検索 |
| `lookup_sfen_by_scan(sfen)` | `YaneuraOuBookEntry \| None` | 全件スキャンで検索（明示的に走査する場合のみ） |

### YaneuraOuBookEntry

| プロパティ | 型 | 説明 |
|------------|-----|------|
| `sfen` | `str` | 局面の SFEN |
| `min_ply` | `int` | 元データの手数（キーは手数を `1` に正規化して照合） |
| `comment` | `str` | エントリーのコメント |
| `moves` | `list[YaneuraOuBookMove]` | 候補手のリスト |

`len(entry)` で候補手数を取得できます。

### YaneuraOuBookMove

| プロパティ | 型 | 説明 |
|------------|-----|------|
| `mv` | `Move` | 指し手（16bit） |
| `ponder` | `Move` | 予想応手（ponder） |
| `score` | `int \| None` | 評価値 |
| `depth` | `int \| None` | 探索深さ |
| `count` | `int \| None` | 採用回数 |
| `comment` | `str` | 指し手コメント |

### 参照可否の診断

`open()` は先頭の一定行だけを検証して開きます。大きなファイルでも開く時間を抑えるためです。
`diagnostics()` は二分探索が安全に使えるかを返します。

- `diagnostics().complete` が `False` の場合、`validate_full()` でファイル全体の
  ソートを確認するまで `lookup_sfen()` はエラーになります。
- ファイルがソートされていない場合も、`lookup_sfen()` は（数 GB のファイルを暗黙に
  全走査しないよう）エラーになります。全件スキャンを意図する場合のみ
  `lookup_sfen_by_scan()` を使ってください。

`YaneuraOuBookDiagnostics` の主なプロパティ:

| プロパティ | 型 | 説明 |
|------------|-----|------|
| `kind` | `str` | `"sorted"` / `"unsorted"` / `"invalid_header"` / `"unvalidated"` |
| `complete` | `bool \| None` | ソート済みで全体が検証済みか（`sorted` 時のみ） |
| `checked_rows` | `int \| None` | 検証済みの行数 |
| `line_number` | `int \| None` | 非ソート検出位置（`unsorted` 時） |

> DB2016 のテキストは UTF-8 として解釈されます。日本語の旧ファイルで
> パースに失敗する場合は、元データが CP932 / Shift_JIS でないか確認してください。

### 例

```python
import rsshogi as rs

book = rs.book.YaneuraOuBook.open("book.db")

# 全体が検証済みでなければ、必要に応じて全検証
if book.diagnostics().complete is False:
    book.validate_full()

entry = book.lookup_position(rs.core.Board())
if entry is not None:
    print(entry.min_ply, entry.comment)
    for move in entry.moves:
        print(move.mv.to_usi(), move.ponder.to_usi(), move.score, move.depth, move.count, move.comment)
```

---

## SBK

### SbkBook

| メソッド | 戻り値 | 説明 |
|----------|--------|------|
| `SbkBook.open(path)` | `SbkBook` | `.sbk` ファイルを開く（クラスメソッド） |
| `SbkBook.open_with_control(path, callback)` | `SbkBook` | 進捗通知・キャンセル付きで開く（クラスメソッド） |
| `diagnostics()` | `SbkDiagnostics` | 重複局面・未解決状態の診断を返す |
| `lookup_sfen(sfen)` | `SbkEntry \| None` | SFEN 文字列で検索 |
| `lookup_position(board)` | `SbkEntry \| None` | `Board` から検索 |
| `lookup_state_id(state_id)` | `SbkEntry \| None` | デコード済み state id で検索 |
| `lookup_state_index(state_index)` | `SbkEntry \| None` | 物理 state payload インデックスで検索 |
| `child_entry(parent, index)` | `SbkEntry \| None` | 親エントリーの子局面を取得 |

| プロパティ | 型 | 説明 |
|------------|-----|------|
| `author` | `str \| None` | 定跡の作者 |
| `description` | `str \| None` | 定跡の説明 |

`len(book)` で局面数を取得できます。`lookup_state_id()` は SBK の `id` メタデータを使い、
物理インデックスと同一とは仮定しません。`lookup_state_index()` は物理インデックスを指します。
不明な id・範囲外インデックス・`next_state_id` を持たない手・範囲外の手インデックスは
いずれも `None` を返します。

### SbkEntry

| プロパティ | 型 | 説明 |
|------------|-----|------|
| `state_id` | `int \| None` | デコード済み state id |
| `state_index` | `int` | 物理 state payload インデックス |
| `sfen` | `str` | 局面の SFEN |
| `games` | `int \| None` | 対局数 |
| `won_black` | `int \| None` | 先手勝ち数 |
| `won_white` | `int \| None` | 後手勝ち数 |
| `comment` | `str \| None` | コメント |
| `moves` | `list[SbkMove]` | 候補手のリスト |
| `evals` | `list[SbkEval]` | 評価情報のリスト |

`len(entry)` で候補手数を取得できます。

### SbkMove

| プロパティ | 型 | 説明 |
|------------|-----|------|
| `mv` | `Move` | 指し手（16bit） |
| `raw_move` | `int` | 変換前の生の指し手値 |
| `evaluation` | `int \| None` | 評価値 |
| `weight` | `int \| None` | 重み |
| `next_state_id` | `int \| None` | 遷移先 state id |

### SbkEval

`SbkEntry.evals` から取得できる評価情報オブジェクトです。各フィールドはすべて `None`
になる場合があります。

| プロパティ | 型 | 説明 |
|------------|-----|------|
| `evaluation_value` | `int \| None` | 評価値（センチポーン単位） |
| `depth` | `int \| None` | 探索深度 |
| `sel_depth` | `int \| None` | 選択深度 |
| `nodes` | `int \| None` | 探索ノード数 |
| `variation` | `str \| None` | 変化手順（USI 手順文字列） |
| `engine_name` | `str \| None` | 評価を計算したエンジン名 |

### 例

```python
import rsshogi as rs

book = rs.book.SbkBook.open("book.sbk")
entry = book.lookup_position(rs.core.Board())

if entry is not None:
    print(entry.games, entry.won_black, entry.won_white)
    for move in entry.moves:
        print(move.mv.to_usi(), move.weight, move.evaluation, move.next_state_id)
    for ev in entry.evals:
        print(ev.evaluation_value, ev.depth, ev.engine_name)
```

### 進捗通知付きで開く

インデックス構築の進捗取得や協調的なキャンセルが必要な場合は
`open_with_control(path, callback)` を使います。`callback` は `SbkOpenProgress`
（`indexed_states` / `total_states`）を受け取ります。`False` を返すと open を
キャンセルし、`None` または `True` を返すと続行します。

```python
import rsshogi as rs

progress = []

book = rs.book.SbkBook.open_with_control(
    "book.sbk",
    lambda step: progress.append((step.indexed_states, step.total_states)),
)
```

### ツリー探索

SBK は局面ツリーを持つため、子局面を直接たどれます。

```python
import rsshogi as rs

book = rs.book.SbkBook.open("book.sbk")
parent = book.lookup_position(rs.core.Board())
if parent is not None:
    child = book.child_entry(parent, 0)
```

`SbkDiagnostics` は、インデックス構築後の重複 Packed SFEN 局面
（`duplicate_positions`）と未解決の state payload（`unresolved_next_states`）を報告します。

---

## Packed SFEN

外部定跡ツールと相互運用する場合、`Board.to_packed_sfen()` が 32 バイトの
Packed SFEN キーを返します。詳細は [NumPy 連携](numpy.md) と
[Board](board.md#to_packed_sfenoutnone) を参照してください。

---

## 関連項目

- [定跡 (Book)](book.md) - rsshogi 独自の定跡バイナリ（`StaticBook` / `MemoryBook`）
- [定跡バイナリ形式](../reference/formats/book.md) - rsshogi 定跡バイナリのフォーマット仕様
- [外部定跡フォーマット（リファレンス）](../reference/formats/external-books.md) - DB2016 / SBK の詳細
- [Board](board.md) - 盤面管理クラス
