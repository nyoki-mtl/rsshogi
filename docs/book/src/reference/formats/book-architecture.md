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
│   読み取り専用・遅延ロード・フォーマット忠実(lossless)          │
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

### ルックアップ層 — `MemoryBook` / `StaticBook`

`Book` トレイトを実装する、**正規化済みの参照表現**です。

- キーは [`book_key_from_position()`](book.md) が返す `BookKey`
  （盤面 + 持駒 + 手番の Zobrist key）。定跡における「同じ局面」の定義に忠実です。
- 1 手分のデータは `BookMove { mv, score: i16, depth: u16 }` のみ。
  **ponder・出現回数・勝率・コメントは保持しません（意図的に lossy）**。
- `MemoryBook` は `HashMap<BookKey, Vec<BookMove>>` のメモリ常駐表。
  `StaticBook` はそれをソート済みバイナリへ焼き込み、実行時コストゼロで参照します。
- `Book::get()` は既にメモリ上にある `&[BookMove]` を借用して返す契約です。

「探索エンジンが現局面の候補手を引く」用途では、この層がほぼ理想形です。

### 外部リーダ層 — `YaneuraOuBook` / `SbkBook` / `YbbBook`

外部フォーマットの **読み取り専用リーダ**です。`Book` トレイトは実装しません。

- キーが Zobrist ではなく SFEN 文字列（DB2016）/ Packed SFEN（SBK / YBB）。
  ファイルはこの順で並んでおり、全局面の Zobrist を計算してインデックス化すると
  「ファイル全体を読まない遅延設計」を捨てることになります。
- フォーマット固有のメタ情報を **lossless** で公開します。これを `MemoryBook` に
  直接変換すると `ponder` / `move_count` などが無音で捨てられるため、固有型のまま
  提供します。
- 巨大ファイルでもサイズ比例のメモリ確保をせず、必要な部分だけオンデマンドで
  デコードします。

詳細は [外部定跡（DB2016 / YBB / SBK）](external-books.md) と
[SBK 形式](sbk.md) を参照してください。

### 編集 IR 層 — `BookDatabase`

外部フォーマットとルックアップ層を **橋渡しする中間表現**です。

- `BookKey` だけでは SFEN・Packed SFEN・元 row の手数・ファイル由来の同一性を
  復元できないため、`BookDatabase` は SFEN を含む position data を保持し、
  `BookKey` は派生ヘルパ扱いにしています。
- `BookDatabaseEntry::from_yaneuraou()` / `from_sbk()` で外部エントリを取り込み、
  `to_memory_book()` / `to_static_book()` で lossy なルックアップ表へ射影します。
- `write_yaneuraou_db2016()` / `write_sbk()` で編集済みデータベースを外部形式へ
  書き戻せます。SBK の top-level author / description は現行 `BookDatabase` に格納先が
  ないため初期 writer では出力しません。YBB writer はまだ提供しません。

## 「`MemoryBook` / `StaticBook` が一番理想なのか？」

用途を固定すれば Yes、ライブラリ全体の唯一解としては No です。

- 「局面を引く」用途では理想形。`StaticBook` は実行時コストゼロで配布にも向きます。
- ただし `MemoryBook` は構造的に lossy で、局面そのものを保持しません
  （`BookKey` ハッシュのみ）。Zobrist が衝突しても検証できず、キーから SFEN を
  復元することもできません。これは「引ければ十分」な層だから許される割り切りです。
- 大規模定跡をメモリ常駐させるのは重く、外部リーダの遅延ルックアップの方が
  現実的な場面もあります。

つまり 3 つの層はどれかが上位なのではなく、用途ごとに理想が違うから併存している
という設計です。将来「衝突検証のために局面を持つ `Book` 実装」や「mmap ベースの
大規模 `Book`」が必要になっても、`Book` トレイトはそのままに実装を増やせる余地を
残しています。

## book solver 層（peta_shock 相当）

> ⚠️ **advanced / experimental**。`hash-128` feature 必須。公開面は入口 2 つだけで、
> 初心者向けの 3 型（`StaticBook` / `MemoryBook` / `BookDatabase`）の見通しは崩しません。

参照実装の `makebook peta_shock` 相当（定跡 DAG 上の min-max 後退解析 + 千日手処理）を
rsshogi で行う層です。**評価関数・探索は一切使いません**（eval-free）。入力 `BookDatabase` に
既に書かれている leaf 評価値を、定跡グラフ上で min-max 伝播し、千日手（引き分け 0）と連続王手の
千日手（±MATE + sentinel depth）を処理して、評価値降順に整形した `BookDatabase` を返します。
`think`（評価関数で読んで定跡を作る生成）は engine 層の責務で、この層には含めません。

公開するのは **入口関数 1 つ + options 型 1 つ**だけです。

```rust,ignore
use rsshogi::book::{solve_peta_shock_book, PetaShockOptions, YaneuraOuDb2016WriteOptions};

// 既存スコア付き定跡を後退解析で整形する（戻り値も BookDatabase）。
let solved = solve_peta_shock_book(&db, &PetaShockOptions::new().with_shrink(true))?;

// DB2016 への書き出しは writer に委ねる（solve と write は別関心）。
let text = solved.to_yaneuraou_db2016_string(&YaneuraOuDb2016WriteOptions::peta_shock())?;
```

内部の作業表現（`BookGraph` / `GraphNode` / `ValueDepth` / depth sentinel など）は **すべて
非公開**（`pub(crate)`）です。`BookDatabase` は入出力のハブに徹し、solver の作業構造には
しません（編集 IR の責務を保つため）。

### 設計の要点

- **手番正準化**: node は 128bit Zobrist hash で同一視します。hashkey は後手番化した局面で計算し、
  node の指し手は先手番化した局面の指し手で保持、元の手番は出力再現用に保持します（ある局面と
  その先後反転を同一視して合流させる FlippedBook 互換）。`hash-128` が無効だとエラーになります
  （64bit では別局面がマージされ solve が壊れるため）。
- **エッジ構築**: 入力に明示されない隣接（合流）を、全合法手展開 + `key_after_move` + hash 照合で
  動的に発見します。
- **後退解析**: 出次数 0 の部分（DAG）を確定・凍結 → 連続王手の千日手ループを抽出 → 残るサイクルを
  千日手スコアで初期化 → 不動点まで評価値を親へ伝播（check loop 上だけ DFS）。
- **出力整形**: 評価値降順 sort・shrink（最善 value のみ）・連続王手の depth 調整・元手番への flip 戻し。

## 関連項目

- [定跡バイナリ](book.md)：`StaticBook` のバイナリ形式仕様
- [外部定跡（DB2016 / YBB / SBK）](external-books.md)：外部リーダと writer の Rust API
- [DB2016 形式](yaneuraou.md)：DB2016 フォーマットの詳細
- [SBK 形式](sbk.md)：SBK フォーマットの詳細
- [定跡 (Book)（Python）](../../python/book.md)：Python からの利用
