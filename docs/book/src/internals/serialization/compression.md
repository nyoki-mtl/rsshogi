# 局面の圧縮

> **前提知識**: [棋譜・局面フォーマット](./formats.md)（各フォーマットの特徴と使い分け）

## このページの要点

- 将棋の合法局面数は約 10⁷¹、理論的には **~236 ビット**で一意に識別可能
- 実用的な **256 ビット（32 バイト）圧縮**は静的ハフマン符号化に基づき、固定サイズでランダムアクセス可能
- 具体的な圧縮効果: 100 万局面 × 256 bit = **32 MB**（SFEN なら ~80 MB、非圧縮なら ~400 MB）
- 圧縮は**保存時のみ**適用し、探索中は通常の Position を使うのが一般的

将棋の局面を効率的に圧縮して保存・転送する技術について解説します。
これは定跡データベース、棋譜保存、ネットワーク転送などで重要な技術です。

## なぜ局面の圧縮が必要か

将棋の局面をナイーブに表現すると、かなりのメモリを消費します：

- **盤面**: 81マス × 駒の種類（16種類） = 最低でも4ビット/マス
- **持ち駒**: 各駒種×枚数
- **手番**: 1ビット
- **その他の状態**: 手数、王手フラグなど

合計すると、1局面あたり数百バイトになることもあります。

定跡データベースや棋譜データベースでは、数百万局面を扱うことがあるため、圧縮技術が不可欠です。

## 理論的限界：完全ハッシュは何ビットで表現できるか

### 将棋の状態空間

将棋の全ての合法局面を一意に識別するには、最低何ビット必要でしょうか？

これは「完全ハッシュ（perfect hash）」の問題として知られています。[^yaneuraou-perfect-hash]

### 理論的な下限

将棋の合法局面数は約 10^71 と推定されています。
これをビットで表現するには：

```text
log2(10^71) ≈ 236 ビット
```

つまり、理論的には **236ビット** あれば全ての合法局面を一意に識別できます。

### エントロピーに基づく推定

より精密な計算では、情報理論のエントロピーを使います：

```text
255 < log2(83^40) < 256
```

ここで：
- **83**: 各駒が取りうる位置（盤上81マス + 先手持ち駒 + 後手持ち駒）
- **40**: 玉以外の駒の総数

この計算によれば、**255〜256ビット** で表現できる可能性があります。

### なぜ範囲があるのか

駒の種類や配置には制約があります：

- **歩**: 最大18枚（各筋に1枚ずつまで）
- **成駒**: 元の駒との対応関係
- **持ち駒の上限**: 盤上の駒数との関係

これらの制約を考慮すると、実際の状態空間はより小さくなります。

## 実用的な256ビット圧縮

### 基本戦略

やねうら王の開発者が考案した256ビット圧縮は、静的ハフマン符号化に基づいています。[^yaneuraou-256bit]
頻出する駒に短いビットを割り当てることで、効率的に圧縮します。
この方式のアイデアは**駒箱（piecebox）**の導入です。
盤上にも持ち駒にも存在しない駒を「駒箱」として専用ハフマン符号で書き込むことで、駒の所在が変わっても総ビット数が常に 256 ビットに収まるよう設計されています。

### エンコーディング手順

256 ビット圧縮は、以下の 4 段階で盤面を順次エンコードします。

**Step 1: ヘッダ（15 ビット固定）**

| 要素 | ビット数 | 説明 |
|------|----------|------|
| 手番 | 1 | 先手=0, 後手=1 |
| 先手玉の位置 | 7 | Square インデックス (0-80) |
| 後手玉の位置 | 7 | Square インデックス (0-80) |

**Step 2: 盤上の駒（可変長ハフマン符号化）**

81 マスを順に走査し（玉はスキップ）、各マスの状態をハフマン符号で書き込みます。
駒がある場合は手番ビット(1bit) を、さらに**金以外**なら成りフラグ(1bit) も書き込みます
（金は成れないため成りフラグを持たず、その分 1 ビット短くなります）。

