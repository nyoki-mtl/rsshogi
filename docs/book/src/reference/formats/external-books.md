# 外部定跡

rsshogi は、外部定跡フォーマット向けに **読み取り専用リーダ**と一部フォーマットへの
writer を明示的に提供します。これらの API は `StaticBook::from_file()`
（rsshogi 独自のバイナリ定跡フォーマット）とは別系統です。

> Python から使う場合は [外部定跡（DB2016 / SBK）](../../python/external-books.md)
> を参照してください。本ページは Rust API を扱います。
>
> 外部リーダがなぜ `StaticBook` / `MemoryBook` と別系統なのかは
> [定跡アーキテクチャ（3 層モデル）](book-architecture.md) を参照してください。

## Packed SFEN

`Position::to_packed_sfen()` は、cshogi 互換ツールが使う 32 バイトの Packed SFEN キーを
返します。`PackedSfen` は次を公開します。

- `as_bytes()`: 32 バイト表現を取得する;
- `cmp_bytes()`: 32 バイト表現そのものの辞書順で比較する（YBB 用）;
- `to_le_u32_words()` / `from_le_u32_words()`: SBK / BookConv のインデックス用;
- `cmp_sbk_words()`: SBK が使うワード順での比較。

YBB の byte order と SBK のワード順は同一ではないため、用途ごとに明示的に扱います。

## DB2016

> フォーマットそのものの構造（ヘッダ・sfen 行・指し手行の書式・各フィールドの意味・
> ソート順・設計思想）は [DB2016 形式](yaneuraou.md) を参照してください。
> 本節は Rust API の使い方です。

読み取り専用の `.db` 検索には `YaneuraOuBook` を使います。

```rust,ignore
use rsshogi::book::{YaneuraOuBook, YaneuraOuBookDiagnostics};

let mut book = YaneuraOuBook::open("book.db")?;
if matches!(
    book.diagnostics(),
    YaneuraOuBookDiagnostics::Sorted { complete: false, .. }
) {
    book.validate_full()?;
}
let entry = book.lookup_sfen(sfen)?;
```

このリーダは次に対応します。

- 任意の `#YANEURAOU-DB2016 1.00` ヘッダ;
- UTF-8 BOM・CRLF・LF;
- `#` および `//` のコメント行;
- 指し手のメタデータ: move, ponder, score, depth, count, comment;
- 局面行がソート済みのときの二分探索。

SFEN 検索ではキーの同一性のために行の手数を `1` に正規化し、元の行の手数は `min_ply`
として保持します。

`open()` は局面行の先頭部分を検証します。
診断が `complete == false` の場合は、`validate_full()` でファイル全体のソートを確認すると
`lookup_sfen()` の二分探索を利用できます。
ソート前のファイルを明示的に全件走査する場合は `lookup_sfen_by_scan()` を使います。

`.db` リーダはファイルをストリーミングし、ファイルサイズに依存しない一定量のメモリで検索します。

明示的なストリーミング取り込みを意図する場合は `iter_entries()` を使います。

```rust,ignore
for entry in book.iter_entries()? {
    let entry = entry?;
    // エントリーの取り込みや検査
}
```

open 時のトレードオフを変えたい GUI 呼び出し側は `YaneuraOuBook::open_with_options()`
を使えます。

```rust,ignore
use rsshogi::book::{YaneuraOuAccessMode, YaneuraOuBook, YaneuraOuBookOpenOptions};

let book = YaneuraOuBook::open_with_options(
    "book.db",
    YaneuraOuBookOpenOptions::with_access_mode(
        YaneuraOuAccessMode::AssumeSortedAfterPrefix { prefix_rows: 10_000 },
    ),
)?;
```

`SafeBinary` が既定で、全検証の完了後に二分探索を開始します。
`ValidateFullBeforeLookup` は open 時にファイル全体を検証します。
`AssumeSortedAfterPrefix` は先頭部分の検証後、`AssumeSortedByCaller` は呼び出し側の
ソート保証を使って二分探索を開始します。
`AssumeSortedByCaller` の診断結果は `YaneuraOuBookDiagnostics::Unvalidated` です。
`ScanOnly` は `lookup_sfen()` を
明示的なスキャン経路にしつつ、診断と早期のフォーマットエラー検出のために先頭の一定範囲は
検証します。

進捗とキャンセルを伴うバックグラウンド検証には `validate_full_with_control()` を使います。

```rust,ignore
use rsshogi::book::BookControl;

book.validate_full_with_control(|progress| {
    eprintln!(
        "{} / {} bytes, {} rows",
        progress.processed_bytes(),
        progress.total_bytes(),
        progress.processed_rows()
    );
    BookControl::Continue
})?;
```

キャンセルは協調的です。リーダが進捗コールバックを呼び出したときにのみ観測されます。

DB2016 のテキストは UTF-8 としてデコードされます。
CP932 または Shift_JIS のファイルは UTF-8 へ変換してから開きます。

## YBB

読み取り専用の `.ybb` 検索には `YbbBook` を使います。

```rust,ignore
use rsshogi::book::{YbbBook, YbbBookOpenOptions};

let book = YbbBook::open_with_options(
    "book.ybb",
    YbbBookOpenOptions::new().with_ignore_ply(true).with_flipped(true),
)?;
let entry = book.lookup_position(&position)?;
```

