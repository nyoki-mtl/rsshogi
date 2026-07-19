# Zobrist Hashing

> **前提知識**: [ゲームステート管理](./state-management.md)（StateInfo と差分更新の設計パターン）

## このページの要点

- Zobrist hashing は「局面 → 64 ビット整数」の高速マッピングで、置換表と千日手検出の基盤
- XOR の**自己逆演算**性質により、駒の追加と削除が同じ演算で完了する
- rsshogi は**ハイブリッド方式**を採用: 盤上の駒に XOR、持ち駒に乗算ベース
- 衝突確率は Birthday paradox により **n 局面で約 n²/2⁶⁵**。実用上は 64 ビットで十分

## 歴史的背景

1970 年、Albert Zobrist はウィスコンシン大学の博士論文 "A New Hashing Method with Application for Game Playing" で、盤面の各要素にランダムなビット列を割り当て XOR で合成する手法を提案しました。

この手法が革命的だったのは、局面全体を再計算せずに**差分更新**できる点です。駒が 1 つ動くだけなら、変化した要素の XOR を 2-3 回適用するだけでハッシュ値が更新されます。これにより、毎秒数千万局面を処理する現代のエンジンでもハッシュ計算がボトルネックになりません。

GPS 将棋は持ち駒のハッシュに加算方式を導入し、参照実装がこれを発展させた乗算方式を確立しました。rsshogi もこの系譜を継承しています。置換表での局面の同一性判定や千日手検出は、いずれもこのハッシュを基盤としています。

## 基本原理

Zobrist hashing は、盤面の各要素（駒の配置、手番、持ち駒など）に対してランダムな64ビット値を事前に割り当て、それらをXOR演算で組み合わせることで局面全体のハッシュ値を生成する手法です。
この手法は「tabulation hashing」の一種であり、XOR演算が自己逆演算（self-inverse）であることを活用して、差分更新を高速に行えることが特徴です。

### XOR vs 加算：どちらを使うべきか

Zobrist hashing の実装では、ハッシュ値の組み合わせに **XOR（排他的論理和）** または **加算** を使用できます。
歴史的には、Zobrist の原論文（1970年）では XOR が使用されていましたが、GPS将棋が加算方式を導入し、やねうら王もこれに倣っています。

#### XOR方式の特徴

```rust,ignore
// XOR方式
hash ^= Zobrist::psq(sq, piece);  // 駒を追加
hash ^= Zobrist::psq(sq, piece);  // 同じ操作で駒を削除（自己逆演算）
```

**利点:**
- 自己逆演算のため、追加と削除が同じ操作
- 数学的に美しく、理論的に明確

**欠点:**
- 持ち駒の枚数変化で複雑な処理が必要（3D配列が必要）

#### 加算方式の特徴

```rust,ignore
// 加算方式
hash += Zobrist::hand_add(color, piece_type);    // 持ち駒を1枚増やす
hash -= Zobrist::hand_remove(color, piece_type); // 持ち駒を1枚減らす
```

**利点:**
- 持ち駒の増減が直感的（add/subtractで表現できる）
- 2D配列で実装可能（メモリ効率が良い）

**欠点:**
- 追加と削除が異なる操作
- オーバーフローに注意が必要（64ビットなら実用上問題ない）

#### rsshogi の選択

rsshogi では、やねうら王互換の**ハイブリッド方式**を採用しています。

- **盤上の駒**: XOR 方式（追加・削除が自己逆演算）
- **持ち駒**: 乗算方式（基本ハッシュ値 × 枚数で算出）

持ち駒のハッシュは、駒種ごとに1枚分の基本値を保持し、`base.mul_u64(count)` で計算します。
これにより、3D 配列を持つ必要がなく 2D 配列で実装できます。
差分更新時は `add` / `sub` メソッドで加減算します。

### ハッシュ値の構成要素

将棋の局面ハッシュは以下の要素から構成されます。

1. **盤上の駒**: 各マス × 各駒種 × 先手/後手
2. **持ち駒**: 各駒種 × 枚数 × 先手/後手
3. **手番**: 先手番か後手番か

## rsshogi における実装

rsshogi では、やねうら王互換の Zobrist hashing を実装しています。

