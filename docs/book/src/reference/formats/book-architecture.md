# 定跡アーキテクチャ

定跡は Native Book v4、外部形式の reader、変換用の所有 IR に分ける。
評価値による採択や抽選は利用側のエンジンが行う。

| 用途 | 型 | 契約 |
| --- | --- | --- |
| 部分参照 | `NativeBook` | open 時点の snapshot。検索は所有 entry と `Result` を返す |
| 編集と保存 | `NativeBookEditor` | 1 writer の排他、局面 snapshot の追記、明示的な compaction |
| 初期構築 | `NativeBookBuilder` | 任意順の順次入力、メモリ予算付き外部 sort、新規出力 |
| 外部形式の参照 | `YaneuraOuBook` / `YbbBook` / `SbkBook` | 各形式の値と意味を保持する concrete reader |
| YBB の出力 | `YbbWriter` / `BookDatabase::write_ybb` | 整列済み行の逐次出力、または所有 IR の検査とソート。情報破棄は明示指定 |
| 変換用 IR | `BookDatabase` | SFEN と形式固有情報を持つ所有データ。全件ロードは明示的に行う |
| 外部形式の差分保存 | `BookDatabasePatch` と各 reader の `write_*_with_patch` | 変更局面だけを所有し、DB2016 または SBK 全体を逐次出力する |

