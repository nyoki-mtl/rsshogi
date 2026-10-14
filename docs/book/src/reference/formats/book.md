# StaticBook バイナリフォーマット仕様

`StaticBook`（`rsshogi::book::StaticBook`）は、`rsshogi` 独自の読み取り専用定跡バイナリフォーマット（現行バージョン `2`）である。
局面キー配列、オフセット配列、および固定長候補手配列の 3 段構造をとることで、大規模な定跡に対する二分探索（`binary_search`）とスライス参照を最小のメモリフットプリントで実現する。

## 基本設計

- **主キー**：手数（ply）を除いた局面の Zobrist ハッシュである `BookKey`（通常ビルドでは 64-bit、Cargo feature `hash-128` 有効時は 128-bit）を使用する。
- **可変長の候補手リスト**：オフセット配列 `offsets` によって、1 局面あたり任意の個数の `BookMove` スライスを表現する。
- **エンディアン**：すべてのマルチバイト整数はリトルエンディアン（LE）で記録される。

## 1. ワイヤーレイアウト仕様

バイナリデータは、22 バイトの固定長ヘッダに続けて、`keys` 配列、`offsets` 配列、`moves` 配列を隙間なく配置する。

```text
+-----------------------------------------------------------------------+
| Header (22 bytes)                                                     |
|   magic: b"RSHOGIBK" (8B) | version: u16 (2B) | flags: u16 (2B)       |
|   key_bytes: u8 (1B)      | reserved: u8 (1B)                         |
|   node_count: u32 (4B)    | move_count: u32 (4B)                      |
+-----------------------------------------------------------------------+
| keys    (node_count * key_bytes bytes)                                |
+-----------------------------------------------------------------------+
| offsets ((node_count + 1) * 4 bytes)                                  |
+-----------------------------------------------------------------------+
| moves   (move_count * 6 bytes)                                        |
+-----------------------------------------------------------------------+
```

ファイル全体のバイト長は厳密に `22 + node_count * key_bytes + (node_count + 1) * 4 + move_count * 6` と一致しなければならず、不足または余剰バイトがある場合は `BookError::InvalidFormat` を返す。

### ファイルヘッダ（22 バイト）

| オフセット | サイズ | 型 | フィールド名 | 内容と検証規則 |
| :--- | :--- | :--- | :--- | :--- |
| `0x00` | 8B | `[u8; 8]` | `magic` | 固定文字列 `b"RSHOGIBK"`。不一致時は `BookError::InvalidFormat("magic mismatch")`。 |
| `0x08` | 2B | `u16` | `version` | `2` 固定。`2` 以外の値は `BookError::Unsupported("version mismatch")` を返す。 |
| `0x0A` | 2B | `u16` | `flags` | `0x0003`（`FLAG_HAS_SCORE \| FLAG_HAS_DEPTH`）。ビット 0（score）とビット 1（depth）の両方が立っていない場合は `BookError::Unsupported`。 |
| `0x0C` | 1B | `u8` | `key_bytes` | 1 キーあたりのバイト数。通常ビルドでは `8`（64-bit Zobrist）、`hash-128` ビルドでは `16`（128-bit Zobrist）。実行中ビルドのキー幅と異なる場合は `BookError::Unsupported`。 |
| `0x0D` | 1B | `u8` | `reserved` | 予約領域（書き出し時は `0`）。 |
| `0x0E` | 4B | `u32` | `node_count` | 登録局面数 `N`。 |
| `0x12` | 4B | `u32` | `move_count` | 全候補手の合計数 `M`。 |

### `keys` 配列（`node_count * key_bytes` バイト）

`node_count` 個の `BookKey` を `(low_u64, high_u64)` の辞書順による **厳密な昇順（重複なし）** で格納する。

- `key_bytes == 8` の場合：各要素は `low_u64: u64`（8 バイト LE）。
- `key_bytes == 16` の場合：各要素は `low_u64: u64`（8 バイト LE）に続けて `high_u64: u64`（8 バイト LE）の計 16 バイト。

隣接するキーで `keys[i] >= keys[i + 1]` となる箇所が存在する場合は `BookError::InvalidFormat("keys must be strictly increasing")` となる。

### `offsets` 配列（`(node_count + 1) * 4` バイト）

