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

`open()` は局面行のうち先頭の一定範囲だけを検証します。診断が `complete == false` を
報告する場合、`validate_full()` がファイル全体のソートを確認するまで `lookup_sfen()`
はエラーを返します。ソート検証がファイルの非ソートを報告した場合も、`lookup_sfen()`
は（数 GB のファイルを暗黙に全走査しないよう）エラーを返します。低速な全件スキャンを
意図する場合のみ `lookup_sfen_by_scan()` を使ってください。

`.db` リーダはファイルサイズに比例したメモリ確保を行いません。

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

`SafeBinary` が既定で、全検証が完了するまで二分探索をブロックし続けます。
`ValidateFullBeforeLookup` は open 時にファイル全体を検証します。
`AssumeSortedAfterPrefix` と `AssumeSortedByCaller` は全検証の前に二分探索を許可しますが、
そのリスク（非ソートのファイルに対する二分探索はエントリーを取りこぼし得る）を表面化
させる責任は呼び出し側にあります。`AssumeSortedByCaller` は順序チェックを行わないため、
`YaneuraOuBookDiagnostics::Unvalidated` を報告します。`ScanOnly` は `lookup_sfen()` を
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

DB2016 のテキストは現在 UTF-8 としてデコードされます。日本語の旧来ファイルで
パースに失敗する場合は、行の構文が不正だと判断する前に、元データが CP932 / Shift_JIS で
ないか確認してください。

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

YBB リーダはファイル全体をメモリへ展開せず、header と index 領域サイズだけを open 時に
検証します。検索時は `PackedSfen[32]` の byte order で index を二分探索し、ヒットした
局面の moves 領域だけを読み込みます。

候補手は `.ybb` の moves 領域に格納された順序で返します。参照実装の probe 時の
候補手ソート（`.ybb` では実質 eval 降順）までは reader 内で再現しません。必要な順序で
使う場合は呼び出し側で明示的に並べ替えてください。

対応する範囲:

- magic `YANE-BINBOOK-V1\0`;
- `flags bit0 = move depth あり`;
- `Move16` / eval `i16` / optional depth `u16`;
- `IgnoreBookPly` 相当の ply mismatch 無視;
- `FlippedBook` 相当の先後反転 lookup。

`.ybb` は count / comment / ponder を持たないため、`YaneuraOuBookEntry` とは別の
`YbbEntry` / `YbbMove` として公開します。`.db` 指定時に同 basename の `.ybb` へ暗黙
fallback する 互換挙動は提供しません。必要な場合は呼び出し側で明示的に
ファイル名を選んでください。

Python binding にはまだ公開していません。

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

SBK リーダはデコード済みの state グラフ全体を保持せず、ファイル全体をメモリに読み込む
こともしません。コンパクトな state オフセット表と Packed SFEN インデックスは state 数に
比例します。個々の state payload は、インデックス構築時や一致エントリーの検索時に読み込み・
デコードされます。

`SbkBook::iter_entries()` はインデックス済みの state を反復し、各 state をオンデマンドで
デコードします。

SBK 固有のツリー探索では、呼び出し側がデコード済みの state id を直接解決できます。

```rust,ignore
let parent = book.lookup_sfen(sfen)?.expect("parent entry");
if let Some(child) = book.child_entry(&parent, 0)? {
    println!("{}", child.sfen());
}
```

`SbkBook::lookup_state_id()` はデコード済みの SBK `id` メタデータを使い、id が物理 state
インデックスと等しいとは仮定しません。`SbkBook::lookup_state_index()` は物理 state payload
インデックスを指します。負または不明な id・範囲外インデックス・`nextStateId` を持たない手・
範囲外の手インデックスは、いずれも `Ok(None)` を返します。

SBK の full export には `BookDatabase::write_sbk()` / `to_sbk_bytes()` を使います。

```rust,ignore
use rsshogi::book::{BookDatabase, SbkWriteOptions};

let bytes = database.to_sbk_bytes(&SbkWriteOptions::new())?;
```

writer は `BookDatabase` 全体を新しい SBK protobuf として再生成します。state ID は
root を先に並べたうえで `BookStates[i].Id == i` の連番にし、候補手の合法遷移から
`NextStateId` を再構築します。
子局面が `BookDatabase` に存在しない候補手は `NextStateId = -1` として出力し、空の leaf
state は合成しません。ShogiHome と同じく `BoardKey` / `HandKey` は 0 出力を許容します。
top-level の author / description は現行 `BookDatabase` に格納先がないため出力しません。

on-the-fly SBK の差分 patch merge writer と Python binding 露出はまだ提供していません。

## ローカルでの大容量ファイル検証

大容量の参照ファイルは通常のテストには含まれません。ローカルに検証用ファイルがある場合は
環境変数でパスを明示してから ignored test を実行します。

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