### データ構造

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/board/zobrist.rs:zobrist_table_struct}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/board/zobrist.rs#L146-L160)</small>

`ZobristKey` は通常 `u64` のラッパーです（`hash-128` feature で 128bit に拡張可能）。

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/board/zobrist.rs:zobrist_key_struct}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/board/zobrist.rs#L6-L11)</small>

キー操作として `xor` / `add` / `sub` メソッドを提供し、差分更新を効率的に行います。

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/board/zobrist.rs:zobrist_key_ops}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/board/zobrist.rs#L35-L50)</small>

### ランダム値の生成

Zobrist テーブルの値は、固定シード付きの Xorshift64*（xorshift64 + 乗算混合）で **コンパイル時の `const fn`** として生成し、`static` 変数に格納されます。
シードはやねうら王と同じ `20_151_225`（開発開始日 2015/12/25 由来）を使用しています。

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/board/zobrist.rs:zobrist_init}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/board/zobrist.rs#L191-L238)</small>

### ハッシュ値の計算

`Zobrist` 構造体がシングルトンアクセスのラッパーとして機能します。

```rust,ignore
// crates/rsshogi/src/board/zobrist.rs
impl Zobrist {
    /// 盤上の駒のハッシュ値を取得
    pub fn psq(sq: Square, piece: Piece) -> ZobristKey {
        let table = Self::instance();
        table.board[piece.to_index()][sq.to_index_with_none()]
    }

    /// 持ち駒のハッシュ値を取得（基本値 × 枚数）
    pub fn hand(color: Color, piece_type: PieceType, count: usize) -> ZobristKey {
        let table = Self::instance();
        let base = table.hand[color.to_index()][piece_type.to_index()];
        base.mul_u64(count as u64)
    }

    /// 手番のハッシュ値を取得
    pub fn side() -> ZobristKey {
        Self::instance().side()
    }
}
```

## 差分更新の最適化

Zobrist hashing の大きな利点は、局面が変化した際に差分更新が可能なことです。

### 駒の移動

```rust,ignore
// 移動元の駒を除去
hash ^= Zobrist::psq(from, moved_piece);

// 捕獲があれば盤上から取り除く
if let Some(captured) = captured_piece {
    hash ^= Zobrist::psq(to, captured);
    // 持ち駒ハッシュは加算方式で更新（乗算ベースのため）
    hand_hash.add(Zobrist::hand(color, captured.piece_type().demote(), 1));
}

// 移動先に駒を配置（promoted_piece は成り判定済みの駒）
hash ^= Zobrist::psq(to, promoted_piece);

// 手番の切り替え
hash ^= Zobrist::side();
```

### 駒打ち

```rust,ignore
// 持ち駒ハッシュを減算（乗算ベースのため sub で更新）
hand_hash.sub(Zobrist::hand(color, dropped_piece, 1));

// 盤上に配置
hash ^= Zobrist::psq(to, Piece::from_parts(color, dropped_piece));

// 手番の切り替え
hash ^= Zobrist::side();
```

## 衝突確率と安全性

64ビットのハッシュ値を使用する場合、衝突確率は Birthday paradox に基づいて評価できます。

n 個の局面を格納したとき、少なくとも 1 組の衝突が起きる確率は近似的に：

```text
P(衝突) ≈ 1 - e^(-n² / (2 × 2^k))
  ここで k はハッシュのビット幅
```

具体的な衝突確率 50% に達する局面数の目安：

| ビット幅 | 衝突 50% の局面数 | 将棋探索での位置づけ |
|---------|-----------------|-------------------|
| 32 bit | ~65,000 | 一局の探索で容易に衝突 |
| 64 bit | ~40 億 | 長時間探索でもほぼ安全 |
| 128 bit | ~1.8 × 10¹⁹ | 理論上衝突しない |

現代のチェス・将棋プログラムでは64ビットハッシュが標準です。
実用上は、置換表でのハッシュ衝突検出のため、ハッシュ値の上位32ビットを検証値として保存することが一般的です。

## 参照実装の実装との比較

