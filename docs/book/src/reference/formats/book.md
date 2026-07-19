# Book バイナリ形式

`rsshogi::book::StaticBook` が使用する **定跡バイナリ形式**の仕様です。
読み取り専用の構成で、**大量局面に対して高速参照**できることを優先しています。

## 基本方針

- **Zobristキー**（64/128bit）を主キーとする
- **可変長の指し手数**に対応する
- **score / depth は必須**
- 参照は `binary_search + slice` のみ

## エンディアン

すべて **little-endian**。

## ファイル構造

```text
| magic (8) | version (2) | flags (2) | key_bytes (1) | reserved (1) |
| node_count (4) | move_count (4) |
| keys [node_count * key_bytes] |
| offsets [(node_count + 1) * 4] |
| moves [move_count] |
```

### magic

- 固定文字列: `RSHOGIBK`

### version

- 現行: `1`

### flags

| ビット | 意味 |
|--------|------|
| 0 | score を含む |
| 1 | depth を含む |

> 現行仕様では **score/depth は必須**。両方が立っていない場合は読み込み不可。

### key_bytes

| 値 | 意味 |
|----|------|
| 8  | 64bit Zobrist |
| 16 | 128bit Zobrist |

読み込み時の `key_bytes` は、現在の build の `BookKey` 幅と一致している必要があります。
default build は 8、`hash-128` feature build は 16 です。幅が違うファイルは、検索キーが
一致しないため読み込み時にエラーになります。

### node_count / move_count

| 項目 | 型 | 説明 |
|------|----|------|
| node_count | u32 | 局面数 |
| move_count | u32 | 指し手総数 |

## keys 配列

`node_count` 個のキーを **strict 昇順**で保持します。

- ソート順: `(low_u64, high_u64)` の昇順

## offsets 配列

`node_count + 1` 個の **u32** を保持します。

- `offsets[i]..offsets[i+1]` が `i` 番目の局面の手一覧
- `offsets[0] == 0` を満たす
- 最後の `offsets[node_count] == move_count` を満たす
- 非減少であること

## moves 配列

`move_count` 個の `BookMove` を連続配置します。

### BookMove レイアウト

| 項目 | 型 | 説明 |
|------|----|------|
| mv | u16 | 16bit指し手 |
| score | i16 | 評価値（`Eval::raw()`） |
| depth | u16 | 深さ |

## 制約・注意

- `StaticBook` の参照は **常に read-only**
- `score/depth` の欠落は非対応
- Zobristの衝突は理論上あり得る（確率は極小）

## 参考実装

- `crates/rsshogi/src/book/static_book.rs`

## 関連項目

- [定跡アーキテクチャ（3 層モデル）](book-architecture.md)：`StaticBook` /
  `MemoryBook` と外部リーダの位置づけ
- [外部定跡（DB2016 / YBB / SBK）](external-books.md)：外部フォーマットの参照
