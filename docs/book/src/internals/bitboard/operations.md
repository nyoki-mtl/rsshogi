# 基本演算、幾何テーブル、SIMD 命令

`Bitboard` の基本演算は、128 ビット全体の集合演算（AND / OR / XOR / ANDNOT / 交差判定）と、立っているビットを 1 マスずつ取り出すビット走査（Bitscan）、および 2 マス間の幾何学的関係を引くルックアップテーブルの 3 要素から構成される。

---

## 128 ビット論理演算と SSE2 / SSE4.1 命令

`Bitboard` は `#[repr(C, align(16))]` で 16 バイト境界にアラインされているため、`x86_64` 環境（`not(miri)`）では SSE2 および SSE4.1 の 128 ビット SIMD 命令（`core::arch::x86_64`）を用いて、`p[0]` と `p[1]` の 2 レーンを 1 命令で同時処理する。
Miri 実行時や `const fn` 文脈、および非 x86_64 ターゲットでは、自動的に同じ意味を持つスカラー実装（`[u64; 2]` の要素ごと演算）へフォールバックする。

| `Bitboard` の操作 | x86_64 SIMD イントリンシック | 発行される機械語命令 | スカラー実装（`const fn` / フォールバック） |
| :--- | :--- | :--- | :--- |
| `self & rhs`（論理積） | `_mm_and_si128` | `pand` / `vpand` | `[p[0] & rhs.p[0], p[1] & rhs.p[1]]` |
| `self \| rhs`（論理和） | `_mm_or_si128` | `por` / `vpor` | `[p[0] \| rhs.p[0], p[1] \| rhs.p[1]]` |
| `self ^ rhs`（排他的論理和） | `_mm_xor_si128` | `pxor` / `vpxor` | `[p[0] ^ rhs.p[0], p[1] ^ rhs.p[1]]` |
| `self.andnot(rhs)`（差集合 `self & !rhs`） | `_mm_andnot_si128(rhs, self)` | `pandn` / `vpandn` | `[p[0] & !rhs.p[0], p[1] & !rhs.p[1]]` |
| `!self`（盤面内ビット反転） | `BOARD_MASK` との XOR | `pxor` / `vpxor` | `[!p[0] & BOARD_MASK_LOW, !p[1] & BOARD_MASK_HIGH]` |
| `self.intersects(rhs)`（交差判定） | `_mm_testz_si128(a, b) == 0` | `ptest` / `vptest` | `((p[0] & rhs.p[0]) \| (p[1] & rhs.p[1])) != 0` |
| `self == rhs`（等値比較） | `_mm_testz_si128(a ^ b, a ^ b) != 0` | `pxor` + `ptest` | `p[0] == rhs.p[0] && p[1] == rhs.p[1]` |

特に注目すべきは、差集合演算 `andnot` と SSE4.1 の `_mm_testz_si128`（`ptest` 命令）である。

- **`andnot` による命令統合**：移動先マスクから味方駒を除外する `attacks & !our_pieces` を素直に書くと、ビット反転（`!our_pieces`）とマスク処理の後に AND を行う 2〜3 命令が必要になる。`attacks.andnot(our_pieces)` は SSE2 の `_mm_andnot_si128`（第 1 引数をビット反転して第 2 引数と AND をとる 1 命令）に直接対応し、盤外ビットのマスク処理も不要となる。
- **`_mm_testz_si128` による 128 ビット一括ゼロ判定**：`intersects`（2 つのビットボードに共通のマスがあるか）や `PartialEq::eq`（2 つのビットボードが完全に等しいか）を 64 ビット汎用レジスタで行うと、`p[0]` と `p[1]` の比較結果を合成する必要がある。SSE4.1 が有効な環境では `_mm_testz_si128` を使うことで、128 ビットレジスタ全体の AND 結果がゼロかどうかを CPU の ZF（ゼロフラグ）へ直接反映させ、分岐予測ミスやレジスタ間転送を省いている。

---

## ビット走査（`lsb` / `pop_lsb`）と BMI1 命令

ビットボード内に立っているビット（`1`）の個数を数える `count()` と、最も小さいインデックスのマスを 1 つずつ取り出す `pop_lsb()` は、指し手生成ループの中核である。

### ポピュレーションカウント（`count`）

```rust
pub const fn count(self) -> u32 {
    self.p[0].count_ones() + self.p[1].count_ones()
}
```

x86_64 の `popcnt` 命令が有効な環境では、`count_ones()` は各 64 ビット要素に対する 1 命令のハードウェア `popcnt` にコンパイルされる。

### 最下位ビットの抽出と消去（`pop_lsb` / `Iterator`）