参照実装では、Zobrist hashing が以下のように実装されています（[`source/position.cpp:26-35`](https://github.com/yaneurao/YaneuraOu/blob/eb2856f9/source/position.cpp#L26-L35)）。

```cpp
namespace Zobrist {
    HASH_KEY zero;                          // ゼロ(==0)
    HASH_KEY side;                          // 手番(==1)
    HASH_KEY psq[SQ_NB_PLUS1][PIECE_NB];   // 駒pcが盤上sqに配置されているときのZobrist Key
    HASH_KEY hand[COLOR_NB][PIECE_HAND_NB]; // c側の手駒prが一枚増えるごとにこれを加算するZobristKey
    HASH_KEY depth[MAX_PLY];                // 深さも考慮に入れたHASH KEYを作りたいときに用いる(実験用)
}
```

初期化処理では、PRNGを使って各値を生成しています（[`source/position.cpp:92-121`](https://github.com/yaneurao/YaneuraOu/blob/eb2856f9/source/position.cpp#L92-L121)）。

```cpp
void Position::init() {
    PRNG rng(20151225); // 開発開始日

    // 手番としてbit0を用いる。それ以外はbit0を使わない
    SET_HASH(Zobrist::side, 1, 0, 0, 0);

    // pc==NO_PIECEのときは0であることを保証
    for (auto pc : Piece())
        for (auto sq : SQ)
            if (pc)
                SET_HASH(Zobrist::psq[sq][pc], rng.rand<Key>() & ~1ULL, ...);
}
```

## パフォーマンス考慮事項

### メモリアクセスパターン

Zobrist テーブルは頻繁にアクセスされるため、キャッシュ効率を考慮した配置が重要である。
現状の実装（`crates/rsshogi/src/board/zobrist.rs`）は標準配列レイアウトを採用しているが、`#[repr(align(64))]` の導入や SOA 形式への再配置など追加の最適化余地がある。

### SIMD 最適化の可能性

複数の局面を並列処理する場合、SIMD 命令を使用して複数のハッシュ値を同時に計算できます。
現在の rsshogi 実装では Zobrist ハッシュ計算に `std::simd` は使用していません（nightly 専用のため）。
以下は概念的な擬似コードです（実 API ではありません）。

```rust,ignore
// ※ 概念コード。rsshogi の実 API ではない。
// 実際の計算は apply_move32() 内の差分更新（XOR / add / sub）で行われる。
fn compute_hashes_parallel_concept(hashes: &mut [u64; 4]) {
    // 4局面分の差分 XOR を並列適用するイメージ
}
```

## 応用例

### 置換表（Transposition Table）での使用

置換表は、探索済みの局面情報をキャッシュすることで、同一局面の再探索を避ける重要な最適化手法です。
Zobrist hashをキーとして使用し、通常3エントリをクラスタとして管理します。

```rust,ignore
pub struct TranspositionTable {
    entries: Vec<TTEntry>,
    size_mask: usize,
}

impl TranspositionTable {
    pub fn probe(&self, hash: u64) -> Option<&TTEntry> {
        let index = (hash as usize) & self.size_mask;
        let entry = &self.entries[index];

        // 上位32ビットで検証（衝突検出）
        if entry.hash_high == (hash >> 32) as u32 {
            Some(entry)
        } else {
            None
        }
    }
}
```

置換表の使用により、探索性能が20-70%向上することが報告されています（[やねうら王の置換表詳細解説](https://yaneuraou.yaneu.com/2018/11/18/transposition-table-details/)）。

### 千日手検出の実装

rsshogi では、互換のカウンタ方式で千日手を検出しています（`crates/rsshogi/src/board/position/rules.rs`）。

`apply_move32()` の中で `StateInfo` の `repetition_counter` を更新し、
同一局面の出現回数を記録します。判定時はカウンタの値を閾値と比較するだけなので O(1) です。

```rust,ignore
pub fn is_repetition(&self, threshold: u8) -> bool {
    self.repetition_counter() >= i32::from(threshold)
}
```

`repetition_counter` は `apply_move32()` 中に `board_key`（Zobrist ハッシュ）と持ち駒の一致を
過去の局面と照合して計算されます。連続王手の千日手（`continuous_check`）や
優等/劣等局面の判定には `repetition_state()` / `repetition_state_with_ply()` を使用します。

## デバッグとテスト

Zobrist hashing の正しさを検証するためのテスト手法：

### rsshogi のテストスイート

rsshogi では包括的なテストを実装しています（`crates/rsshogi/src/board/zobrist.rs` の `tests` モジュール）。

```rust,ignore
#[test]
fn zobrist_table_uniqueness() {
    let mut hashes = HashSet::new();
    hashes.insert(ZOBRIST.side());
    hashes.insert(ZOBRIST.no_pawns());

    for piece in 0..Piece::COUNT {
        for square in 0..Square::COUNT {
            let hash = ZOBRIST.board_at_index(piece, square);
            if hash != ZobristKey::default() {
                assert!(hashes.insert(hash), "Duplicate hash found");
            }
        }
    }

    for color in 0..Color::COUNT {
        for piece in 1..PieceType::HAND_TABLE_SIZE {
            let hash = ZOBRIST.hand_at_index(color, piece);
            if hash != ZobristKey::default() {
                assert!(hashes.insert(hash), "Duplicate hash found");
            }
        }
    }
}

#[test]
fn zobrist_distribution() {
    let mut bit_count = [0u32; 64];
    for piece in 1..Piece::COUNT {
        for square in 0..Square::COUNT {
            let hash = ZOBRIST.board_at_index(piece, square).low_u64();
            for (bit, count) in bit_count.iter_mut().enumerate() {
                if (hash >> bit) & 1 == 1 { *count += 1; }
            }
        }
    }
    // 各ビットが約50%の確率で1になることを確認
    let total: u32 = (Piece::COUNT as u32 - 1) * Square::COUNT as u32;
    for (bit, count) in bit_count.iter().enumerate() {
        let ratio = f64::from(*count) / f64::from(total);
        assert!((0.3..0.7).contains(&ratio),
            "Bit {bit} has unusual distribution: {ratio:.2}");
    }
}
```

## 落とし穴

### 持ち駒 0 枚のハッシュ値が 0

`base.mul_u64(0)` は常に 0 を返します。これは正しい動作ですが、
初期化時に持ち駒のハッシュを加算し忘れても結果が 0 のため検出しにくいバグになります。
zobrist.rs の `tests` モジュールで包括的なテストを行い、整合性を検証してください。

### 手番ビットの bit 0 問題

参照実装では、手番の Zobrist 値として `side = 1`（bit 0 のみ）を使用し、
他のすべてのランダム値で bit 0 を 0 にマスクします（`& ~1ULL`）。
これにより、ハッシュ値の bit 0 を見るだけで手番を判定できます。

rsshogi はこの bit 0 マスキングを採用しておらず、手番の判定には `side_to_move()` を直接参照します。
もし rsshogi のハッシュ値を 参照実装の置換表と共有する場合は、
bit 0 マスキングの有無に注意してください。

### 衝突の沈黙する性質

ハッシュ衝突が起きても、プログラムはクラッシュせず誤った結果を静かに返します。
置換表から取得した指し手が現局面では不正な手である場合や、千日手検出で偽陽性が生じる場合があります。
`pos.validate()` / `pos.validate_all()` による定期的な検証が重要です。

## まとめ

- Zobrist hashing は「局面 → 64 ビット整数」の O(1) マッピングで、エンジンの基盤技術
- XOR の自己逆演算性質により、差分更新が 2-3 回の演算で完了
- rsshogi のハイブリッド方式（盤上 XOR + 持ち駒 乗算）は 互換
- 64 ビットハッシュの衝突確率は実用上十分に低い（Birthday paradox で ~40 億局面まで安全）
- 衝突は沈黙するため、`pos.validate()` / `pos.validate_all()` による検証テストが不可欠

## 次に読む

→ **[合法手生成](../movegen/index.md)**: Position の問い合わせ API を活用した指し手生成の仕組みに進みます。

## 参考資料

- [Chess Programming Wiki - Zobrist Hashing](https://www.chessprogramming.org/Zobrist_Hashing) - Zobrist hashingの理論と歴史
- [やねうら王の置換表詳細解説](https://yaneuraou.yaneu.com/2018/11/18/transposition-table-details/) - 置換表の実装と最適化
- [Rustic Chess - Zobrist Hashing](https://rustic-chess.org/board_representation/zobrist_hashing.html) - Rustでの実装例
- Albert L. Zobrist (1970). "A New Hashing Method with Application for Game Playing" - 原論文
