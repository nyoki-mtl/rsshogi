# YaneuraOu 定跡フォーマット仕様（DB2016 / YBB）

`rsshogi` は、[やねうら王（YaneuraOu）](https://github.com/yaneurao/YaneuraOu) で採用されている 2 種類の定跡フォーマットをサポートする。

1. **DB2016（`.db`）**：`#YANEURAOU-DB2016 1.00` ヘッダを持つ標準テキスト定跡フォーマット。`YaneuraOuBook` による読み取り、`YaneuraOuDb2016Writer` による逐次出力、`BookDatabase` からの書き出しに対応する。
2. **YBB（`.ybb`）**：`YANE-BINBOOK-V1\0` マジックを持つ固定長インデックス付きバイナリ定跡フォーマット。`YbbBook` による検索と全件走査、`YbbWriter` と `BookDatabase` による書き出しに対応する。

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
- **元手数の保持**：`YaneuraOuBookEntry::original_ply()` は省略時に `None`、明記された場合は `Some(ply)` を返す。明示的な `0` も保持する。`min_ply()` は省略時に `0` を返す。

### 候補手行（定跡手行）

`sfen` 行に続く非コメント行は、半角スペース区切りの候補手行として解析される。

```text
move ponder score depth count [# インラインコメント]
```

| 列位置 | フィールド | 型（`YaneuraOuBookMove`） | 内容と省略規則 |
| :--- | :--- | :--- | :--- |
| 1 | `move` | `Move` | 現局面での指し手（USI 形式）。リーダは通常手に加えて `none` や `resign` も受理する。 |
| 2 | `ponder` | `Move` | 予想される相手の応手（USI 形式）。存在しない場合は `none`（`Move::MOVE_NONE`）。 |
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
- **通常手の制約**：`YaneuraOuBook` リーダは `none` や `resign` を受理する。書き出しでは通常移動と駒打ちの符号化を検査し、特殊手を拒否する。局面に対する合法性は呼び出し側で確認する。
- **候補手の並び順と出力列**：既定では `score` の降順（`score` が `None` の手は末尾）に並べ替えられ、`with_preserved_move_order()` 指定時は登録順を維持する。各行は `move ponder score depth` の 4 列を必ず出力し、`count` が `Some` のときのみ第 5 列を出力する。

既定出力は `BookPosition::original_ply()` の省略と明示的な `0` を保持する。
`with_fixed_ply(n)` は `0` を含めて指定値を出力し、`with_omitted_ply()` は固定値指定より優先して手数を省略する。
`write_yaneuraou_db2016` は全件の整列と形式検証を済ませてから、1 エントリーずつ出力する。
出力全文の保持が必要な場合は `to_yaneuraou_db2016_string` を使う。

### DB2016 の逐次出力

`YaneuraOuDb2016Writer::new(output, options, expected_entry_count)` はヘッダーを直ちに出力する。
`write_entry(&entry)` に、正規化 SFEN のバイト辞書順で厳密な昇順の `BookDatabaseEntry` を渡す。
全件の IR は不要で、追加メモリは最大エントリーのサイズに比例する。
入力の整列と重複の採択は呼び出し側が行い、逐次 writer は `with_keep_last_on_duplicate()` を拒否する。

`expected_entry_count` が `Some(n)` なら件数超過を行入力時に、不足を `finish()` 時に検出する。
`with_noe(true)` を指定する場合は事前の件数指定が必須となる。
候補手の符号化、候補手順とコメントの出力規則は `BookDatabase` の出力と共通である。
局面に対する候補手の合法性は呼び出し側で確認する。

```rust
use std::io::BufWriter;
use rsshogi::board::hirate_position;
use rsshogi::book::{
    BookDatabaseEntry, BookEntryMetadata, BookPosition,
    YaneuraOuDb2016WriteOptions, YaneuraOuDb2016Writer,
};

let options = YaneuraOuDb2016WriteOptions::new().with_preserved_move_order();
let mut writer = YaneuraOuDb2016Writer::new(BufWriter::new(Vec::new()), options, None)?;
let entry = BookDatabaseEntry::new(
    BookPosition::from_position(&hirate_position(), Some(0)),
    Vec::new(),
    BookEntryMetadata::default(),
);
writer.write_entry(&entry)?;
let _output = writer.finish()?.into_inner()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

各エントリーの候補手の重複と符号化を出力前に検査する。
検証失敗は、そのエントリーの出力と件数を変更しない。
I/O 失敗後の writer は再利用できず、途中までの出力が残る場合がある。
`finish()` は件数を確認して flush するが、永続化や保存先の置換は呼び出し側が行う。

## 2. YBB バイナリフォーマット（`YANE-BINBOOK-V1`、`.ybb`）

YBB（YaneuraOu Binary Book）は、32 バイトの `PackedSfen` をキーとする固定長インデックス配列と、コンパクトな指し手配列で構成されるバイナリ定跡フォーマットである。
Rust の `rsshogi::book` で読み書きを提供する（Python バインディングには未公開）。

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

### `YbbBook` のオープン設定（`YbbBookOpenOptions`）

- **`with_ignore_ply(bool)`**：`false`（既定）のときは `packed_sfen` と手数 `ply` の両方が一致する行を検索する。`true`（YaneuraOu の `IgnoreBookPly` 相当）のときは手数 `ply` の違いを無視し、同一 `packed_sfen` の先頭行を返す。
- **`with_flipped(bool)`**：`false`（既定）のときは入力局面そのままの向きで検索する。`true`（YaneuraOu の `FlippedBook` 相当）のときは、通常検索でヒットしなかった場合に盤面を 180 度回転および先後反転した局面で再検索し、ヒットした候補手の座標も反転して返す（`YbbEntry::flipped()` が `true` になる）。
- **`with_storage_mode(YbbStorageMode)`**：データ保持方式を選ぶ。`Memory`（既定）はファイル全体と索引をメモリに保持する。`File` はファイルを保持し、参照した索引行と候補手だけを読み込む。
- **`with_validation_mode(YbbValidationMode)`**：検査方針を選ぶ。`Full`（既定）は open 時に全件を検査する。`OnAccess` は行の範囲と候補手の符号化を参照時に検査する。

両設定は独立しており、`storage_mode()` と `validation_mode()` で取得できる。
`Memory` と `OnAccess` の組み合わせでは全データをメモリに保持するが、内容の全件検査は open 時に行わない。
`File` と `Full` の組み合わせでは全データをメモリに展開せず、ファイルから全行を検査してから返す。
どの組み合わせでも、open 時にヘッダーと索引領域の長さを検査する。

`OnAccess` の検索には `YbbWriter` などで生成した整列済みファイルを用いる。
各行の範囲と候補手の符号化の不正や I/O 失敗は `BookError` で返す。
`validate_full()` と `iter_entries()` の全件走査では索引の逆順も検出する。
全件検査はキーの局面復元と候補手の合法性を検査しない。

`File` の reader とその clone の利用中は、参照するファイルの内容を変更しない。
clone は同じファイルハンドルを共有し、カーソルの移動と読み込みを排他制御する。

```rust
use rsshogi::book::{YbbBook, YbbBookOpenOptions, YbbStorageMode, YbbValidationMode};

let book = YbbBook::open_with_options(
    "large.ybb",
    YbbBookOpenOptions::new()
        .with_storage_mode(YbbStorageMode::File)
        .with_validation_mode(YbbValidationMode::OnAccess),
)?;
book.validate_full()?;
```

`validate_full()` はデータ保持方式と open 時の検査方針によらず全件を検査し、設定を変更しない。
`validate_full_with_control(callback)` は同じ検査を進捗通知とキャンセル付きで実行する。
callback には `YbbValidationProgress` を渡し、`processed_row_count()` と `total_row_count()` で進捗を取得できる。
開始時と各行の検査後に通知し、空の定跡でも開始時に通知する。
`BookControl::Cancel` を返すと `BookError::Cancelled` で終了し、reader は引き続き利用できる。

### 全件走査と書き出し

`YbbBook::iter_entries()` は `Iterator<Item = Result<YbbEntry, BookError>>` を返す。

ファイル参照では iterator ごとに索引と候補手領域を別々にバッファリングする。
通常は各 64 KiB を使い、大きな候補手行ではその行全体を収める領域を使う。
追加メモリは局面数に比例せず、非連続な候補手 offset と複数 iterator の交互利用にも対応する。
`validate_full()` もこの全件走査経路を使う。
索引順に全行を復号し、手数違い、重複行、候補手順を保持する。
検索オプションの `ignore_ply` と `flipped` は適用せず、すべて保存時の向き（`flipped=false`）で返す。
既定の `open` はファイル全体と索引をメモリに保持し、iterator は各行の候補手だけを順次復号する。
`YbbStorageMode::File` の場合は、索引行と候補手をファイルから順次読み込む。

`YbbWriter::new(output, entry_count, options)` は `Write + Seek` を実装した、空で先頭に位置する出力先を受け取る。
`write_entry` には `PackedSfen::cmp_bytes` の非減少順で行を渡す。
同一キーの行の順番や候補手順は変更しないため、手数を無視した検索で選ばれる先頭行も保持する。
`YbbEntry::new` と `YbbMove::new` で新規行を構築できる。
`YbbEntry::new` は各候補手の `mv()` を保存する指し手として採用し、`raw_move()` もその値に揃える。
この経路はキーをバイト列として扱い、局面の復元や合法手検査は行わない。
指し手の符号化と候補手数の範囲は検査する。

`YbbWriteOptions::new().with_depth(true)` は全候補手に `Some(depth)` を必要とする。
既定の depth なし出力には全候補手に `None` を必要とし、不一致を暗黙に補正しない。
予定行数の不一致、未整列入力、`flipped=true` の検索結果、候補手数の `u16` 超過はエラーになる。

writer は候補手を逐次出力し、44バイト/行の索引だけをメモリに保持する。
ファイルには `BufWriter<File>` を使って小さな書き込みをまとめる。
`finish` は索引とヘッダーを書き込み、出力末尾へ移動して flush し、出力先を返す。
エラー後の writer は再利用できず、途中出力が残る場合がある。
`finish` はディスクへの永続化や保存先の置換を保証しないため、必要なら呼び出し側で一時ファイルへの出力と置換を行う。

```rust
use std::fs::OpenOptions;
use std::io::BufWriter;
use rsshogi::book::{YbbBook, YbbWriteOptions, YbbWriter};

let book = YbbBook::open("source.ybb")?;
let file = OpenOptions::new().write(true).create_new(true).open("copy.ybb")?;
let options = YbbWriteOptions::new().with_depth(book.has_depth());
let mut writer = YbbWriter::new(BufWriter::new(file), book.len(), options)?;
for entry in book.iter_entries() {
    writer.write_entry(&entry?)?;
}
writer.finish()?;
```

出力は索引と候補手領域を詰め直すため、元ファイルの領域配置や余剰バイトを再現するものではない。

### `BookDatabase` からの出力

`BookDatabase::write_ybb(&mut output, &options)` は全行を検査し、キー順に出力する。
ソート用のキーと参照、および writer の索引に O(局面数) の追加メモリを使い、全候補手を変換済みデータとして保持しない。
所有 IR は正規化した局面を一意に扱うため、YBB の手数違いや重複行を保持して再出力する用途には `YbbBook::iter_entries()` と `YbbWriter` を使う。

| 入力 | 出力契約 |
| --- | --- |
| 局面 | `PackedSfen` に符号化できることを要求する。候補手はその局面で合法であることを検査する。 |
| 手数 | `BookPosition::original_ply()` を必須とし、0 を含む `u16` の範囲を保持する。既定値を補わない。 |
| 評価値 | `BookCandidate::score()` を必須とし、`i16` の範囲を要求する。 |
| depth あり | 全候補手に depth を必須とし、0 を含む `u16` の範囲を要求する。 |
| depth なし | 入力に depth があれば、破棄の明示許可を要求する。 |
| メタデータ | ponder、weight、count、形式固有メタデータは保存できない。0 の count や空の形式固有メタデータも、設定されていれば破棄の明示許可を要求する。 |

既定では情報破棄を拒否する。
`with_allow_lossy(true)` を指定すると、保存できないメタデータと、depth なし出力での入力 depth を破棄する。
必須値の欠落や保存する整数の範囲外、不正入力は、この指定でもエラーになる。
入力検査は出力開始前に行うが、I/O エラー時には未完成の出力が残り得る。
`YbbWriter` への直接入力では `with_allow_lossy` は情報破棄を行わず、depth 有無の一致を要求する。

```rust
use std::io::Cursor;
use rsshogi::board::hirate_position;
use rsshogi::book::{
    BookCandidate, BookDatabase, BookDatabaseEntry, BookEntryMetadata,
    BookMoveMetadata, BookPosition, YbbWriteOptions,
};
use rsshogi::types::Move;

let position = hirate_position();
let entry = BookDatabaseEntry::new(
    BookPosition::from_position(&position, Some(1)),
    vec![BookCandidate::new(
        Move::from_usi("7g7f").expect("valid USI"), None, Some(-25), Some(12),
        None, None, BookMoveMetadata::default(),
    )],
    BookEntryMetadata::default(),
);
let database = BookDatabase::try_from_entries(vec![entry])?;
let mut output = Cursor::new(Vec::new());
database.write_ybb(&mut output, &YbbWriteOptions::new().with_depth(true))?;
```

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

## 既存 DB2016 の差分保存

`YaneuraOuBook::write_yaneuraou_db2016_with_patch` は、元ファイルと `BookDatabasePatch` を SFEN 順にマージして出力する。
元データは正規化 SFEN の厳密な昇順を要求し、重複、逆順、不正行は、削除や置換する行も含めて拒否する。
全件 IR と出力全文を保持せず、候補手順、ponder、count、未設定値、手数の省略と明示的な 0、各コメントを保持する。
先頭局面より前のコメントも保持するが、任意の `# NOE` は省略する。
空白と改行コードのバイト一致は保証しない。
出力コメントの CRLF と単独 CR は LF に正規化し、各行に `# ` を付ける。
この規則は差分保存と `BookDatabase` からの出力、逐次 writer で共通とする。
SBK metadata と weight を含む差分は `Unsupported` とする。

```rust,ignore
use rsshogi::board::InitialPosition;
use rsshogi::book::{BookDatabasePatch, YaneuraOuBook};
use std::{fs::File, io::BufWriter};

let source = YaneuraOuBook::open("input.db")?;
let mut patch = BookDatabasePatch::new();
patch.remove_entry_by_sfen(InitialPosition::Standard.to_sfen())?;
let mut output = BufWriter::new(File::create("staged.db")?);
source.write_yaneuraou_db2016_with_patch(&mut output, &patch)?;
```

`write_yaneuraou_db2016_with_patch_with_control` は `BookPatchWriteProgress` と `BookControl` で進捗通知と協調キャンセルを行う。
処理件数は元局面数で、追加局面数や完了率ではない。
DB2016 の総数は `None` となる。
部分出力、永続化、入力の寿命の契約は[定跡アーキテクチャ](book-architecture.md)を参照する。

## 関連項目

- [定跡 3 層アーキテクチャ](book-architecture.md)
- [NativeBook バイナリフォーマット仕様](book.md)
- [ShogiHome SBK フォーマット仕様](sbk.md)
