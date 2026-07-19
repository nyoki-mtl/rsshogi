# 拡張命令リファレンス

> **前提知識**: [SIMD 概論](index.md)、[拡張命令の歴史](history.md)

## このページの要点

- SSE2 の 128 ビット論理演算（`por`, `pand`, `pxor`, `pandn`）はビットボードの基礎命令
- SSSE3 の `pshufb`（バイトシャッフル）はバイト逆順操作に使い、飛び利き計算で活躍する
- POPCNT と TZCNT/LZCNT はビットスキャンの中核であり、指し手列挙で毎回呼ばれる
- PEXT/PDEP（BMI2）は Magic Bitboard の代替となる強力な命令だが CPU 互換性に注意が必要
- AVX2 は 256 ビット整数演算を提供し、複数ビットボードの一括処理や NNUE 推論で使われる

## 命令一覧

本ページで解説する命令を用途別に分類します。

| 用途 | 命令 | 命令セット | 主な使用場面 |
|------|------|-----------|------------|
| 論理演算 | `por`, `pand`, `pxor`, `pandn` | SSE2 | ビットボード合成・マスク |
| シフト | `psllq`, `psrlq` | SSE2 | 歩の利き計算、ビット移動 |
| 比較 | `pcmpeqd` | SSE2 | ゼロ判定 |
| テスト | `ptest` | SSE4.1 | ビットボードの交差判定 |
| シャッフル | `pshufb` | SSSE3 | バイト逆順（byte_reverse） |
| ビットカウント | `popcnt` | POPCNT | 駒数カウント、手数見積もり |
| ビットスキャン | `tzcnt` | BMI1 | LSB 検出（指し手列挙） |
| ビットスキャン | `lzcnt` | BMI1 | MSB 検出 |
| ビット抽出 | `pext` | BMI2 | 飛び利きの直接計算 |
| ビット挿入 | `pdep` | BMI2 | PEXT の逆操作 |
| 256bit 論理演算 | `vpor`, `vpand` 等 | AVX2 | 複数ビットボードの一括処理 |
| 512bit 演算 | `vporq` 等 | AVX-512 | NNUE 推論の高速化 |

## SSE2: 128 ビット整数論理演算

SSE2 はビットボード演算の最も基本的な命令セットです。
x86-64 の必須仕様であり、すべての 64 ビット x86 CPU で利用可能です。

### 論理演算

128 ビット幅の論理演算は、ビットボードの合成・マスク適用・差分更新で頻繁に使われます。

| intrinsic | アセンブリ | 動作 | 用途例 |
|-----------|-----------|------|--------|
| `_mm_or_si128(a, b)` | `por` | a OR b | 先手・後手のビットボード合成 |
| `_mm_and_si128(a, b)` | `pand` | a AND b | マスク適用（特定の段・筋の抽出） |
| `_mm_xor_si128(a, b)` | `pxor` | a XOR b | 差分更新（駒の移動） |
| `_mm_andnot_si128(a, b)` | `pandn` | (NOT a) AND b | マスクの除外 |

**注意**: `_mm_andnot_si128(a, b)` は `(NOT a) AND b` であり、`a AND (NOT b)` **ではありません**。
引数の順序に注意が必要です。

OR:  駒の配置を合成
  先手駒 | 後手駒 = 全駒

AND: 特定範囲の抽出
  全駒 & 段マスク = その段にいる駒

```text
XOR: 差分更新（駒の移動）
  盤面 ^ (移動元 | 移動先) = 移動後の盤面
```

### シフト演算

SSE2 のシフト命令は **64 ビット要素単位**で動作します。
128 ビット全体としてのシフトではない点に注意が必要です。

| intrinsic | アセンブリ | 動作 |
|-----------|-----------|------|
| `_mm_slli_epi64(a, n)` | `psllq` | 各 64bit 要素を左に n ビットシフト |
| `_mm_srli_epi64(a, n)` | `psrlq` | 各 64bit 要素を右に n ビットシフト |
| `_mm_slli_si128(a, n)` | `pslldq` | 128bit 全体を左に n **バイト**シフト |
| `_mm_srli_si128(a, n)` | `psrldq` | 128bit 全体を右に n **バイト**シフト |

