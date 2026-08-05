# Zobrist Hashing

> **前提知識**: [ゲームステート管理](./state-management.md)（StateInfo と差分更新の設計パターン）

## このページの要点

- Zobrist hashing は「局面 → 64 ビット整数」の高速マッピングで、置換表と千日手検出の基盤
- XOR の**自己逆演算**性質により、駒の追加と削除が同じ演算で完了する
- rsshogi は盤上の駒も持ち駒も **XOR 一本**で合成する。持ち駒は枚数を添字にしたテーブルを引く
- 局面あたり `board_key`（盤上 + 手番）と `key`（持ち駒込み）の 2 本を維持する
- 衝突確率は Birthday paradox により **n 局面で約 n²/2⁶⁵**。実用上は 64 ビットで十分

## 歴史的背景

1970 年、Albert Zobrist はウィスコンシン大学の博士論文 "A New Hashing Method with Application for Game Playing" で、盤面の各要素にランダムなビット列を割り当て XOR で合成する手法を提案しました。

この手法が革命的だったのは、局面全体を再計算せずに**差分更新**できる点です。駒が 1 つ動くだけなら、変化した要素の XOR を 2-3 回適用するだけでハッシュ値が更新されます。これにより、毎秒数千万局面を処理する現代のエンジンでもハッシュ計算がボトルネックになりません。

GPS 将棋は持ち駒のハッシュに加算方式を導入し、参照実装がこれを発展させた乗算方式を確立しました。rsshogi も当初はこの系譜を継承していましたが、現在は持ち駒も XOR で合成する方式に移行しています（理由は次節）。置換表での局面の同一性判定や千日手検出は、いずれもこのハッシュを基盤としています。

## 基本原理

Zobrist hashing は、盤面の各要素（駒の配置、手番、持ち駒など）に対してランダムな64ビット値を事前に割り当て、それらをXOR演算で組み合わせることで局面全体のハッシュ値を生成する手法です。
この手法は「tabulation hashing」の一種であり、XOR演算が自己逆演算（self-inverse）であることを活用して、差分更新を高速に行えることが特徴です。

### 持ち駒をどう合成するか：XOR と加算

盤上の駒は XOR で合成します。マス上に駒が「ある / ない」の 2 状態しかないため、
自己逆演算の XOR がそのまま追加と削除の両方になります。ここに議論の余地はありません。

問題は持ち駒です。持ち駒は 0 枚から最大 18 枚までの多値状態なので、
「ある / ない」の 2 状態には収まりません。取りうる実装は 2 つあります。

#### 加算方式（乗算ベース）

駒種ごとに 1 枚分の基本値を持ち、枚数を掛けた値を加減算します。
GPS 将棋が導入し、参照実装がこれを発展させました。

```rust,ignore
// 加算方式（rsshogi では廃止済み）
hand_hash.add(base.mul_u64(1)); // 持ち駒を 1 枚増やす
hand_hash.sub(base.mul_u64(1)); // 持ち駒を 1 枚減らす
```

テーブルは「色 × 駒種」の 2 次元で済み、128 バイトに収まります。

#### count 添字 XOR 方式

枚数ごとに独立したキーを持ち、変化前後のキーを両方 XOR します。
Stockfish が material key で同型の問題をこの方式で解いています。

```rust,ignore
// count 添字 XOR 方式（現在の rsshogi）
key ^= Zobrist::hand_delta(color, piece_type, before, after);
// hand_delta(c, pt, n, m) == hand(c, pt, n) ^ hand(c, pt, m)
```

テーブルは「色 × (駒種 + 枚数)」の 2 次元で、1152 バイトになります。

#### rsshogi の選択

rsshogi は **count 添字 XOR 方式**を採用しています。根拠は 3 点です。

1. **キー型の代数が縮退する**。`ZobristKey` に `add` / `sub` / `mul_u64` が
   public に生えていた唯一の理由が加算方式であり、XOR 一本に潰せます。
   公開型に `wrapping_mul` が生えている状態は、誤用を誘う設計です。
2. **do と undo が対称になる**。加算方式では駒打ちで `sub`、捕獲で `add` という
   符号の非対称があり、取り違えやすい形でした。XOR なら両者が同一の演算です。