| 駒種 | 符号長 | 合計（手番+成り含む） |
|------|--------|---------------------|
| 空マス | 1 bit | 1 bit |
| 歩 | 2 bit | 4 bit |
| 香・桂・銀 | 4 bit | 6 bit |
| 金 | 5 bit | 6 bit（成りフラグなし） |
| 角・飛 | 6 bit | 8 bit |

**Step 3: 持ち駒（ハフマン符号化）**

先手・後手の順に、駒種ごとに持ち駒の枚数分だけ符号を書き込みます。

**Step 4: 駒箱（piecebox）による帳尻合わせ**

将棋の駒は全 40 枚（玉 2 枚を除く）で固定です。
盤上にも持ち駒にも存在しない「駒箱」の駒を専用のハフマン符号で書き込むことで、
**駒の総数が一定 → 総ビット数が常に 256 ビット**になります。

駒が盤上→持ち駒に移動しても、駒箱の分が減って総量が変わりません。

### なぜ 256 ビット固定になるのか

各駒種の総枚数は決まっています（例: 歩 18 枚、香 4 枚）。
盤上・持ち駒・駒箱のいずれに属していても同じ駒種のハフマン符号長は等しいため、
駒の所在が変わっても合計ビット数は変化しません。

### PackedSfen 構造体

rsshogi では、圧縮された局面を `PackedSfen` 構造体で表現します：

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/board/position/packed_sfen.rs:packed_sfen_struct}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/board/position/packed_sfen.rs#L7-L12)</small>

### 実装の流れ

rsshogi の `Position::to_packed_sfen()` は以下の手順で 256 ビットに圧縮します：

```rust,ignore
pub fn to_packed_sfen(&self) -> PackedSfen {
    let mut writer = BitWriter::new(&mut words);

    // Step 1: ヘッダ（15 ビット固定）
    writer.write_one_bit(self.turn().raw() != 0);
    for color in [Color::BLACK, Color::WHITE] {
        writer.write_n_bits(self.king_square(color).raw(), 7);
    }

    // Step 2: 盤上の駒（可変長ハフマン）
    //   piecebox_count で「まだ使われていない駒」を追跡
    // huffman_index 順 [NONE, 歩, 香, 桂, 銀, 角, 飛, 金] の各駒種総数
    let mut piecebox_count = [0, 18, 4, 4, 4, 2, 2, 4];
    for sq in 0..81 {
        let piece = self.piece_on(sq);
        if piece.is_king() { continue; }
        write_board_piece(&mut writer, piece);   // ハフマン符号 + 手番 + 成り
        piecebox_count[piece_index] -= 1;        // 使用済みとしてカウント
    }

    // Step 3: 持ち駒
    for color in [BLACK, WHITE] {
        for piece_type in [歩, 香, 桂, 銀, 金, 角, 飛] {
            for _ in 0..Hand::count_of(hand, piece_type) {
                write_hand_piece(&mut writer, piece);
                piecebox_count[piece_index] -= 1;
            }
        }
    }

    // Step 4: 駒箱（残りの駒）
    for piece_type in [歩, 香, 桂, 銀, 金, 角, 飛] {
        for _ in 0..piecebox_count[piece_index] {
            write_piecebox_piece(&mut writer, piece_type);
        }
    }

    debug_assert!(writer.cursor() == 256);
    PackedSfen { data: words_to_bytes(&words) }
}
```

### ビットストリームの読み書き

効率的な圧縮には、ビット単位での読み書きが必要です：

```rust,ignore
struct BitWriter {
    data: Vec<u8>,
    current_byte: u8,
    bits_in_current: usize,
}

impl BitWriter {
    fn new() -> Self {
        BitWriter {
            data: Vec::new(),
            current_byte: 0,
            bits_in_current: 0,
        }
    }

    fn write_bit(&mut self, bit: bool) {
        if bit {
            self.current_byte |= 1 << self.bits_in_current;
        }

        self.bits_in_current += 1;

        if self.bits_in_current == 8 {
            self.data.push(self.current_byte);
            self.current_byte = 0;
            self.bits_in_current = 0;
        }
    }

    fn write_bits(&mut self, value: usize, num_bits: usize) {
        for i in 0..num_bits {
            let bit = (value >> i) & 1 == 1;
            self.write_bit(bit);
        }
    }

    fn into_bytes(mut self) -> [u8; 32] {
        if self.bits_in_current > 0 {
            self.data.push(self.current_byte);
        }

        let mut result = [0u8; 32];
        result[..self.data.len()].copy_from_slice(&self.data);
        result
    }
}
```

