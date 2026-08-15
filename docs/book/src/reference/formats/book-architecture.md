# 定跡アーキテクチャ（3 層モデル）

rsshogi の定跡サポートは、参照、保存、編集という用途に合わせて 3 層に分かれています。
`StaticBook` と `MemoryBook` は高速な参照を担い、`YaneuraOuBook`、`SbkBook`、
`YbbBook` は各ファイル形式の情報を保持します。
`BookDatabase` は形式間の編集と変換を受け持ちます。

## 用途ごとの3層

定跡の扱い方は、次の三つに分けられます。

- **参照**：局面を `BookKey`（既定は 64 bit、`hash-128` feature では 128 bit）へ正規化し、
  探索で使う候補手をメモリ上の表から取得します。`MemoryBook` と `StaticBook` が担当します。
- **保存**：評価、出現回数、コメント、ponder など、形式固有の情報を保持します。
  `YaneuraOuBook`、`SbkBook`、`YbbBook` が担当します。
- **編集**：SFEN、Packed SFEN、元の手数、ファイル上の同一性を保ちながら形式を変換します。
  `BookDatabase` が担当します。

各型の役割を分けることで、参照速度と形式固有情報の両方を扱えます。

```text
┌──────────────────────────────────────────────────────────────┐
│ 外部リーダ層   YaneuraOuBook / SbkBook / YbbBook              │
│   読み取り専用・フォーマット忠実(lossless)                      │
│   キー = SFEN / Packed SFEN（フォーマット固有の同一性）        │
└───────────────────────────┬──────────────────────────────────┘
                            │ from_yaneuraou() / from_sbk()
                            │ write_yaneuraou_db2016() / write_sbk()
                            ▼
┌──────────────────────────────────────────────────────────────┐
│ 編集 IR 層     BookDatabase                                   │
│   SFEN / Packed SFEN / 元 ply / 由来を保持し往復変換のハブ      │
│   BookKey は派生ヘルパ扱い（IR の主キーではない）              │
└───────────────────────────┬──────────────────────────────────┘
                            │ to_memory_book() / to_static_book()
                            ▼
┌──────────────────────────────────────────────────────────────┐
│ ルックアップ層 MemoryBook / StaticBook（Book トレイト）       │
│   Zobrist キー・正規化済み・lossy・高速参照                    │
│   キー = BookKey（盤面 + 持駒 + 手番の Zobrist key）           │
└──────────────────────────────────────────────────────────────┘
```

## 各層の役割

### ルックアップ層（`MemoryBook` / `StaticBook`）

`Book` トレイトを実装する、**正規化済みの参照表現**です。

- キーは [`book_key_from_position()`](book.md) が返す `BookKey`
  （盤面 + 持駒 + 手番の Zobrist key）。定跡における「同じ局面」の定義に忠実です。
- 1 手分のデータは `BookMove { mv, score: i16, depth: u16 }` のみ。
  参照に必要な候補手、評価値、深さに絞った lossy な表現です。
- `MemoryBook` は `HashMap<BookKey, Vec<BookMove>>` のメモリ常駐表。
  `StaticBook` はそれをソート済みバイナリへ焼き込み、実行時コストゼロで参照します。
- `Book::get()` は既にメモリ上にある `&[BookMove]` を借用して返す契約です。

「探索エンジンが現局面の候補手を引く」用途では、この層がほぼ理想形です。

### 外部リーダ層（`YaneuraOuBook` / `SbkBook` / `YbbBook`）

外部フォーマットを形式固有の型で参照するリーダです。

- キーが Zobrist ではなく SFEN 文字列（DB2016）/ Packed SFEN（SBK / YBB）。
  ファイルに記録された局面の同一性を保ったまま検索できます。
- フォーマット固有のメタ情報を **lossless** で公開します。これを `MemoryBook` に
  直接変換すると `ponder` / `move_count` などが無音で捨てられるため、固有型のまま
  提供します。
- 読み込み方法は形式ごとに異なります。DB2016 は必要なエントリを順次読み取り、
  YBB は `open()` 時にファイル全体をメモリへ読み込んで固定長レコードを検証します。

詳細は [外部定跡（DB2016 / YBB / SBK）](external-books.md) と
[SBK 形式](sbk.md) を参照してください。

### 編集 IR 層（`BookDatabase`）

外部フォーマットとルックアップ層を **橋渡しする中間表現**です。

- `BookDatabase` は SFEN、Packed SFEN、元 row の手数、ファイル由来の同一性を
  position data として保持します。`BookKey` は参照表を作るときに導出します。
- `BookDatabaseEntry::from_yaneuraou()` / `from_sbk()` で外部エントリを取り込み、
  `to_memory_book()` / `to_static_book()` で lossy なルックアップ表へ射影します。
- `write_yaneuraou_db2016()` / `write_sbk()` で編集済みデータベースを外部形式へ
  書き戻せます。`write_sbk()` は `BookDatabase` が保持する局面と候補手を出力します。

## 参照層と保存層の選び方

探索中の参照には `MemoryBook` と `StaticBook` が適しています。
形式固有の情報を扱う処理には外部リーダか `BookDatabase` を選びます。

- `StaticBook` は配布用のソート済みバイナリ、`MemoryBook` は実行中に構築する参照表に向きます。
- 両者は `BookKey` と候補手に情報を絞るため、元の SFEN や形式固有のメタデータが必要な処理には
  `BookDatabase` を使います。
- 大規模なファイルは、DB2016 や SBK の外部リーダを使うと必要なエントリを順次参照できます。

参照層は `Book` トレイトを共通の入口とし、保存層と編集層は形式固有の情報を明示的に扱います。

## 関連項目

- [定跡バイナリ](book.md)：`StaticBook` のバイナリ形式仕様
- [外部定跡（DB2016 / YBB / SBK）](external-books.md)：外部リーダと writer の Rust API
- [DB2016 形式](yaneuraou.md)：DB2016 フォーマットの詳細
- [SBK 形式](sbk.md)：SBK フォーマットの詳細
- [定跡 (Book)（Python）](../../python/book.md)：Python からの利用