128 ビット全体のビット単位シフトを実現するには、要素間の桁上げを手動で処理する必要があります。

128bit 左シフト（n < 64 の場合）:
  hi' = (hi << n) | (lo >> (64 - n))   ← lo からの桁上げ
  lo' = lo << n

```text
128bit 左シフト（64 ≤ n < 128 の場合）:
  hi' = lo << (n - 64)
  lo' = 0
```

歩の利き計算では「全体を 1 マス分シフト」する操作が頻出しますが、ビットボードのレイアウトによっては筋方向のシフトが 64 ビット要素をまたぐため、この桁上げ処理が必要になります。

### 比較・ゼロ判定

| intrinsic | アセンブリ | 動作 |
|-----------|-----------|------|
| `_mm_cmpeq_epi64(a, b)` | `pcmpeqq`[^pcmpeqq] | 64bit 要素ごとの等値比較 |
| `_mm_movemask_epi8(a)` | `pmovmskb` | 各バイトの最上位ビットを 16bit マスクに集約 |

ビットボードがゼロかどうかの判定は、`_mm_cmpeq_epi64` でゼロレジスタと比較し、`_mm_movemask_epi8` で結果を集約する方法が SSE2 の範囲で可能です。
ただし SSE4.1 の `_mm_testz_si128` を使えばより効率的です。

## SSSE3: バイトシャッフル

SSSE3 で追加された `_mm_shuffle_epi8` は、128 ビットレジスタ内のバイトを任意の順序に並べ替える強力な命令です。

| intrinsic | アセンブリ | 動作 |
|-----------|-----------|------|
| `_mm_shuffle_epi8(a, mask)` | `pshufb` | mask に従ってバイト単位で並べ替え |

### byte_reverse への応用

