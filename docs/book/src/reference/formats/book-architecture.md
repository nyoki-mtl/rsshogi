# 定跡アーキテクチャ（3 層モデル）

rsshogi の定跡サポートは、ひとつの「理想の定跡型」に収束させるのではなく、
**用途の異なる 3 つの層**に分けて設計されています。なぜ `StaticBook` / `MemoryBook`
と、`YaneuraOuBook` / `SbkBook` が別系統なのか、その理由をまとめます。

## なぜ 1 つに統一しないのか

定跡には「速く引きたい」「忠実に保存したい」「編集・変換したい」という、
互いに衝突する要求があります。これらを 1 つの型で満たすことはできません。

- **速く引く**には、局面を `BookKey`（default 64bit / `hash-128` feature で 128bit）へ正規化し、メタ情報を削ぎ落として
  メモリ上の表に並べるのが理想です。← `MemoryBook` / `StaticBook`
- **忠実に保存する**には、各フォーマット固有の情報（評価・出現回数・コメント・ponder
  など）を欠落なく保持する必要があります。← `YaneuraOuBook` / `SbkBook` / `YbbBook`
- **編集・変換する**には、SFEN・Packed SFEN・元 row の手数・ファイル由来の同一性を
  すべて復元できる中間表現が必要です。← `BookDatabase`

そのため rsshogi は、これらを別の抽象として切り分けています。

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
  **ponder・出現回数・勝率・コメントは保持しません（意図的に lossy）**。
- `MemoryBook` は `HashMap<BookKey, Vec<BookMove>>` のメモリ常駐表。
  `StaticBook` はそれをソート済みバイナリへ焼き込み、実行時コストゼロで参照します。
- `Book::get()` は既にメモリ上にある `&[BookMove]` を借用して返す契約です。

「探索エンジンが現局面の候補手を引く」用途では、この層がほぼ理想形です。

### 外部リーダ層（`YaneuraOuBook` / `SbkBook` / `YbbBook`）

外部フォーマットの **読み取り専用リーダ**です。`Book` トレイトは実装しません。

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

- `BookKey` だけでは SFEN・Packed SFEN・元 row の手数・ファイル由来の同一性を
  復元できないため、`BookDatabase` は SFEN を含む position data を保持し、
  `BookKey` は派生ヘルパ扱いにしています。
- `BookDatabaseEntry::from_yaneuraou()` / `from_sbk()` で外部エントリを取り込み、
  `to_memory_book()` / `to_static_book()` で lossy なルックアップ表へ射影します。
- `write_yaneuraou_db2016()` / `write_sbk()` で編集済みデータベースを外部形式へ
  書き戻せます。SBK の top-level author / description は `BookDatabase` の対象外なので、
  `write_sbk()` は局面と候補手のデータを出力します。

## 「`MemoryBook` / `StaticBook` が一番理想なのか？」

用途を固定すれば Yes、ライブラリ全体の唯一解としては No です。

- 「局面を引く」用途では理想形。`StaticBook` は実行時コストゼロで配布にも向きます。
- ただし `MemoryBook` は構造的に lossy で、局面そのものを保持しません
  （`BookKey` ハッシュのみ）。Zobrist が衝突しても検証できず、キーから SFEN を
  復元することもできません。これは「引ければ十分」な層だから許される割り切りです。
- 大規模定跡をメモリ常駐させるのは重く、外部リーダの遅延ルックアップの方が
  現実的な場面もあります。

つまり 3 つの層はどれかが上位なのではなく、用途ごとに理想が違うから併存している
という設計です。参照の入口を `Book` トレイトに固定してあるのは、用途が増えたときに
実装だけを足せるようにするためです。

## 関連項目

- [定跡バイナリ](book.md)：`StaticBook` のバイナリ形式仕様
- [外部定跡（DB2016 / YBB / SBK）](external-books.md)：外部リーダと writer の Rust API
- [DB2016 形式](yaneuraou.md)：DB2016 フォーマットの詳細
- [SBK 形式](sbk.md)：SBK フォーマットの詳細
- [定跡 (Book)（Python）](../../python/book.md)：Python からの利用
