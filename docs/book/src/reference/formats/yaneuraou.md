# DB2016 形式

DB2016（拡張子 `.db`、ヘッダ `#YANEURAOU-DB2016 1.00`）は
[やねうら王](https://github.com/yaneurao/YaneuraOu) の標準定跡フォーマットです。
**テキスト形式**で、1 局面ごとに SFEN 行とそれに続く定跡手行を並べます。

rsshogi は [`YaneuraOuBook`](external-books.md#db2016) による読み取りと、`BookDatabase` からの書き出しを提供します。
本ページはフォーマットそのものの構造と、rsshogi が解釈・出力する範囲を説明します。

> 出典: やねうら王ブログ
> [「標準将棋定跡フォーマットについて」](https://yaneuraou.yaneu.com/2016/02/05/standard-shogi-book-format/)
> および Wiki [「定跡の作成」](https://github.com/yaneurao/YaneuraOu/wiki/%E5%AE%9A%E8%B7%A1%E3%81%AE%E4%BD%9C%E6%88%90)。
> パースの細部は rsshogi のリーダ（`crates/rsshogi/src/book/yaneuraou.rs`）が
> 実際に解釈する範囲に基づきます。

## 形式の性質

DB2016 は SFEN 文字列を局面キーにする可読なテキスト形式です。
同じ局面を異なる手数で記録した場合も、rsshogi の検索では盤面・手番・持ち駒だけを比較します。

## ファイル構造

```text
#YANEURAOU-DB2016 1.00
sfen lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1
7g7f 8c8d 0 32 1
2g2f 3c3d 0 32 1
sfen <次の局面の SFEN>
<指し手行...>
```

- 1 行目は任意のヘッダ `#YANEURAOU-DB2016 1.00`。
- `sfen ` で始まる行: 局面（SFEN 文字列）。次の `sfen` 行までが 1 局面のブロック。
- それ以外の行: 直前の局面に対する定跡手。

## 局面行（sfen 行）

```text
sfen lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1
```

`sfen <盤面> <手番> <持駒> [手数]` の 3 要素と任意の手数で構成されます。

rsshogi はこの手数を検索キーに含めず、局面を手数 `1` の SFEN に正規化します。
行に書かれた手数は `YaneuraOuBookEntry::min_ply()` として取得できます。
手数列を省略した入力は `0` として公開します。

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

- 後方カラムは省略可能です。
- 中間の数値列を省くときは `none` または `None` を使います。
- `resign` は指し手トークンとして受理されます。

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

rsshogi は `#` 行・`//` 行を局面または直前の指し手コメントとして保持し、UTF-8 BOM・CRLF・LF を許容します。
`iter_entries()` は局面ブロックごとに結果を返すため、不正な指し手行を含むブロックを `Err` として報告した後も、後続ブロックの反復を続けられます。

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

`YaneuraOuBook` は読み取り専用ですが、編集 IR である [`BookDatabase`](book-architecture.md#編集-ir-層bookdatabase)
から DB2016 テキストを**書き出す**ことができます。

```rust,ignore
use rsshogi::book::{BookDatabase, YaneuraOuDb2016WriteOptions};

let mut file = std::fs::File::create("out.db")?;
db.write_yaneuraou_db2016(&mut file, &YaneuraOuDb2016WriteOptions::new())?;

let text = db.to_yaneuraou_db2016_string(&YaneuraOuDb2016WriteOptions::new())?;
```

`write_yaneuraou_db2016()` は同じ文字列表現を writer に書き込む API です。
巨大な本を扱う際は、出力文字列がメモリに構築される点を考慮します。

### 出力契約

- 局面行は正規化 SFEN の bytewise strict 昇順で出力します。
- 同じ正規化 SFEN が重複すると既定ではエラーになり、`with_keep_last_on_duplicate()` を指定した場合だけ最後の entry を出力します。
- 候補手は既定で score の降順になり、score がない候補は末尾です。
- `with_preserved_move_order()` は候補手を入力順に保ちます。
- 各候補手行は `move ponder score depth` を常に出力し、`count` は値がある場合だけ第 5 列として出力します。
- したがって count がない候補は count 列を出力せず、writer は架空の count を補いません。
- `ponder`、score、depth がない場合は対応する列に `none` を出力します。
- 局面の元手数が `0` の場合、既定の writer は手数列を出力しません。
- `with_fixed_ply(n)` は元手数を `n` に置き換え、`with_omitted_ply()` はすべての局面で手数列を省略します。
- 局面または候補手の複数行コメントは、対応する行の直後に複数の `# ...` 行として出力します。
- `# NOE` 行は `with_noe(true)` を指定した場合だけ出力します。

## 関連項目

- [定跡アーキテクチャ（3 層モデル）](book-architecture.md)：外部リーダの位置づけ
- [外部定跡（DB2016 / YBB / SBK）](external-books.md)：`YaneuraOuBook` / `YbbBook` / `SbkBook` の Rust API
- [SBK 形式](sbk.md)：もう一つの外部定跡フォーマット
- [外部定跡（Python）](../../python/external-books.md)：Python からの利用