`Bitboard` は `Iterator<Item = Square>` を実装しており、`for sq in bb` と書くだけで内部的に `pop_lsb(&mut self)` が繰り返される。

```rust
pub fn pop_lsb(&mut self) -> Option<Square> {
    if self.p[0] != 0 {
        let idx = self.p[0].trailing_zeros() as i8;
        self.p[0] &= self.p[0] - 1;
        return Some(unsafe { Square::from_raw_unchecked(idx) });
    }
    if self.p[1] != 0 {
        let idx = self.p[1].trailing_zeros() as i8 + 63;
        self.p[1] &= self.p[1] - 1;
        return Some(unsafe { Square::from_raw_unchecked(idx) });
    }
    None
}
```

1. **1〜7 筋の走査（`self.p[0] != 0`）**：将棋盤の全体の約 78%（63 / 81 マス）は `p[0]` に含まれるため、まず `p[0]` を検査する。`trailing_zeros()`（`tzcnt` または `bsf` 命令）で最下位ビット位置 `idx` を取得し、`self.p[0] &= self.p[0] - 1`（BMI1 の `blsr` 命令にコンパイルされる）でその最下位ビットだけを `0` に消去する。
2. **8〜9 筋の走査（`self.p[1] != 0`）**：`p[0]` が空になったら `p[1]` を同様に走査し、ビット位置に `+ 63` を加算して `Square`（`63..=80`）を返す。

また、検査対象のマスが 1〜7 筋か 8〜9 筋かがあらかじめ分かっているホットパス向けに、片方の 64 ビット要素だけを走査する `for_each_low`（`p[0]` のみ）と `for_each_high`（`p[1]` のみ）も提供されている。

---

## 幾何学的ルックアップテーブル（`board/lookup.rs`）

2 つのマス間の直線関係（間にあるマス、貫通する直線、3 マスの同一直線判定）は、ピン（絶対ピン）検出、王手回避における合い駒生成、および 1 手詰めソルバの合法性判定で頻繁に利用される。
`crates/rsshogi/src/board/lookup.rs` では、これらをコンパイル時の `const` ブロックで `[[u128; 81]; 81]`（`packed_bits` 形式）として事前生成している。

### 1. `Bitboard::between(from, to)`（`BETWEEN` テーブル）

2 マス `from` と `to` が縦、横、斜めのいずれかの同一直線上に並んでいるとき、**両端（`from` と `to`）を除いた厳密にその間にあるマスの集合** を返す。
同一直線上にない場合や隣接している場合は `Bitboard::EMPTY` を返す。

- **王手回避での利用**：飛び駒（飛、角、香、馬、龍）による遠隔王手を受けた際、`Bitboard::between(checker_sq, king_sq)` がそのまま「合い駒（移動合いや打つ合い）によって王手を遮断できるマスの集合」となる。
- **ピン検出での利用**：敵の飛び駒と自玉の間の `between` と全体の占有ビットボード `occupied` の論理積をとり、そのビット数（`count()`）がちょうど `1` であれば、その 1 マスにある味方駒は絶対ピン（`blockers_for_king`）であると即座に判定できる。

### 2. `Bitboard::line(from, to)`（`LINE` テーブル）

2 マス `from` と `to` が縦、横、斜めの同一直線上に並んでいるとき、`from` と `to` を通り **盤端から盤端まで貫通する直線全体のマス集合** を返す。
同一直線上にない場合は `Bitboard::EMPTY` を返す。

- **ピンされた駒の移動可能判定**：絶対ピンされている味方駒（`from`）は、ピンしている敵駒と自玉を結ぶ直線（`Bitboard::line(from, king_sq)`）の上でしか移動できない。移動先候補ビットボードと `line(from, king_sq)` の AND をとるだけで、ピン方向から外れる非合法手を一括除外できる。

### 3. `Bitboard::is_aligned(from, to, king)`

3 つのマス `from`、`to`、`king` が縦、横、斜めの同一直線上にあり、かつ **`from` と `to` が `king` から見て同じ側（同じレイ方向）に存在するか** を判定する `const fn` である。

```rust
let same_line = from_df * to_dr == from_dr * to_df;
let same_side = (from_df * to_df + from_dr * to_dr) > 0;
```

単なる同一直線判定（外積 `from_df * to_dr == from_dr * to_df`）だけでなく、内積 `(from_df * to_df + from_dr * to_dr) > 0` を条件に含めることで、玉を挟んで反対側にあるマスを除外している。
この関数は `legality.rs` のピン整合性確認や、`mate.rs`（1 手詰めソルバ）における玉方の応手の高速なピン安全性フィルタ（`surely_legal_defender_move`）で直接呼び出される。