## ハフマン符号化の応用

### 駒の出現頻度

実戦の局面では、駒の出現頻度に偏りがあります：

- **歩**: 最も頻出（18枚）
- **金・銀**: 次に頻出（各4枚）
- **飛・角**: 比較的少ない（各2枚）

rsshogi では、以下の静的ハフマンテーブルを使用して駒を符号化します：

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/board/position/packed_sfen.rs:huffman_table}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/board/position/packed_sfen.rs#L122-L133)</small>

### 適応的ビット配分

静的な256ビット配分ではなく、局面ごとに最適なビット配分を計算することも可能です：

```rust,ignore
fn optimal_encoding(pos: &Position) -> Vec<u8> {
    // 1. 各駒種の出現回数を数える
    let freq = count_piece_frequencies(pos);

    // 2. ハフマン木を構築
    let huffman_tree = build_huffman_tree(freq);

    // 3. ハフマン符号で駒を符号化
    let encoded = encode_with_huffman(pos, &huffman_tree);

    // 4. ハフマン木の情報も含めて保存
    let mut result = serialize_huffman_tree(&huffman_tree);
    result.extend(encoded);

    result
}
```

この方法では、256ビット固定ではなく、可変長の圧縮が可能ですが、復元時にハフマン木の情報が必要です。

## 実用上の考慮事項

### 圧縮 vs 差分更新

Position の重要な特性の1つは、**差分更新（incremental update）**の高速性です：

```rust,ignore
// do_move() でハッシュ値を差分更新
new_hash = old_hash ^ zobrist_delta;  // O(1)
```

256ビット圧縮を使う場合、以下の選択肢があります：

1. **圧縮を保存のみに使う**: メモリ上では通常の Position、保存時のみ圧縮
2. **圧縮形式で探索**: 圧縮したまま差分更新（複雑だが省メモリ）

多くのエンジンは、1の方式（保存時のみ圧縮）を採用しています。

### 用途別の最適化

| 用途 | 最適な方式 |
|------|-----------|
| 定跡データベース | 256ビット固定（高速アクセス） |
| 棋譜保存 | 可変長圧縮（サイズ優先） |
| ネットワーク転送 | SFEN形式（可読性） |
| メモリキャッシュ | 通常の Position（速度優先） |

## 圧縮率の比較

各表現方式のサイズを比較します：

| 方式 | サイズ | 特徴 |
|------|--------|------|
| 通常の Position | 200-500バイト | 高速、差分更新可能 |
| SFEN文字列 | 60-100バイト | 可読性、デバッグ容易 |
| 256ビット圧縮 | 32バイト | 固定サイズ、高速アクセス |
| ハフマン符号化 | 25-35バイト | 可変長、最小サイズ |

## 実装のベストプラクティス

### 1. テストケースの重要性

圧縮・復元が正確であることを確認するテストは必須です：

```rust,ignore
#[test]
fn test_compression_roundtrip() {
    let positions = load_test_positions();

    for pos in positions {
        // 圧縮
        let compressed = CompressedPosition::from_position(&pos);

        // 復元
        let restored = compressed.to_position();

        // 完全一致を確認
        assert_eq!(pos, restored);
        assert_eq!(pos.hash(), restored.hash());
    }
}
```

### 2. エンディアンへの配慮

ビット列をバイト配列に変換する際、エンディアンに注意が必要です：

```rust,ignore
// リトルエンディアンで統一
fn write_u64_le(writer: &mut BitWriter, value: u64) {
    for i in 0..8 {
        let byte = (value >> (i * 8)) & 0xFF;
        writer.write_bits(byte as usize, 8);
    }
}
```

### 3. バージョン管理