3. **128 ビット構成が素直になる**。加算方式の 128 ビット実装は limb 間の
   桁上がりを伝播しないため、実体は「独立な 64 ビット加算ハッシュ 2 本」という
   奇妙な構成でした。XOR なら独立した 2 レーンであることが定義から明らかです。

テーブルが 128 バイトから 1152 バイトに増えますが、
盤上駒のテーブルが 20 KB あり L1 traffic を支配するため、この差は誤差の範囲です。

なお「`base * count` は下位ビットが枚数の情報しか持たない」という
エントロピー論は根拠として採用していません。検証の結果、
衝突確率の集計値に有意な差は生じないと判断したためです。

### ハッシュ値の構成要素

将棋の局面ハッシュは以下の要素から構成されます。

1. **盤上の駒**: 各マス × 各駒種 × 先手/後手
2. **持ち駒**: 各駒種 × 枚数 × 先手/後手
3. **手番**: 先手番か後手番か

### 2 本のキー

rsshogi は局面あたり 2 本のキーを維持します。

| キー | 被覆範囲 | 用途 |
|---|---|---|
| `board_key` | 盤上の駒 + 手番 | 千日手・優等/劣等局面判定の等価性フィルタ |
| `key` | `board_key` + 持ち駒 | 置換表、book、局面の同一性判定全般 |

分けている理由は**優等局面判定が半順序の比較だから**です。
「同じ盤面で、自分の持ち駒が相手より多いか等しい」という判定は、
いかなる単一ハッシュでも表現できません。したがって

1. 盤面のみの等価性フィルタ（`board_key` の比較）
2. 生の `Hand` 値の比較（`Hand::is_equal_or_superior`）

の両方が必要になります。`board_key` の被覆範囲は千日手判定の契約であり、変更しません。

キーの真実点は `Position` ではなく現局面の `StateHot` にあります。
`Position::key()` / `board_key()` は state stack を経由して読み出します。
これにより `undo_move32()` はスタックの添字を戻すだけで済み、
キーのロードもストアも発生しません。

## rsshogi における実装

### データ構造

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/board/zobrist.rs:zobrist_table_struct}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/board/zobrist.rs#L130-L147)</small>

持ち駒テーブルの slot は「駒種 + 枚数」を連結した ragged layout です。
各駒種が `0..=表現可能な最大枚数` を連続で占め、歩 32・香桂銀金 各 8・角飛 各 4 で計 72 slot になります。

上限を**物理的な最大枚数**（歩 18 枚など）ではなく **`Hand` のビットフィールド幅が
表現できる枚数**に取っているのは、SFEN パーサが物理上限を超える持ち駒数を受理し、
その検出を `Position::validate()` に委ねる設計だからです。
局面の構築時（検証より前）にキーの全再計算が走るため、
キーは表現可能な全枚数に対して定義されている必要があります。

テーブル形状は `Hand::count_mask()` から `const` 導出しているため、
`Hand` のビットレイアウトを変えるとテーブルが自動追随します。

`ZobristKey` は通常 `u64` のラッパーです（`hash-128` feature で 128bit に拡張可能）。

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/board/zobrist.rs:zobrist_key_struct}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/board/zobrist.rs#L30-L35)</small>

キーが持つ演算は XOR だけです。`BitXor` / `BitXorAssign` に一本化されており、
加算・減算・乗算は提供しません。

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/board/zobrist.rs:zobrist_key_ops}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/board/zobrist.rs#L97-L128)</small>

### ランダム値の生成

Zobrist テーブルの値は、固定シード付きの Xorshift64*（xorshift64 + 乗算混合）で **コンパイル時の `const fn`** として生成し、`static` 変数に格納されます。
シードはやねうら王と同じ `20_151_225`（開発開始日 2015/12/25 由来）を使用していますが、**キー値には互換性がありません**（詳細は「落とし穴」節）。

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/board/zobrist.rs:zobrist_init}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/board/zobrist.rs#L237-L299)</small>