YBB リーダは open 時にファイル全体を読み込み、header、index、すべての move record を検証します。
検索時は `PackedSfen[32]` の byte order でメモリ上の index を二分探索し、ヒットした局面の moves を復号します。

候補手は `.ybb` の moves 領域に格納された順序で返します。
評価値順で使う場合は、取得後に呼び出し側で並べ替えます。

対応する範囲:

- magic `YANE-BINBOOK-V1\0`;
- `flags bit0 = move depth あり`;
- `Move16` / eval `i16` / optional depth `u16`;
- `IgnoreBookPly` 相当の ply mismatch 無視;
- `FlippedBook` 相当の先後反転 lookup。

`.ybb` は count / comment / ponder を持たないため、`YaneuraOuBookEntry` とは別の
`YbbEntry` / `YbbMove` として公開します。
`.db` と `.ybb` はそれぞれ対応する reader へ明示的なファイル名を渡します。
YBB は Rust API から利用します。

## SBK

> フォーマットそのものの構造（protobuf スキーマ・state グラフ・局面エンコード・
> 他形式との違い）は [SBK 形式](sbk.md) を参照してください。本節は Rust API の使い方です。

読み取り専用の `.sbk` 検索には `SbkBook` を使います。

```rust,ignore
use rsshogi::book::SbkBook;

let book = SbkBook::open("book.sbk")?;
let entry = book.lookup_sfen(sfen)?;
```

SBK リーダは protobuf の state オフセットを走査し、各 state を潜在的なルートとして
たどり、明示的な局面または `nextStateId` リンクの追跡から Packed SFEN キーを導出して、
コンパクトなソート済みインデックスに格納します。不正なグラフ辺はスキップされるため、
壊れた分岐があっても定跡の残りを開く妨げにはなりません。検索ではインデックスを二分探索し、
一致した state payload だけをデコードします。

保持されるメタデータ:

- トップレベルの author と description;
- state id, games, wonBlack, wonWhite, comment, eval レコード;
- move word, 変換後の move, evaluation, weight, nextStateId。

`SbkBook::open_with_control()` はインデックス構築中の進捗を報告し、コールバックで open を
キャンセルできます。

```rust,ignore
use rsshogi::book::{BookControl, SbkBook};

let book = SbkBook::open_with_control("book.sbk", |progress| {
    eprintln!("{} / {}", progress.indexed_states(), progress.total_states());
    BookControl::Continue
})?;
```

`SbkBook::diagnostics()` は、重複した Packed SFEN 局面と未解決の state payload を
報告します。

SBK リーダはコンパクトな state オフセット表と Packed SFEN インデックスを保持します。
これらのサイズは state 数に比例し、個々の state payload はインデックス構築時や
一致エントリーの検索時に読み込んでデコードします。

`SbkBook::iter_entries()` はインデックス済みの state を反復し、各 state をオンデマンドで
デコードします。

SBK 固有のツリー探索では、呼び出し側がデコード済みの state id を直接解決できます。

```rust,ignore
let parent = book.lookup_sfen(sfen)?.expect("parent entry");
if let Some(child) = book.child_entry(&parent, 0)? {
    println!("{}", child.sfen());
}
```

`SbkBook::lookup_state_id()` はデコード済みの SBK `id` メタデータ、
`SbkBook::lookup_state_index()` は物理 state payload インデックスを使います。
解決できるエントリがある場合は `Some`、負または不明な id、範囲外インデックス、
終端の手には `None` を返します。

SBK の full export には `BookDatabase::write_sbk()` / `to_sbk_bytes()` を使います。

```rust,ignore
use rsshogi::book::{BookDatabase, SbkWriteOptions};

let bytes = database.to_sbk_bytes(&SbkWriteOptions::new())?;
```

writer は `BookDatabase` 全体を新しい SBK protobuf として再生成します。state ID は
root を先に並べたうえで `BookStates[i].Id == i` の連番にし、候補手の合法遷移から
`NextStateId` を再構築します。
候補手に対応する子局面がある場合は `NextStateId` を付け、終端は `-1` で表します。
ShogiHome と同じく `BoardKey` / `HandKey` は 0 出力を許容します。
writer は `BookDatabase` が保持する局面と候補手を full export します。

Python では `rsshogi.book.SbkBook` から SBK を参照できます。

## ローカルでの大容量ファイル検証

大容量ファイルの検証には、環境変数でローカルファイルのパスを指定して ignored test を実行します。

```bash
RSSHOGI_LARGE_YANEURAOU_DB=/path/to/book.db cargo test -p rsshogi test_yaneuraou_db_large_local_smoke -- --ignored --nocapture
RSSHOGI_LARGE_SBK_FIXTURE=/path/to/book.sbk cargo test -p rsshogi test_sbk_large_local_smoke -- --ignored --nocapture
```

小さい ShogiHome の SBK フィクスチャは次で確認できます。

```bash
RSSHOGI_SBK_FIXTURE_DIR=/path/to/sbk-fixtures cargo test -p rsshogi test_sbk_shogihome_fixtures_smoke -- --ignored --nocapture
```

## 関連項目

- [定跡バイナリ](book.md) - rsshogi 独自のバイナリ定跡フォーマット仕様
- [棋譜フォーマット一覧](index.md) - 棋譜・学習・定跡フォーマットの一覧
- [外部定跡（Python）](../../python/external-books.md) - Python からの利用