圧縮形式は将来変更される可能性があるため、バージョン情報を含めます：

```rust,ignore
pub struct CompressedPosition {
    version: u8,      // 圧縮形式のバージョン
    data: [u8; 32],   // 256ビットデータ
}
```

## デバッグのヒント

### ビット配分の可視化

圧縮された局面のビット配分を可視化すると、デバッグが容易になります：

```rust,ignore
impl CompressedPosition {
    pub fn dump_bit_layout(&self) {
        let reader = BitReader::new(&self.data);

        println!("Bit layout:");
        println!("  [0-0]   Side to move: {}", reader.read_bit());
        println!("  [1-7]   Black king: {}", reader.read_bits(7));
        println!("  [8-14]  White king: {}", reader.read_bits(7));
        // ... 以下同様
    }
}
```

### 差分チェック

圧縮前後で情報が失われていないかを確認：

```rust,ignore
fn verify_compression(original: &Position, compressed: &CompressedPosition) {
    let restored = compressed.to_position();

    // 盤面の一致確認
    for sq in Square::all() {
        assert_eq!(
            original.piece_at(sq),
            restored.piece_at(sq),
            "Mismatch at {sq}"
        );
    }

    // 持ち駒の一致確認
    for color in [Color::BLACK, Color::WHITE] {
        assert_eq!(
            original.hand(color),
            restored.hand(color),
            "{color:?} hand mismatch"
        );
    }

    // 手番の一致確認
    assert_eq!(original.side_to_move(), restored.side_to_move());
}
```

## 落とし穴

### 圧縮・復元のラウンドトリップテスト

符号化のバグは「特定の駒配置でのみ発生する」沈黙的な不具合になりやすいです。
必ず大量の局面（perft で生成した全局面など）でラウンドトリップテストを実行してください。

### エンディアンの不一致

圧縮データをファイルに保存する場合、リトルエンディアン/ビッグエンディアンの違いで
異なるプラットフォーム間でデータが読めなくなることがあります。
rsshogi ではリトルエンディアンで統一しています。

## まとめ

- 理論的限界は ~236 ビット、実用的な 256 ビット圧縮はエントロピー限界に近い
- 静的ハフマン符号化により**固定 32 バイト**で任意の局面を表現
- 100 万局面で ~32 MB（SFEN 比 60% 削減、非圧縮比 92% 削減）
- 探索中は通常の Position、保存時のみ圧縮するのが実用的
- ラウンドトリップテストとエンディアン統一が実装の鍵

## 参考文献

[^yaneuraou-perfect-hash]: やねうら王, ["将棋の完全ハッシュは何bitで表現できますか？"](https://yaneuraou.yaneu.com/2015/12/29/%e5%b0%86%e6%a3%8b%e3%81%ae%e5%ae%8c%e5%85%a8%e3%83%8f%e3%83%83%e3%82%b7%e3%83%a5%e3%81%af%e4%bd%95bit%e3%81%a7%e8%a1%a8%e7%8f%be%e3%81%a7%e3%81%8d%e3%81%be%e3%81%99%e3%81%8b%ef%bc%9f/) (2015) — 理論的な下限（~236 ビット）の計算と状態空間の考察
[^yaneuraou-256bit]: やねうら王, ["将棋の局面を256bitに圧縮するには？"](https://yaneuraou.yaneu.com/2016/07/02/%E5%B0%86%E6%A3%8B%E3%81%AE%E5%B1%80%E9%9D%A2%E3%82%92256bit%E3%81%AB%E5%9C%A7%E7%B8%AE%E3%81%99%E3%82%8B%E3%81%AB%E3%81%AF%EF%BC%9F/) (2016) — 駒箱概念の導入と静的ハフマン符号化による 256 ビット固定圧縮の考案
- [nanoha mini](https://nanohamine.github.io/)：256 ビット圧縮の先駆的実装
- [ハフマン符号化](https://ja.wikipedia.org/wiki/%E3%83%8F%E3%83%95%E3%83%9E%E3%83%B3%E7%AC%A6%E5%8F%B7)：情報理論の理論的背景