`next_key` はビット幅によらず PRNG をちょうど 4 回消費し、`low` を常に 1 draw 目とします。
この空回しにより、64 ビットビルドのキーは 128 ビットビルドの low limb と一致します。

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

    /// 持ち駒を count 枚保有している状態のハッシュ値を取得
    pub fn hand(color: Color, piece_type: PieceType, count: u32) -> ZobristKey {
        let piece_idx = piece_type.to_index();
        Self::instance().hand[color.to_index()][hand_slot(piece_idx, count)]
    }

    /// 持ち駒の枚数が from から to へ変化したときの差分を取得
    pub fn hand_delta(color: Color, piece_type: PieceType, from: u32, to: u32) -> ZobristKey {
        let piece_idx = piece_type.to_index();
        let row = &Self::instance().hand[color.to_index()];
        row[hand_slot(piece_idx, from)] ^ row[hand_slot(piece_idx, to)]
    }

    /// 手番のハッシュ値を取得
    pub fn side() -> ZobristKey {
        Self::instance().side()
    }
}
```

`hand_slot` は枚数をフィールド幅でマスクするため、`u32` の全域で定義されます。
また、持ち駒にならない駒種は offset 0 / mask 0 で slot 0（歩 0 枚 = ゼロキー）に落ちるため、
どちらの関数にも分岐がありません。

## 差分更新の最適化

Zobrist hashing の大きな利点は、局面が変化した際に差分更新が可能なことです。

盤上の変化と手番は `board_key` と `key` の**両方**に効きます。
持ち駒の変化は `key` にしか効きません。

### 駒の移動

```rust,ignore
// 移動元から除去し、移動先に配置する（new_piece は成り判定済みの駒）
let moved = Zobrist::psq(from, moved_piece) ^ Zobrist::psq(to, new_piece);
board_key ^= moved;
key ^= moved;

// 捕獲があれば盤上から取り除き、持ち駒を 1 枚増やす
if let Some(captured) = captured_piece {
    let removed = Zobrist::psq(to, captured);
    board_key ^= removed;
    key ^= removed;
    key ^= Zobrist::hand_delta(us, hand_piece_type, count_before, count_before + 1);
}

// 手番の切り替え
let side = Zobrist::side();
board_key ^= side;
key ^= side;
```

### 駒打ち

```rust,ignore
// 盤上に配置する
let placed = Zobrist::psq(to, Piece::from_parts(color, dropped_piece));
board_key ^= placed;
key ^= placed;

// 持ち駒を 1 枚減らす
key ^= Zobrist::hand_delta(color, dropped_piece, count_before, count_before - 1);