DB2016 と SBK の差分保存は、元 reader の上に局面の置換と削除を重ねる。
この差分保存 API は 1.7.1 で追加する Rust API であり、Python bindings には公開しない。
差分は手数を除いた正規化 SFEN で識別し、同じ局面への最後の操作を保持する。
`upsert_entry` は候補手とメタデータを含む局面全体を置換するため、値の一部だけを変更する場合も、保持する内容を揃えて渡す。
検索時は `BookDatabasePatch::edit_by_sfen` を先に確認し、`Upsert` を返し、`Delete` なら未登録とし、差分がなければ reader を検索する。
差分保存は全文書の書き直しであり、Native Book の transaction 追記とは保存量が異なる。
DB2016 のメモリは差分と最大局面行に、SBK は差分と局面索引と最大 state に比例する。
DB2016 は元局面の正規化 SFEN が厳密な昇順であることを要求する。
SBK は保存前に全辺を検査し、既存 reader が参照用に受理する未解決 state や不整合を持つファイルでも、保存を拒否する場合がある。
形式別の保持項目と拒否条件は [DB2016](yaneuraou.md#既存-db2016-の差分保存) と [SBK](sbk.md#5-既存-sbk-の差分保存) を参照。
出力先には元ファイルと別の保存領域を渡し、処理中に元ファイルを変更してはいけない。
成功時は flush するが、永続化とファイル置換は呼び出し側が行う。
エラーやキャンセルでは部分出力が残り得るため、公開せず破棄する。
差分は成功時も失敗時も変更されず、新しい出力先へ再試行できる。
保存出力を元のパスへ置換した後は reader を開き直す。
新しい reader への切り替え後は空の差分集合を使い、古い索引と保存済み差分を引き継がない。

## Native Book v4

局面は盤面、持ち駒、手番を含む 32B の正規形 `PackedSfen` で識別する。手数と履歴は含まず、復元時の手数は 1。キーは byte 辞書順で比較し、Zobrist の `hash-128` 設定には依存しない。

ファイル形式は version 4（magic `RSSHGBK4`）である。
ライブラリの版番号とファイル形式の版番号は独立している。

本体は 128B ヘッダー、56B 単位のブロック索引、圧縮ブロック、コメント索引、本文、文書メタデータを持つ。
writer は指定した局面数ごとにキーの prefix と可変長整数を符号化し、独立した Zstandard frame に圧縮する。
新規構築の既定と推奨値は 64 局面で、入力前に `NativeBookBuilder::with_block_positions(256)` のように 1..=16384 を指定できる。
保存された値は `NativeBook::block_positions()` で取得し、編集後の compaction、別名保存と救出でも維持する。
reader は 1..=16384 局面のブロックを受理し、open 時に索引を RAM に読み、検索時に必要なブロックだけを展開する。
展開済み 1 ブロックを cache し、検索はコメント本文を読まない。
通常保存はヘッダーや本体を書き換えず、変更局面と文書メタデータを 1 transaction として追記する。
追記内の候補手は固定 16B であり、圧縮本体と同じ評価値、深さと表示順位を保持する。
reader は capture した長さの snapshot を保持し、再 open するまで後の保存を反映しない。

候補手は `mv`、`score`、任意の `depth: Option<u32>`、`display_order` を持ち、回数を保存しない。
depth の未設定と 0 は区別する。
`NativeBookMetadata.depth_kind: u32` は文書全体の深さの意味を示し、0 は未指定、1 は探索深さ、2 は定跡内の後退解析距離である。
未知の値も保存、再読込と compaction で保持する。
エンジンは `depth_kind == 1` を確認した場合だけ探索深さの条件へ使う。
メタデータの追記はヘッダーの depth_kind より優先し、compaction は最新値を新本体へ反映する。

```rust
extern crate rsshogi;
use rsshogi::board::Position;
use rsshogi::book::{
    NativeBook, NativeBookEditor, NativeBookEntry, NativeBookMetadata,
    NativeBookPosition, NativeCandidate,
};
use rsshogi::types::Move;

let position = Position::from_sfen(
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
)?;
let candidate = NativeCandidate::new(Move::from_usi("7g7f").expect("valid USI"));
let entry = NativeBookEntry::from_position(&position, vec![candidate])?;
let mut editor = NativeBookEditor::create("new.book", NativeBookMetadata::default())?;
editor.upsert_position(NativeBookPosition::new(entry))?;
editor.save()?;
drop(editor);
let book = NativeBook::open("new.book")?;
assert!(book.get(&position)?.is_some());
# Ok::<(), Box<dyn std::error::Error>>(())
```

`NativeBook::entries()` と `records()` はキー順の streaming iterator。前者は検索データのみ、後者はコメント込みの snapshot を返す。`NativeBookEditor::records()` は未保存の編集も含む。`write_to` は未保存の変更を含む snapshot を新規出力し、`write_to_with_replace` は明示的な置換にも対応する。保存先の writer ロックも取得する。

完全検証は `validate_full` で行う。ログの不正箇所を飛び越えず、既に上書きされた古い snapshot の本文も検査する。`recover_tail` は正常な開始ヘッダーがある未完了 tail だけを保全付きで復旧し、`salvage_prefix` は原本を保持して検証済み prefix を別ファイルへ再構築する。詳細は [バイナリ仕様](book.md) を参照。

コメントと文書メタデータは任意である。エンジンの候補手検索`get`はコメント本文を読まず、採択にも使わない。
ただしopen時は、追記ログのCRCと内容を検証するため、ログ内のコメントも読む。
本体のコメント量は通常の候補手検索の読込量に加わらないが、ログ内のコメント量は起動時の検証量に加わる。
全件検証、保存、compactionにもコメント量に応じたコストがある。

## 外部形式からの変換

Rust の `BookDatabase`、`YaneuraOuBook`、`YbbBook`、`SbkBook` は `write_native_book(path, &NativeBookImportOptions)` を持つ。
外部 reader からは所有 IR 全件への展開を経ず、順次変換して builder に渡す。

```rust,no_run
use rsshogi::book::{DEPTH_KIND_SEARCH, NativeBookImportOptions, YaneuraOuBook};

let source = YaneuraOuBook::open("source.db")?;
let options = NativeBookImportOptions::new()
    .with_depth_kind(DEPTH_KIND_SEARCH)
    .with_allow_lossy(true)
    .with_memory_budget(64 * 1024 * 1024)
    .with_block_positions(64);
let report = source.write_native_book("output.book", &options)?;
# Ok::<(), rsshogi::book::BookError>(())
```

`with_metadata` は題名、説明、深さ種別をまとめて置き換え、`with_depth_kind` は深さ種別だけを更新する。
併用した場合は呼び出し順に適用する。
全形式で深さ種別の既定値は `DEPTH_KIND_UNSPECIFIED`（0）とし、形式や深さ値から由来を推測しない。
SBK では説明が `None` のときだけ元の description を引き継ぐ。
builder の予算は既定 256 MiB、ブロック局面数は既定 64 とする。
予算は 1 MiB 以上、ブロック局面数は `1..=16384` を入力走査前に検査する。
reader の保持領域や単一入力行のメモリは builder の予算に含めない。

既定の `allow_lossy = false` は保存できない情報があれば新規出力を公開せず失敗する。
`allow_lossy = true` を明示すると、`NativeBookImportReport` に出力件数、除外局面数、フィールド別の欠落件数、入力位置と SFEN 付きの先頭 32 件の例、省略例数を返す。
`dropped_fields` の既存キー名と件数の意味は維持する。
項目名の集合は拡張可能なため、consumer は未知のキーも表示または保持して扱う。
診断例の `reason` とエラーメッセージは表示用であり、機械的な分岐条件にしない。
候補手の depth は `u32` の全域を保持する。
入力に回数があれば、0 を含め `candidate.count` の欠落として数え、strict 変換を失敗させる。
lossy 変換では回数を捨てて報告し、別のフィールドへ代入しない。
SFEN を解析できても PackedSfen で表現不能な局面は、`with_skip_unencodable_positions(true)` を指定した場合だけ除外する。
候補手の合法性、一意性、個数を検査してから除外するため、非合法手や重複候補手はスキップ指定でもエラーになる。
スキップ行は `position` の欠落 1 件だけを記録し、その行の手数や回数を重ねて数えない。
このスキップ自体には `allow_lossy` を必要としない。
構文不正の SFEN や、復号不能または非正準の YBB キーは常に拒否する。

SBK reader の重複局面診断は、オプションによらず変換開始時に拒否する。
未解決状態は `allow_lossy = true` の場合だけ `sbk.unresolved_state` と除外局面数へ加算する。
SBK の候補手検査は reader が返す解決済み行が対象であり、未解決状態や reader が省略する要素まで元ファイル全体の検査済みとはしない。

DB2016 の候補手評価は入力で宣言された cp として扱い、大きな整数を詰みと推測しない。
SBK の手の評価は注釈値であるため cp に転用せず、状態評価や PV を候補手へ複製しない。
ponder、元の手数、形式固有 metadata など v4 に保存できない値は欠落報告の対象になる。
DB2016 の明示手数は `position.original_ply` を 1 件とし、省略時は数えない。
所有 IR の `yaneuraou.min_ply` は元手数（省略時は 0）と異なる場合だけ追加計上する。
YBB の手数は各行の `ybb.ply` として数える。

既存保存先や、開始後に別の処理が作成した保存先は上書きしない。
公開前の失敗では自分の出力を公開せず、一時ファイルの削除を試みる。
削除時の I/O 障害や異常終了でも一時ファイルが残らないことまでは保証しない。
完成したファイルの公開後に同期などが失敗した場合は `BookError::SaveUncertain` を返し、出力が存在し得る。
呼び出し側は無条件再実行や自動削除を避け、再オープンと `validate_full()` などで内容を確認する。
内容の検証成功と永続化成功の保証は区別する。

棋譜から構築する場合は局面を再生し、`NativeBookEntry::from_position` で候補手を作って `NativeBookBuilder::push` に渡す。同じ局面の集計や候補手の重複解消は収集側で明示する。builder は重複局面を拒否する。子局面の評価を使う場合は、収集側で手番視点、詰み距離、bound を変換する。

## 1.5.0 への移行

旧 `Book` trait、`BookKey`、`BookMove`、`BookEntry`、`MemoryBook`、`StaticBook`、`BookBuilder`、`NativeBookConvertOptions`、`NativeScorePolicy`、`to_memory_book`、`to_static_book` は削除した。
ハッシュキー API の互換層は設けない。

wire version 4 の reader は旧 v2 と固定長 v3 を受理しない。
旧 v2 は局面情報を失っているため、ファイル単独から v4 へ変換できない。
固定長 v3 も互換 reader を設けず、元の棋譜または外部定跡から再生成する。
外部 reader と `BookDatabase` の DB2016 / SBK 書出しは引き続き利用できる。
