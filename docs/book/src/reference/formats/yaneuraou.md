# DB2016 形式

DB2016（拡張子 `.db`、ヘッダ `#YANEURAOU-DB2016 1.00`）は
[やねうら王](https://github.com/yaneurao/YaneuraOu) の標準定跡フォーマットです。
**テキスト形式**で、1 局面ごとに SFEN 行とそれに続く定跡手行を並べます。

rsshogi は読み取り専用リーダ [`YaneuraOuBook`](external-books.md#db2016) を
提供します。本ページはフォーマットそのものの構造と、rsshogi がそれをどう解釈するかを
説明します。

> 出典: やねうら王ブログ
> [「標準将棋定跡フォーマットについて」](https://yaneuraou.yaneu.com/2016/02/05/standard-shogi-book-format/)
> および Wiki [「定跡の作成」](https://github.com/yaneurao/YaneuraOu/wiki/%E5%AE%9A%E8%B7%A1%E3%81%AE%E4%BD%9C%E6%88%90)。
> パースの細部は rsshogi のリーダ（`crates/rsshogi/src/book/yaneuraou.rs`）が
> 実際に解釈する範囲に基づきます。

## 設計思想（なぜテキストか）

DB2016 は当初の SQLite ベースから移行して生まれました。著者の方針は明快で、
「起動時に定跡ファイルを丸読みして `std::map` に持っておけばよい」、
複雑なバイナリ形式は不要、というものです。これにより、

- **可読性・編集性**: テキストエディタで直接開いて編集できる。
- **互換性**: キーをハッシュ値ではなく **SFEN 文字列**にしたことで、エンジン更新で
  ハッシュ計算が変わっても定跡が壊れない。

メモリ効率より可読性・編集性・互換性を優先した形式です（この点が、コンパクトさを
狙う SBK や Apery BIN とは対照的）。

## ファイル構造

```text
#YANEURAOU-DB2016 1.00
sfen lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1
7g7f 8c8d 0 32 1
2g2f 3c3d 0 32 1
sfen <次の局面の SFEN>
<指し手行...>
```

- 1 行目: ヘッダ `#YANEURAOU-DB2016 1.00`。
- `sfen ` で始まる行: 局面（SFEN 文字列）。次の `sfen` 行までが 1 局面のブロック。
- それ以外の行: 直前の局面に対する定跡手。

## 局面行（sfen 行）

```text
sfen lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1
```

`sfen <盤面> <手番> <持駒> <手数>` の 4 要素で、**手数（ply）を含みます**。

rsshogi はこの手数を検索キーには含めず、**キーの同一性のために手数を `1` へ正規化**し、
元の手数は `min_ply` として保持します（[`YaneuraOuBookEntry::min_ply()`]）。
これは「同一局面は手数によらず同じ定跡」という [position identity の方針](book-architecture.md)
に沿った扱いで、ShogiHome の挙動とも一致します。

## 定跡手行

各フィールドは半角スペース区切りで、次の順に並びます。

```text
move  ponder  value  depth  move_count
```

| 位置 | フィールド | 意味 |
|------|-----------|------|
| 1 | `move` | 現局面での着手（USI 形式） |
| 2 | `ponder` | 予想される相手の応手（無い場合は `none`） |
| 3 | `value` | 評価値（歩 = 100 点） |
| 4 | `depth` | 探索深さ |
| 5 | `move_count` | 出現回数、または評価値付き定跡ではエンジンバージョン（例: v3.21 → `321`） |

- **後方カラムは省略可能**。途中のカラムを省くときは `none` で埋めます。
- rsshogi は `none` / `None` / 空欄をいずれも「値なし」として受理します。

サンプル行（rsshogi のテストフィクスチャより）:

```text
4e7h+ none 540 38 1
9f9e  none none none
```

1 行目は「成りを伴う着手、ponder なし、評価値 540、depth 38、出現回数 1」。
2 行目は評価値・depth・回数をすべて省略した形です。

rsshogi の `YaneuraOuBookMove` は `mv` / `ponder` / `score` / `depth` / `count` /
`comment` を保持します（`MemoryBook` の `BookMove` が `ponder` や `count` を
落とすのに対し、外部リーダ層では lossless に公開します。
→ [定跡アーキテクチャ](book-architecture.md)）。

## コメント行

- 独立した `# ...` 行と `// ...` 行はコメント扱い。
- 指し手行の 5 カラム以降は、その指し手のコメントとして保持。
- `sfen` で始まらないその他の非空行は、コメントではなく定跡手行として解析します。

rsshogi は `#` 行・`//` 行を局面または直前の指し手コメントとして保持し、
UTF-8 BOM・CRLF・LF を許容します。

## ソート順と二分探索

DB2016 の局面行は通例ソートされています。

- **標準定跡**: 出現頻度の高い順。
- **評価値付き定跡**: 手番側から見て評価値の良い順。
- いずれも **1 番目の指し手が最善手**である保証があります。

rsshogi はこのソートを利用して `lookup_sfen()` を二分探索で行いますが、
**ソートが検証できた場合のみ**に限ります。`open()` は先頭の一定範囲だけを検証し、
診断が `complete == false` のときは `validate_full()` を促すか、明示的な
`lookup_sfen_by_scan()` を要求します。詳細は
[外部定跡（DB2016 / YBB / SBK）](external-books.md#db2016) を参照してください。

## 書き出し（writer）

`YaneuraOuBook` は読み取り専用ですが、編集 IR である [`BookDatabase`](book-architecture.md#編集-ir-層--bookdatabase)
から DB2016 テキストを**書き出す**ことができます。

```rust,ignore
use rsshogi::book::{BookDatabase, YaneuraOuDb2016WriteOptions};

// 主 API: ストリーミング書き出し（巨大な本でもメモリ常駐を避ける）
let mut file = std::fs::File::create("out.db")?;
db.write_yaneuraou_db2016(&mut file, &YaneuraOuDb2016WriteOptions::new())?;

// 便宜 API: String 版（薄い wrapper）
let text = db.to_yaneuraou_db2016_string(&YaneuraOuDb2016WriteOptions::new())?;
```

writer は「素直なシリアライザ」です。値の解釈・clamp・正規化・手番 flip は一切行いません
（整形・正規化は [book solver](book-architecture.md#book-solver-層peta_shock-相当) の責務）。

### 出力契約

- **position 行のソート（常時適用・options 外）**: normalized SFEN（ply を `1` に正規化した形）の
  **strict 昇順**で出力します。`BookDatabase` の entry 順では出しません。normalized SFEN が重複する
  entry は既定で**エラー**（`with_keep_last_on_duplicate()` で後勝ちにできます）。これにより出力は
  常に `lookup_sfen()` の二分探索が成立する形（`open()` の診断が `Sorted { complete: true }`）になります。
- **指し手の並び（`YaneuraOuDb2016WriteOptions`）**: 既定は**評価値降順**（`ScoreDescending`、
  `score == None` は末尾・同 score は元順維持の stable sort）。`with_preserved_move_order()` で入力順を保持します。
- **列セット**: 既定は **lossless**（`count` / `comment` / `ponder` を保持し round-trip が閉じる）。
  `YaneuraOuDb2016WriteOptions::peta_shock()` は **`move none value depth` の 4 列**に絞り、ponder は
  常に `none`、count/comment を落とします（peta_shock 出力用の lossy profile）。
- **depth sentinel**: `9997` / `9998` / `9999` も writer は**素通し**（解釈しません）。これらの生成は solver の責務です。
- **ply**: 既定は `Preserve`（`original_ply` が `Some(n≥1)` ならその n、`Some(0)` なら省略、`None` なら 1）。
  `with_fixed_ply(n)` / `with_omitted_ply()` で上書きできます。
- **コメント**: 複数行コメントは複数の `# ...` 行に分解して出力します（position 行直後＝局面コメント、
  指し手行直後＝その手のコメント）。
- **`# NOE` 行**: 既定では出力しません（`with_noe(true)` で出力可）。

公開 API は **書き込み 1 + String 版 1 + options 1** に絞っています。

## 関連ツールと発展形式

- **makebook**: 定跡生成コマンド。`think`（思考生成）/ `from_sfen` / `merge` / `sort`
  などのサブコマンドがあります。
- **makebook peta_shock**: 千日手を含む複雑な定跡に対応する新しい生成手法。
  depth に特殊コード（`999` = 抜け出せないループ、`+1000`/`+2000` = 千日手打開権の
  有無）を用います。
- **ScriptCollection / makebook**: split・merge・sort・convert などの
  処理は Python 実装へ移行しています。

rsshogi が対象とするのは、これらが出力する DB2016 テキストの**読み取り**です。

## 関連項目

- [定跡アーキテクチャ（3 層モデル）](book-architecture.md)：外部リーダの位置づけ
- [外部定跡（DB2016 / YBB / SBK）](external-books.md)：`YaneuraOuBook` / `YbbBook` / `SbkBook` の Rust API
- [SBK 形式](sbk.md)：もう一つの外部定跡フォーマット
- [外部定跡（Python）](../../python/external-books.md)：Python からの利用