// 手番の切り替え
let side = Zobrist::side();
board_key ^= side;
key ^= side;
```

駒打ちと捕獲で `hand_delta` の呼び方が同一である点に注目してください。
加算方式では前者が `sub`、後者が `add` という非対称になっていました。

### 巻き戻し

`undo_move32()` はキーを一切触りません。各局面のキーは state stack の
対応するエントリに残っているため、スタックの添字を戻すだけで復元されます。

### 1 手先読み

局面を変更せずに「この手を指した後のキー」を求められます。
探索エンジンが置換表を prefetch する用途を想定した API です。

```rust,ignore
let next_key = pos.key_after(mv); // 局面は変わらない
// ここで engine 側が置換表を prefetch する
pos.apply_move32(mv);
assert_eq!(pos.key(), next_key);
```

prefetch を発行する API 自体はライブラリに置いていません。
置換表は engine の所有物であり、rsshogi は prefetch すべきアドレスを知り得ないためです。
キーを返すところが責務の境界になります。

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

rsshogi との違いは 2 点です。

- **持ち駒テーブルの意味**。参照実装の `hand[COLOR_NB][PIECE_HAND_NB]` は
  「1 枚増えるごとに加算する値」ですが、rsshogi のテーブルは
  「ちょうど n 枚保有している状態の値」であり、枚数が添字に入ります。
- **bit 0 の扱い**。参照実装は `side = 1` と `& ~1ULL` でキーの bit 0 に手番を埋め込みますが、
  rsshogi は行いません（「落とし穴」節を参照）。

PRNG（Xorshift64\*）とシード `20151225` は共通なので生成の枠組みは同じですが、
上記の違いによりキー値は一致しません。

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
// 実際の計算は apply_move32() 内の差分更新（XOR）で行われる。
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
        for slot in 0..ZobristTable::HAND_SLOT_COUNT {
            let hash = ZOBRIST.hand_at_index(color, slot);
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

枚数 0 の slot は意図的にゼロにしてあります。
空の持ち駒が合成キーに寄与しないため、全再計算が枚数 0 の駒種を読み飛ばせるからです。

一方でこれは、持ち駒のハッシュを合成し忘れても結果が変わらないケースを作ります。
テーブルや更新サイトを触ったら、次の 2 つを必ず通してください。

- `zobrist.rs` の `tests` モジュール: テーブル自体の不変条件
  （枚数 0 の slot がゼロであること、枚数ごとのキーが一意であること）
- `tests/property_zobrist_key.rs`: 「合成キー == 盤面キー ^ 持ち駒寄与の全再計算」を
  複数局面の全合法手について固定する

### 枚数の折り返し

`Zobrist::hand()` は枚数を `Hand` のフィールド幅でマスクします。
歩は 5 ビットなので 32 枚は 0 枚と同じ slot に落ちます。
これは意図した動作で、`Hand::add` / `Hand::sub` がフィールド幅を超えたときの
挙動と一致させてあります。物理的にありえない枚数の局面（`Position::validate()` が
弾く局面）でも、差分更新と全再計算が食い違わないことを保証するためです。

### 参照実装とはキー値が一致しない

生成 scheme（固定シードの Xorshift64\*、seed 20151225）は参照実装と同じですが、
**キー値には互換性がありません**。

参照実装では、手番の Zobrist 値として `side = 1`（bit 0 のみ）を使用し、
他のすべてのランダム値で bit 0 を 0 にマスクします（`& ~1ULL`）。
これにより、ハッシュ値の bit 0 を見るだけで手番を判定できます。

rsshogi はこの bit 0 マスキングを採用していません。
キーだけから手番を読む必要のある消費者が存在しない一方で、
誰も読まないビットのために全キーの実効エントロピーを 63 ビットに落とし、
衝突確率を恒久的に 2 倍にする取引は、10⁹–10¹⁰ 局面規模の book ビルドで最も損をするためです。
加えて bit 0 の不変条件は、シリアライズ済みの book キーに構造を永久に焼き付けます。

手番の判定には `side_to_move()` を直接参照してください。
参照実装の置換表や book とキーを突き合わせることはできません。

### 衝突の沈黙する性質

ハッシュ衝突が起きても、プログラムはクラッシュせず誤った結果を静かに返します。
置換表から取得した指し手が現局面では不正な手である場合や、千日手検出で偽陽性が生じる場合があります。
`pos.validate()` / `pos.validate_all()` による定期的な検証が重要です。

## まとめ

- Zobrist hashing は「局面 → 64 ビット整数」の O(1) マッピングで、エンジンの基盤技術
- XOR の自己逆演算性質により、差分更新が 2-3 回の演算で完了
- rsshogi は盤上も持ち駒も XOR で合成する。持ち駒は枚数を添字にしたテーブルを引く
- 局面あたり `board_key` と `key` の 2 本を維持する。分割を強制しているのは優等局面判定の半順序性
- キーの真実点は `StateHot` にあり、`undo_move32()` はキーを触らない
- 64 ビットハッシュの衝突確率は実用上十分に低い（Birthday paradox で ~40 億局面まで安全）
- 衝突は沈黙するため、`pos.validate()` / `pos.validate_all()` による検証テストが不可欠

## 次に読む

→ **[合法手生成](../movegen/index.md)**: Position の問い合わせ API を活用した指し手生成の仕組みに進みます。

## 参考資料

- [Chess Programming Wiki - Zobrist Hashing](https://www.chessprogramming.org/Zobrist_Hashing) - Zobrist hashingの理論と歴史
- [やねうら王の置換表詳細解説](https://yaneuraou.yaneu.com/2018/11/18/transposition-table-details/) - 置換表の実装と最適化
- [Rustic Chess - Zobrist Hashing](https://rustic-chess.org/board_representation/zobrist_hashing.html) - Rustでの実装例
- Albert L. Zobrist (1970). "A New Hashing Method with Application for Game Playing" - 原論文