`node_count + 1` 個の `u32`（LE）を格納する。
`i` 番目（`0 <= i < node_count`）の局面に対応する候補手スライスは `moves[offsets[i] as usize .. offsets[i + 1] as usize]` となる。

`offsets` 配列は以下の不変条件を満たす必要がある。

- 要素数が `node_count + 1` であること。
- 先頭要素が `offsets[0] == 0` であること。
- 末尾要素が `offsets[node_count] as usize == move_count` であること。
- すべての `i` について単調非減少（`offsets[i] <= offsets[i + 1]`）であること。

### `moves` 配列（`move_count * 6` バイト、`BookMove`）

各候補手（`BookMove`）は 6 バイト固定長で連続配置される。

| オフセット | サイズ | 型 | フィールド名 | 説明 |
| :--- | :--- | :--- | :--- | :--- |
| `0x00` | 2B | `u16` | `mv` | 16-bit 圧縮指し手（`Move::raw()`、YaneuraOu 互換ビットレイアウト）。 |
| `0x02` | 2B | `i16` | `score` | 評価値（`Eval::raw()`、手番視点）。 |
| `0x04` | 2B | `u16` | `depth` | 探索深さ。 |

## 2. 候補手のソート順と生成経路の違い

外側の `keys` 配列は常に昇順ソートされるが、同一局面内の候補手（`moves[offsets[i]..offsets[i+1]]`）の並び順は `StaticBook` の生成経路によって異なる。

| 生成メソッド | 同一局面内の候補手の並び順 | 主な用途 |
| :--- | :--- | :--- |
| `BookBuilder::build_static()` | `(score 降順, depth 降順, mv.raw() 昇順)` に決定的にソートされる。 | 棋譜集合や挿入データから優先度順の定跡を構築する場合。 |
| `StaticBook::from_memory_book_sorted(&memory_book)` | `(score 降順, depth 降順, mv.raw() 昇順)` に決定的にソートされる。 | `MemoryBook` から最善手先頭の `StaticBook` を得る場合。 |
| `StaticBook::from_memory_book(&memory_book)` | `MemoryBook` に登録された候補手の順序をそのまま保持する。 | 呼び出し側で制御した候補手順序を維持したい場合。 |
| `BookDatabase::to_static_book(&options)` | `BookDatabase` 内の候補手リスト（`candidates`）の順序をそのまま保持する。 | DB2016 や SBK など外部定跡の候補手順序を保って変換する場合。 |

## 3. コード例

### Python

```python
from rsshogi import Board, BookBuilder, BookMove, MemoryBook, Move, Record, StaticBook, book_key_from_position

# 1. BookBuilder で Record（本手順と分岐）を取り込み StaticBook を生成する
builder = BookBuilder()
record = Record.from_kif_file("game.kif")
builder.extend_from_game_record(record, default_score=0, default_depth=16)

static_book = builder.build_static()
static_book.write_file("opening.book")

# 2. ファイルから読み込んで初期局面の候補手を検索する
loaded = StaticBook.from_file("opening.book")
board = Board()
key = book_key_from_position(board)
entry = loaded.get(key)
if entry is not None:
    for bm in entry.moves:
        print(bm.mv.to_usi(), bm.score, bm.depth)
```

### Rust

```rust
use rsshogi::board::{self, Position};
use rsshogi::book::{Book, BookBuilder, StaticBook, book_key_from_position};

board::init();
let pos = board::hirate_position();

// BookBuilder からソート済み StaticBook を構築して保存
let mut builder = BookBuilder::new();
builder.extend_from_game_record(&record, 0, 16)?;
let static_book = builder.build_static();
static_book.write_file("opening.book")?;

// StaticBook を読み込んで検索
let loaded = StaticBook::from_file("opening.book")?;
let key = book_key_from_position(&pos);
if let Some(entry) = loaded.get(key) {
    for bm in entry.moves() {
        println!("move={} score={} depth={}", bm.mv().to_usi(), bm.score(), bm.depth());
    }
}
```

## 関連項目

- [定跡 3 層アーキテクチャ](book-architecture.md)
- [YaneuraOu 定跡フォーマット仕様（DB2016 / YBB）](yaneuraou.md)
- [ShogiHome SBK フォーマット仕様](sbk.md)
