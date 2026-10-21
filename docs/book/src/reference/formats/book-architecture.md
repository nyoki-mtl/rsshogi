# 定跡アーキテクチャ

定跡は Native Book v4、外部形式の reader、変換用の所有 IR に分ける。
評価値による採択や抽選は利用側のエンジンが行う。

| 用途 | 型 | 契約 |
| --- | --- | --- |
| 部分参照 | `NativeBook` | open 時点の snapshot。検索は所有 entry と `Result` を返す |
| 編集と保存 | `NativeBookEditor` | 1 writer の排他、局面 snapshot の追記、明示的な compaction |
| 初期構築 | `NativeBookBuilder` | 任意順の順次入力、メモリ予算付き外部 sort、新規出力 |
| 外部形式の参照 | `YaneuraOuBook` / `YbbBook` / `SbkBook` | 各形式の値と意味を保持する concrete reader |
| 変換用 IR | `BookDatabase` | SFEN と形式固有情報を持つ所有データ。全件ロードは明示的に行う |

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

Rust の `BookDatabase`、`YaneuraOuBook`、`SbkBook` は `write_native_book(path, NativeBookImportOptions)` を持つ。外部 reader からは所有 IR 全件への展開を経ず、順次変換して builder に渡す。

既定の `allow_lossy = false` は保存できない情報があれば新規出力を公開せず失敗する。
`allow_lossy = true` を明示すると、`NativeBookImportReport` に出力件数、除外局面数、フィールド別の欠落件数、入力位置と SFEN 付きの先頭 32 件の例、省略例数を返す。
候補手の depth は `u32` の全域を保持する。
入力に回数があれば、0 を含め `candidate.count` の欠落として数え、strict 変換を失敗させる。
lossy 変換では回数を捨てて報告し、別のフィールドへ代入しない。
局面が PackedSfen で表現不能なら、既定はエラー、明示的な lossy 変換では報告して除外する。

DB2016 の候補手評価は入力で宣言された cp として扱い、大きな整数を詰みと推測しない。
SBK の手の評価は注釈値であるため cp に転用せず、状態評価や PV を候補手へ複製しない。
ponder、元の手数、形式固有 metadata など v4 に保存できない値は欠落報告の対象になる。

棋譜から構築する場合は局面を再生し、`NativeBookEntry::from_position` で候補手を作って `NativeBookBuilder::push` に渡す。同じ局面の集計や候補手の重複解消は収集側で明示する。builder は重複局面を拒否する。子局面の評価を使う場合は、収集側で手番視点、詰み距離、bound を変換する。

## 1.5.0 への移行

旧 `Book` trait、`BookKey`、`BookMove`、`BookEntry`、`MemoryBook`、`StaticBook`、`BookBuilder`、`NativeBookConvertOptions`、`NativeScorePolicy`、`to_memory_book`、`to_static_book` は削除した。
ハッシュキー API の互換層は設けない。

wire version 4 の reader は旧 v2 と固定長 v3 を受理しない。
旧 v2 は局面情報を失っているため、ファイル単独から v4 へ変換できない。
固定長 v3 も互換 reader を設けず、元の棋譜または外部定跡から再生成する。
外部 reader と `BookDatabase` の DB2016 / SBK 書出しは引き続き利用できる。
