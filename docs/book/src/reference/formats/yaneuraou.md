# YaneuraOu 定跡フォーマット仕様（DB2016 / YBB）

`rsshogi` は、[やねうら王（YaneuraOu）](https://github.com/yaneurao/YaneuraOu) で採用されている 2 種類の定跡フォーマットをサポートする。

1. **DB2016（`.db`）**：`#YANEURAOU-DB2016 1.00` ヘッダを持つ標準テキスト定跡フォーマット。`YaneuraOuBook` による読み取りと、`BookDatabase` からの書き出しに対応する。
2. **YBB（`.ybb`）**：`YANE-BINBOOK-V1\0` マジックを持つ固定長インデックス付きバイナリ定跡フォーマット。`YbbBook` による読み取りに対応する。

## 1. DB2016 テキストフォーマット（`.db`）

DB2016 は、1 局面ごとに `sfen` 行を置き、その直後に候補手行を並べる UTF-8 テキスト形式である。

```text
#YANEURAOU-DB2016 1.00
sfen lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1
7g7f 8c8d 0 32 1
2g2f 3c3d 0 32 1
sfen lnsgkgsnl/1r5b1/ppppppppp/9/9/2P6/PP1PPPPPP/1B5R1/LNSGKGSNL w - 2
3c3d 2g2f 0 30 5
```

### ヘッダとコメント行

- **ヘッダ行**：1 行目に `#YANEURAOU-DB2016 1.00` を記述する（省略されたファイルも読み取り可能）。UTF-8 BOM、LF、CRLF のいずれの改行コードも受理する。
- **独立コメント行**：`#` または `//` で始まる行はコメント行として扱われる。`sfen` 行の直後（最初の候補手行より前）にあるコメント行は局面コメント（`YaneuraOuBookEntry::comment`）に格納され、候補手行の直後にあるコメント行はその候補手のコメント（`YaneuraOuBookMove::comment`）に格納される。

### 局面行（`sfen` 行）と手数正規化

`sfen <盤面> <手番> <持駒> [手数]` の形式で局面を指定する。

- **検索キーの正規化**：`YaneuraOuBook` は検索時に手数（ply）を無視し、常に手数を `1` に正規化した SFEN（`<盤面> <手番> <持駒> 1`）を局面キーとして比較する。
- **元手数の保持**：ファイル上に記述された手数は `YaneuraOuBookEntry::min_ply()`（省略されていた場合は `0`）として保持される。

### 候補手行（定跡手行）

`sfen` 行に続く非コメント行は、半角スペース区切りの候補手行として解析される。

```text
move ponder score depth count [# インラインコメント]
```

| 列位置 | フィールド | 型（`YaneuraOuBookMove`） | 内容と省略規則 |
| :--- | :--- | :--- | :--- |
| 1 | `move` | `Move` | 現局面での指し手（USI 形式）。リーダは通常手に加えて `none` や `resign` も受理する。 |
| 2 | `ponder` | `Move` | 予想される相手の応手（USI 形式）。存在しない場合は `none`（`Move::NONE`）。 |
| 3 | `score` | `Option<i32>` | 評価値（単位：cp、手番視点）。`none` または省略時は `None`。 |
| 4 | `depth` | `Option<u32>` | 探索深さ。`none` または省略時は `None`。 |
| 5 | `count` | `Option<u64>` | 採択回数（出現回数）。`none` または省略時は `None`。 |
| 6 以降 | `comment` | `String` | 第 5 列より後ろのトークン（`#` や `//` 以降を含む）は候補手コメントとして保持される。 |

後方カラムは省略可能であり、例えば `4e7h+ none 540 38 1` や `9f9e none none none` のような行も受理される。

### アクセスモード（`YaneuraOuAccessMode`）と二分探索

`lookup_sfen` / `lookup_position` で二分探索を行うには、ファイル内の `sfen` 行が **正規化 SFEN のバイト辞書順（厳密な昇順、重複なし）** に並んでいる必要がある。
`YaneuraOuBook::open_with_options` では、`YaneuraOuAccessMode` によってオープン時の検証範囲と検索方式を制御できる。

| `YaneuraOuAccessMode` | オープン時の動作 | `lookup_sfen` の動作 |
| :--- | :--- | :--- |
| `SafeBinary { prefix_rows: 10_000 }`（既定） | 先頭 `prefix_rows` 局面行のみソート順を検証する。 | ファイル全体が検証済み（`complete == true`）であれば二分探索を行う。未完了の場合は `validate_full()` を呼ぶか `lookup_sfen_by_scan()` を使用する。 |
| `ValidateFullBeforeLookup` | オープン時にファイル全体のソート順を検証する。 | ソート済みであれば即座に二分探索を行う。 |
| `AssumeSortedAfterPrefix { prefix_rows }` | 先頭 `prefix_rows` 局面行のみ検証し、以降もソート済みとみなす。 | 即座に二分探索を行う（巨大ファイルを高速に開く GUI 等に向く）。 |
| `AssumeSortedByCaller` | オープン時の検証を省略する（診断結果は `Unvalidated`）。 | 呼び出し側のソート保証のもとで即座に二分探索を行う。 |
| `ScanOnly` | 診断用に先頭のみ検査し、検索は常に線形走査とする。 | 二分探索を使わず、ファイル先頭から線形走査を行う。 |

未ソートのファイルを検索する場合は `lookup_sfen_by_scan()` を呼び出すことで、アクセスモードに関わらず線形走査で検索できる。
また、全エントリを先頭からストリーミング処理する場合は `iter_entries()` を使用する。

### `BookDatabase` からの DB2016 書き出し

編集 IR である `BookDatabase` から `write_yaneuraou_db2016` または `to_yaneuraou_db2016_string` を呼び出すことで、DB2016 形式のテキストを出力できる。

- **局面のソート順**：正規化 SFEN のバイト辞書順（厳密な昇順）で出力される。同一の正規化 SFEN が重複している場合は既定でエラーとなり、`YaneuraOuDb2016WriteOptions::with_keep_last_on_duplicate()` 指定時のみ最後のエントリを採用する。
- **通常手の制約**：`YaneuraOuBook` リーダは `none` や `resign` を受理するが、`BookDatabase` の取り込みおよび書き出しでは **通常の合法手（`mv.is_normal()`）のみ** が許可される。
- **候補手の並び順と出力列**：既定では `score` の降順（`score` が `None` の手は末尾）に並べ替えられ、`with_preserved_move_order()` 指定時は登録順を維持する。各行は `move ponder score depth` の 4 列を必ず出力し、`count` が `Some` のときのみ第 5 列を出力する。

## 2. YBB バイナリフォーマット（`YANE-BINBOOK-V1`、`.ybb`）

YBB（YaneuraOu Binary Book）は、32 バイトの `PackedSfen` をキーとする固定長インデックス配列と、コンパクトな指し手配列で構成されるバイナリ定跡フォーマットである。
Rust の `rsshogi::book::YbbBook` で読み取り専用アクセスを提供する（Python バインディングには未公開）。

### ファイル全体構造とワイヤーレイアウト

YBB ファイルは、32 バイトのヘッダ、44 バイト × `N` 個のインデックス領域、および可変オフセットの指し手データ領域の 3 部で構成される。
すべての整数フィールドはリトルエンディアン（LE）である。

#### ヘッダ（32 バイト）

| オフセット | サイズ | 型 | フィールド名 | 内容と検証規則 |
| :--- | :--- | :--- | :--- | :--- |
| `0x00` | 16B | `[u8; 16]` | `magic` | `b"YANE-BINBOOK-V1\0"` 固定。不一致時は `BookError::InvalidFormat`。 |
| `0x10` | 8B | `u64` | `count` | インデックス領域に格納されている局面レコード数 `N`。 |
| `0x18` | 8B | `u64` | `flags` | ビット 0（`flags & 1`）が `1` のとき指し手レコードに `depth`（2B）を含む。未定義ビット（`flags & !1 != 0`）が立っている場合は `BookError::Unsupported`。 |

#### インデックスエントリ（44 バイト × `N`）

オフセット `32` から始まり、32 バイトの `packed_sfen` に続けて 12 バイトのメタ情報（計 44 バイト）が並ぶ。
インデックス配列全体は `PackedSfen::cmp_bytes`（32 バイト配列の辞書順）で非減少順にソートされていなければならない。

| オフセット | サイズ | 型 | フィールド名 | 説明 |
| :--- | :--- | :--- | :--- | :--- |
| `0x00` | 32B | `[u8; 32]` | `packed_sfen` | YaneuraOu 互換の 32 バイト `PackedSfen`。 |
| `0x20` | 8B | `u64` | `moves_offset` | 指し手データ領域の先頭（`32 + 44 * count`）からのバイトオフセット。 |
| `0x28` | 2B | `u16` | `ply` | 局面の手数。 |
| `0x2A` | 2B | `u16` | `move_count` | この局面に属する候補手の個数 `k`。 |

#### 指し手レコード（4 バイトまたは 6 バイト × `k`、`YbbMove`）

指し手データ領域（オフセット `32 + 44 * count` 以降）には、各局面の候補手が連続格納される。
1 手あたりのサイズは `flags & 1 == 0` のとき 4 バイト、`flags & 1 == 1` のとき 6 バイトである。

| オフセット | サイズ | 型 | フィールド名 | 説明 |
| :--- | :--- | :--- | :--- | :--- |
| `0x00` | 2B | `u16` | `raw_move` | YaneuraOu `Move16` 形式の指し手（`Move::from_raw(raw_move)`）。通常の盤上移動または駒打ちである必要がある。 |
| `0x02` | 2B | `i16` | `eval` | 評価値（手番視点）。 |
| `0x04` | 2B | `u16` | `depth` | 探索深さ（ヘッダの `flags & 1 != 0` のときのみ存在する）。 |

### `YbbBook` の検索オプション（`YbbBookOpenOptions`）

- **`with_ignore_ply(bool)`**：`false`（既定）のときは `packed_sfen` と手数 `ply` の両方が一致する行を検索する。`true`（YaneuraOu の `IgnoreBookPly` 相当）のときは手数 `ply` の違いを無視し、同一 `packed_sfen` の先頭行を返す。
- **`with_flipped(bool)`**：`false`（既定）のときは入力局面そのままの向きで検索する。`true`（YaneuraOu の `FlippedBook` 相当）のときは、通常検索でヒットしなかった場合に盤面を 180 度回転および先後反転した局面で再検索し、ヒットした候補手の座標も反転して返す（`YbbEntry::flipped()` が `true` になる）。

## 3. コード例

### Python（DB2016）

```python
from rsshogi.book import YaneuraOuBook
from rsshogi.core import Board

book = YaneuraOuBook.open("user_book1.db")
diag = book.diagnostics()
if diag.kind == "sorted" and not diag.complete:
    book.validate_full()

board = Board()
entry = book.lookup_position(board)
if entry is not None:
    for m in entry.moves:
        print(m.mv.to_usi(), m.ponder.to_usi(), m.score, m.depth, m.count)
```

### Rust（DB2016 および YBB）

```rust
use rsshogi::board;
use rsshogi::book::{
    BookDatabase, YaneuraOuAccessMode, YaneuraOuBook, YaneuraOuBookOpenOptions,
    YaneuraOuDb2016WriteOptions, YbbBook, YbbBookOpenOptions,
};

board::init();
let pos = board::hirate_position();

// 1. DB2016 定跡のオープンと検索
let db_book = YaneuraOuBook::open_with_options(
    "user_book1.db",
    YaneuraOuBookOpenOptions::with_access_mode(
        YaneuraOuAccessMode::AssumeSortedAfterPrefix { prefix_rows: 10_000 },
    ),
)?;
if let Some(entry) = db_book.lookup_position(&pos)? {
    for mv in entry.moves() {
        println!("{} score={:?} count={:?}", mv.mv().to_usi(), mv.score(), mv.count());
    }
}

// 2. BookDatabase からの DB2016 書き出し
let db = BookDatabase::new();
let text = db.to_yaneuraou_db2016_string(&YaneuraOuDb2016WriteOptions::new())?;

// 3. YBB バイナリ定跡のオープンと検索
let ybb = YbbBook::open_with_options(
    "book.ybb",
    YbbBookOpenOptions::new().with_ignore_ply(true).with_flipped(true),
)?;
if let Some(entry) = ybb.lookup_position(&pos)? {
    for mv in entry.moves() {
        println!("{} eval={} depth={:?}", mv.mv().to_usi(), mv.eval(), mv.depth());
    }
}
```

## 関連項目

- [定跡 3 層アーキテクチャ](book-architecture.md)
- [StaticBook バイナリフォーマット仕様](book.md)
- [ShogiHome SBK フォーマット仕様](sbk.md)