ビットボードのバイト逆順操作（byte_reverse）は、`pshufb`（`_mm_shuffle_epi8`）1 命令で実現できます。
この操作は rsshogi が採用する [Qugiy 方式の飛び利き計算](../bitboard/attacks.md#rsshogi-の選択-qugiy-方式差分抽出byte_reverse)で、逆方向のビーム処理に必要です。

```text
入力:  [b15 b14 b13 b12 b11 b10 b9 b8 | b7 b6 b5 b4 b3 b2 b1 b0]
mask:  [  0   1   2   3   4   5  6  7 |  8  9 10 11 12 13 14 15]
出力:  [b0  b1  b2  b3  b4  b5 b6 b7 | b8 b9 b10 b11 b12 b13 b14 b15]
```

rsshogi は環境に応じて 3 つの実装パスを持ちます。

- **SSSE3 あり（128bit）**: `Bitboard::byte_reverse()` が `_mm_shuffle_epi8`（`pshufb`）を使用。
- **AVX2（256bit）**: `U64x4::byte_reverse_simd()` が `_mm256_shuffle_epi8`（`vpshufb`）で各 128bit レーン内を逆順化。
- **SSSE3 非対応**: `u64x2_ops::byte_reverse` が各 64bit を `u64::swap_bytes()` で逆順にするスカラー実装にフォールバック。

つまり SSSE3 非対応環境でもスカラー実装で正しく動作します。

## SSE4.1: テスト命令

| intrinsic | アセンブリ | 動作 |
|-----------|-----------|------|
| `_mm_testz_si128(a, b)` | `ptest` | `(a AND b) == 0` なら ZF=1 |

`ptest` は 2 つの 128 ビット値の AND を取り、結果がゼロかどうかをフラグレジスタに反映します。
値そのものを書き出さずにフラグだけを更新するため、条件分岐と組み合わせて効率的に使えます。

### 主な用途

```rust,ignore
// ビットボードがゼロかどうか
fn is_empty(bb: Bitboard) -> bool {
    // ptest xmm0, xmm0 → ZF で判定
    _mm_testz_si128(bb.m, bb.m) != 0
}

// 2 つのビットボードに共通のビットがあるか
fn has_intersection(a: Bitboard, b: Bitboard) -> bool {
    // ptest xmm0, xmm1 → (a & b) == 0 なら交差なし
    _mm_testz_si128(a.m, b.m) == 0
}
```

`is_empty` と `has_intersection` は指し手生成と合法性チェックの最内周ループで呼ばれるため、1 命令で完結することの効果は大きいです。

## POPCNT: ビットカウント

| intrinsic | アセンブリ | 動作 |
|-----------|-----------|------|
| `popcnt(x)` | `popcnt` | セットされたビットの数を返す |

POPCNT は 64 ビット整数中の 1 のビット数を数える専用命令です。
Intel では SSE4.2 と同時に Nehalem（2008）で導入され、AMD では ABM 拡張として Phenom II（2007）で先行導入されました。

### 性能特性

| CPU | レイテンシ | スループット |
|-----|-----------|------------|
| Intel Nehalem+ | 3 サイクル | 1 サイクル |
| AMD Zen+ | 1 サイクル | 1 サイクル |

ソフトウェア実装（ビットカウントの分割加算）と比較して約 2〜5 倍高速です。

### 主な用途

- **駒数のカウント**: あるビットボードに含まれる駒の数
- **指し手候補数の見積もり**: 利きビットボードの POPCNT で指せるマス数を取得
- **SEE（Static Exchange Evaluation）**: 交換の勝ち負けを判定する際の攻撃駒数カウント
- **評価関数**: 駒の配置パターンの評価

128 ビットビットボードに対しては、上位・下位の各 64 ビットに `popcnt` を適用し、結果を加算します。

```rust,ignore
fn count_ones(bb: Bitboard) -> u32 {
    bb.lo().count_ones() + bb.hi().count_ones()
    // → popcnt rax, [bb]
    //   popcnt rcx, [bb+8]
    //   add eax, ecx
}
```

## BMI1: TZCNT / LZCNT / ANDN

BMI1（Bit Manipulation Instruction Set 1）は、ビット操作を効率化する命令群です。
Intel Haswell（2013）と AMD Piledriver（2012）で導入されました。

### TZCNT（Trailing Zero Count）

| intrinsic | アセンブリ | 動作 |
|-----------|-----------|------|
| `_tzcnt_u64(x)` | `tzcnt` | 最下位セットビット（LSB）の位置を返す |

TZCNT は最下位の 1 ビット（LSB）の位置を返します。
ビットボードから指し手を 1 つずつ取り出す「pop LSB」操作の中核です。

```text
入力: 0b0000_0000_0010_1000
                       ^
結果: 3  （ビット 3 が最下位の 1）
```

### LZCNT（Leading Zero Count）

| intrinsic | アセンブリ | 動作 |
|-----------|-----------|------|
| `_lzcnt_u64(x)` | `lzcnt` | 最上位セットビット（MSB）からの先行ゼロ数を返す |

LZCNT は最上位側の先行ゼロの数を返します。
MSB の位置は `63 - lzcnt(x)` で得られます。

### ANDN

| intrinsic | アセンブリ | 動作 |
|-----------|-----------|------|
| `_andn_u64(a, b)` | `andn` | `(NOT a) AND b` |

SSE2 の `pandn` の 64 ビットスカラー版です。
ビットのクリアに使われます。

### BSF / BSR との違い

BMI1 の TZCNT / LZCNT は、従来の BSF（Bit Scan Forward）/ BSR（Bit Scan Reverse）の改良版です。

| 命令 | 入力が 0 の場合 | 命令セット |
|------|----------------|-----------|
| BSF | **未定義動作** | x86 基本命令 |
| BSR | **未定義動作** | x86 基本命令 |
| TZCNT | オペランドサイズを返す（64） | BMI1 |
| LZCNT | オペランドサイズを返す（64） | BMI1 |

BSF/BSR は入力がゼロの場合に結果が未定義であるため、事前にゼロチェックが必要でした。
TZCNT/LZCNT はゼロ入力に対して定義された値を返すため、分岐なしで安全に使えます。

<details>
<summary>補足: TZCNT のエンコーディング互換性（クリックで展開）</summary>

TZCNT のマシンコードエンコーディングは `rep bsf` と同じです。
BMI1 非対応 CPU で TZCNT を実行すると、`rep` プレフィクスが無視されて BSF として動作します。
入力がゼロでない場合は結果が同じであるため、多くの場合は意図せず互換動作します。
ただしゼロ入力時の挙動が異なるため、正確な動作には BMI1 の存在確認が必要です。

</details>

## BMI2: PEXT / PDEP

BMI2（Bit Manipulation Instruction Set 2）は Intel Haswell（2013）で導入されました。
PEXT と PDEP は将棋エンジンの飛び利き計算を劇的に高速化する可能性を持つ命令ですが、[AMD の互換性問題](history.md#amd-の-bmi2pextpdep問題)に注意が必要です。

### PEXT（Parallel Bits Extract）

| intrinsic | アセンブリ | 動作 |
|-----------|-----------|------|
| `_pext_u64(src, mask)` | `pext` | mask の 1 ビット位置から src のビットを抽出し、右詰めで返す |

PEXT は、マスクで指定された位置のビットを抽出し、連続した下位ビットとして返します。

```text
src:   1 0 1 1 0 1 0 0
mask:  1 0 1 0 1 0 1 0   ← ビット 7, 5, 3, 1 を抽出
       ↓   ↓   ↓   ↓
結果:  0 0 0 0 1 1 0 0   ← 抽出した 4 ビットを右詰め
```

### PDEP（Parallel Bits Deposit）

| intrinsic | アセンブリ | 動作 |
|-----------|-----------|------|
| `_pdep_u64(src, mask)` | `pdep` | src の下位ビットを mask の 1 ビット位置に散布する |

PDEP は PEXT の逆操作で、下位ビットをマスク位置に散布します。

```text
src:   0 0 0 0 1 1 0 0
mask:  1 0 1 0 1 0 1 0   ← ビット 7, 5, 3, 1 に配置
                          ↓
結果:  1 0 1 0 0 0 0 0   ← src の下位4ビットを散布
```

### 飛び利き計算への応用

PEXT を使った飛び利き計算は、従来の Magic Bitboard に代わるアプローチです。

```text
1. 盤面の占有ビットボードから、対象方向の関連マスを PEXT で抽出
2. 抽出結果をインデックスとしてテーブルを参照
3. テーブルから利きビットボードを取得
```

Magic Bitboard がマジックナンバーの乗算とシフトでインデックスを計算するのに対し、PEXT は直接的にビットを抽出するため、概念的にも実装的にもシンプルです。
ただし AMD Zen2 以前での性能問題があるため、Magic Bitboard との実行時切り替えが実用的な戦略です。
rsshogi は PEXT を使わず [Qugiy 方式](../bitboard/attacks.md#rsshogi-の選択-qugiy-方式差分抽出byte_reverse)を採用しており、Zen2 環境でも一貫した性能を発揮します。
各アルゴリズムの比較は[飛び利きアルゴリズム比較](../bitboard/slider-survey.md)を参照してください。

### 性能特性

| CPU | PEXT レイテンシ | PEXT スループット |
|-----|---------------|-----------------|
| Intel Haswell | 3 サイクル | 1 サイクル |
| Intel Skylake+ | 3 サイクル | 1 サイクル |
| AMD Zen〜Zen2 | **18+ サイクル** | **18+ サイクル** |
| AMD Zen3+ | 3 サイクル | 1 サイクル |

## AVX2: 256 ビット整数演算

AVX2 は Intel Haswell（2013）で導入された 256 ビット整数 SIMD 命令セットです。
256 ビットの `__m256i` レジスタ（`ymm0`〜`ymm15`）を使用します。

### 主要命令

| intrinsic | アセンブリ | 動作 |
|-----------|-----------|------|
| `_mm256_or_si256(a, b)` | `vpor` | 256bit OR |
| `_mm256_and_si256(a, b)` | `vpand` | 256bit AND |
| `_mm256_xor_si256(a, b)` | `vpxor` | 256bit XOR |
| `_mm256_andnot_si256(a, b)` | `vpandn` | 256bit ANDN |
| `_mm256_add_epi64(a, b)` | `vpaddq` | 64bit 要素ごとの加算 |
| `_mm256_sub_epi64(a, b)` | `vpsubq` | 64bit 要素ごとの減算 |
| `_mm256_slli_epi64(a, n)` | `vpsllq` | 64bit 要素ごとの左シフト |
| `_mm256_srli_epi64(a, n)` | `vpsrlq` | 64bit 要素ごとの右シフト |

### 将棋エンジンでの用途

**1. 複数ビットボードの一括処理**

256 ビットレジスタに 2 つの 128 ビットビットボードを格納し、1 命令で同時に演算できます。
rsshogi では `Bitboard256` がこのパターンを実装しており、[角の 4 方向利き計算](../bitboard/attacks.md#角bishop_attacks-bitboard256-並列処理)で `_mm256_and_si256`、`_mm256_sub_epi64`、`_mm256_xor_si256` を連鎖させて使用します。

**2. NNUE 評価関数の推論**

NNUE の差分更新と推論には大量のベクトル演算（加算・ClippedReLU など）が含まれます。
AVX2 の 256 ビット整数演算を使えば、16 個の 16 ビット値を 1 命令で処理できます。
これは SSE2（8 個同時）の 2 倍のスループットです。

### 注意点: レジスタ間のレーン境界

AVX2 の 256 ビットレジスタは内部的に 2 つの 128 ビット「レーン」に分かれています。
多くのシャッフル命令はレーンをまたげないため、128 ビット境界を超えるデータの移動には追加の命令（`vperm2i128` 等）が必要です。

```text
256bit レジスタ:
[  上位 128bit レーン  |  下位 128bit レーン  ]
  ← レーン内シャッフル OK →← レーン内シャッフル OK →
         ← レーン間移動は追加命令が必要 →
```

## AVX-512: 512 ビット演算

AVX-512 は 512 ビット幅の `__m512i` レジスタ（`zmm0`〜`zmm31`）を使用する命令セットです。
Intel Skylake-X（2017）で初めて導入され、AMD は Zen4（2022）で対応しました。

### 特徴

- **レジスタ本数の倍増**: 32 本の zmm レジスタ（AVX2 は 16 本の ymm）
- **マスクレジスタ**: `k0`〜`k7` の 8 本のマスクレジスタで要素単位の条件処理
- **多数のサブ拡張**: F, CD, BW, DQ, VL, VBMI, VNNI, BF16 など

### サブ拡張の分類

AVX-512 は単一の命令セットではなく、多数のサブ拡張の集合体です。

| サブ拡張 | 主な機能 | 対応 CPU |
|---------|---------|---------|
| F (Foundation) | 512bit 基本演算 | Skylake-X, Zen4 |
| CD (Conflict Detection) | 衝突検出 | Skylake-X, Zen4 |
| BW (Byte/Word) | 8/16bit 要素操作 | Skylake-X, Zen4 |
| DQ (Doubleword/Quadword) | 32/64bit 拡張 | Skylake-X, Zen4 |
| VL (Vector Length) | 128/256bit への適用 | Skylake-X, Zen4 |
| VNNI | 整数ニューラルネット演算 | Ice Lake, Zen4 |
| BF16 | BFloat16 演算 | Cooper Lake, Zen4 |

### 将棋エンジンでの用途

AVX-512 の主な用途は **NNUE 推論の高速化**です。
512 ビット幅で 32 個の 16 ビット値を同時処理できるため、NNUE の差分更新が AVX2 のさらに 2 倍のスループットで実行可能です。

ビットボード演算（128 ビット）に対しては、AVX-512 の恩恵は限定的です。
512 ビットのうち 384 ビットが無駄になるため、ビットボード単体では AVX2 以上の利点はほぼありません。

### 消費電力とクロックダウン

AVX-512 命令を実行すると、多くの CPU でクロック周波数が低下します。[^avx512-throttle]
これは 512 ビット幅の演算ユニットの消費電力が大きいためです。

| 状態 | Intel Skylake-X 例 |
|------|-------------------|
| 非 AVX | 最大ターボクロック |
| AVX2 使用中 | 約 100〜200 MHz 低下 |
| AVX-512 使用中 | 約 200〜500 MHz 低下 |

このクロックダウンは AVX-512 を使わないコード（ビットボード演算など）にも影響するため、
AVX-512 を NNUE 推論のみに限定し、頻繁に切り替えが起きないよう設計する配慮が必要です。

AMD Zen4 以降ではクロックダウンが Intel より小さいとされていますが、ゼロではありません。

## 命令レイテンシ・スループット早見表

代表的な CPU における主要命令の性能をまとめます。[^agner]
レイテンシは結果が使えるまでのサイクル数、スループットは 1 サイクルあたりに発行できる命令数の逆数です。

| 命令 | Intel Skylake | AMD Zen3 | 備考 |
|------|-------------|----------|------|
| `por` / `pand` (128bit) | 1 / 0.33 | 1 / 0.25 | ビットボード基本演算 |
| `vpor` / `vpand` (256bit) | 1 / 0.33 | 1 / 0.25 | AVX2 |
| `pshufb` (128bit) | 1 / 0.5 | 1 / 0.5 | byte_reverse |
| `ptest` (128bit) | 2 / 1 | 1 / 0.33 | ゼロ判定 |
| `popcnt` (64bit) | 3 / 1 | 1 / 1 | ビットカウント |
| `tzcnt` (64bit) | 3 / 1 | 1 / 0.33 | LSB 検出 |
| `lzcnt` (64bit) | 3 / 1 | 1 / 0.33 | MSB 検出 |
| `pext` (64bit) | 3 / 1 | 3 / 1 | ビット抽出 |
| `pdep` (64bit) | 3 / 1 | 3 / 1 | ビット挿入 |

数値は「レイテンシ / スループット（サイクル）」形式。
スループットの値が小さいほど、連続実行時のパイプライン効率が高いことを示します。

## Rust での SIMD intrinsic の使い方

Rust では `std::arch` モジュールを通じて SIMD intrinsic を使用します。

### コンパイル時の機能ゲート

```rust,ignore
// ファイル冒頭で条件付きインポート
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
use std::arch::x86_64::{
    __m128i, _mm_or_si128, _mm_and_si128, _mm_xor_si128,
};

#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
use std::arch::x86_64::{
    __m256i, _mm256_or_si256, _mm256_and_si256,
};
```

### 実行時の機能検出

```rust,ignore
// 実行時に CPU 機能を検出
if is_x86_feature_detected!("avx2") {
    // AVX2 パスを実行
} else if is_x86_feature_detected!("sse2") {
    // SSE2 フォールバック
}
```

### target_feature 属性

```rust,ignore
// この関数は AVX2 が有効な環境でのみ呼び出し可能
#[target_feature(enable = "avx2")]
unsafe fn process_avx2(data: &[u8]) {
    // AVX2 intrinsic を使用
}
```

`#[target_feature]` を付けた関数は `unsafe` になります。
呼び出し元で CPU 機能を確認してから呼ぶ責務があるためです。

## まとめ

| 命令セット | 将棋エンジンでの主要用途 | 必須度 |
|-----------|----------------------|--------|
| SSE2 | ビットボード論理演算 | 必須（x86-64 標準） |
| SSSE3 | byte_reverse | 推奨（2011 年以降の全 CPU） |
| SSE4.1 | ゼロ判定（ptest） | 推奨 |
| POPCNT | ビットカウント | ほぼ必須 |
| BMI1 | TZCNT / LZCNT | 推奨 |
| BMI2 | PEXT / PDEP | Intel 向けオプション（AMD 注意） |
| AVX2 | NNUE 推論、一括処理 | NNUE 使用時は推奨 |
| AVX-512 | NNUE 推論の最速化 | オプション |

---

[^pcmpeqq]: `pcmpeqq`（64 ビット要素比較）は SSE4.1 で追加。SSE2 では `pcmpeqd`（32 ビット要素比較）を使い、追加のマスク処理で 64 ビット比較を実現する。

[^avx512-throttle]: Intel, "Intel 64 and IA-32 Architectures Optimization Reference Manual" — AVX-512 使用時の周波数低下（license reduction）について記載。

[^agner]: Agner Fog, "Instruction tables: Lists of instruction latencies, throughputs and micro-operation breakdowns for Intel, AMD, and VIA CPUs" — https://www.agner.org/optimize/instruction_tables.pdf
