# パフォーマンス最適化

> **前提知識**: [ビットボード](../bitboard/index.md)、[Position 構造体](../position/index.md)

## このページの要点

- 探索エンジンでは NPS（ノード毎秒）が重要だが、rsshogi は評価関数や探索本体ではなく、その土台となる局面更新・合法手生成を提供する
- rsshogi は SSE2 を基本とし、16 バイト整列した `[u64; 2]` の `struct` をロードして 128 ビット演算を 1 命令化
- PEXT（BMI2）は Intel Haswell+ で高速だが **AMD Zen2 では極端に遅い**。rsshogi は PEXT を使わず Qugiy 方式を採用するため、この問題自体を回避している
- ベンチマークは `criterion` で回帰検出し、`perft` で正確性を同時に担保する

## なぜパフォーマンス最適化が重要か

探索エンジンでは、局面更新・合法手生成・利き計算のような頻出処理のコストが全体の探索速度に直結します。
rsshogi 自体は評価関数や探索本体を持ちませんが、これらの低レベル処理を再利用可能な Rust API として提供します。
そのため、ビットボード演算・SIMD・キャッシュ効率はライブラリとしても重要な設計対象です。

## SIMD の概要

> **SIMD 命令そのものの解説は [SIMD 概論](../simd/index.md) 章にまとめています**
> （命令セットの歴史は [拡張命令の歴史](../simd/history.md)、各 intrinsic の動作・性能は
> [拡張命令リファレンス](../simd/instructions.md)）。本ページでは「どの手法を選ぶか」「どう計測するか」に焦点を当てます。

- `target_feature` フラグで SIMD 実装を切り替える。x86-64 では AVX2 が有効なら角の 4 方向並列処理（`Bitboard256`）に活用し、それ以外はスカラーフォールバック。SSE2/SSSE3/SSE4.1 は基本的な 128 ビット演算に使用する。ARM NEON や SSE4.2 専用パスは現時点では実装していない。
- 安全な抽象化を優先しつつ、パフォーマンスクリティカルな部分では一部 `unsafe` を許容し、境界チェックとテストでカバー。
- ベンチマーク結果は `criterion` による比較を利用し、回帰を検出する。

## 128 ビット Bitboard の実装戦略

将棋の 9×9 = 81 マスを扱うには 64 ビットでは不足するため、128 ビット表現が必要です。
実装方法は主に 3 通りあります。

### 1. `[u64; 2]` の整列 struct（rsshogi 採用）

```rust
{{#include ../../../../../crates/rsshogi/src/types/bitboard.rs:bitboard_struct}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/types/bitboard.rs#L108-L129)</small>

**特徴**:
- 実体は `#[repr(C, align(16))] struct Bitboard { p: [u64; 2] }`。`__m128i` をフィールドに持つ union ではない
- x86_64 + SSE2 では `as_m128()` で `__m128i` にロードして 128 ビット演算を 1 命令で実行
- 非 SSE2 環境では `[u64; 2]` を 2 ワードで処理するフォールバック
- `#[repr(C, align(16))]` で 16 バイト境界に整列し、SSE2 ロード/ストアを最適化

### 2. SSE2 レジスタ型（`__m128i`）

```c
typedef union {
    uint64_t p[2];      // 64ビット×2の配列
    __m128i m;          // SSE2レジスタ
} Bitboard;

// OR演算
Bitboard bb_or(Bitboard a, Bitboard b) {
    Bitboard result;
    result.m = _mm_or_si128(a.m, b.m);
    return result;
}
```

**特徴**:
- 将棋エンジンで使われる代表的な表現
- 明示的に SIMD 命令を呼び出し
- コンパイラの最適化に依存しない

### 3. `uint64_t[2]` 配列型

```c
typedef struct {
    uint64_t lo;  // 下位64ビット
    uint64_t hi;  // 上位64ビット
} Bitboard;

// OR演算
Bitboard bb_or(Bitboard a, Bitboard b) {
    return (Bitboard){
        .lo = a.lo | b.lo,
        .hi = a.hi | b.hi
    };
}
```

**特徴**:
- ポータビリティ最優先
- SIMD非対応環境でも動作
- 演算が 2 回に分かれるため若干遅い

## rsshogi の SIMD 最適化

rsshogi は 16 バイト整列した `[u64; 2]` の `struct` を使用し、x86_64 では `__m128i` にロードして SSE2 命令を利用します。

```rust,ignore
// このコードは以下の SSE2 命令にコンパイルされる
let result = bitboard_a | bitboard_b;
// → movdqa xmm0, [bitboard_a]
// → por xmm0, [bitboard_b]
// → movdqa [result], xmm0
```

`a | b` / `a & b` / `a ^ b` / `!a` がそれぞれ `_mm_or_si128` / `_mm_and_si128` /
`_mm_xor_si128` / `_mm_andnot_si128` に対応します（対応表の詳細は
[SSE2: 128 ビット整数論理演算](../simd/instructions.md#sse2-128-ビット整数論理演算) を参照）。

**利点**:
- 64ビット×2 の分岐が不要
- 香・角の利き更新を一定回数の命令で完結
- 差分更新は in-place XOR で実装
- SSE2 非対応環境では自動的にスカラー演算へフォールバック

### SSE2 の限界と将来の拡張

SSE2 は 128 ビット幅の演算を提供しますが、以下の制約があります。

**制約**:
- シフト命令は 64 ビット単位（`_mm_slli_epi64`）
- 128 ビット全体のシフトには桁上げ処理が必要
- POPCNT は SSE4.2 で追加（SSE2 には含まれない）

**回避策**:

```rust,ignore
// 128ビット左シフトの概念例（実体は p: [u64; 2] を 2 ワードで処理し桁上げする）
pub fn shl(self, n: u32) -> Bitboard {
    let [lo, hi] = self.p;
    if n >= 128 {
        Bitboard::EMPTY
    } else if n >= 64 {
        // 下位ワードが上位ワードへ完全移動
        Bitboard::from_parts([0, lo << (n - 64)])
    } else if n == 0 {
        self
    } else {
        // 下位の桁上げ（carry）を上位へ伝播
        Bitboard::from_parts([lo << n, (hi << n) | (lo >> (64 - n))])
    }
}
```

> 上は説明用の概念コードです。`Bitboard` に `u128` フィールドはなく、内部は `p: [u64; 2]` です。
> 実際のシフトは盤外ビットのマスク処理も伴います。

**AVX2 の活用（現在）**:
- AVX2 (`Bitboard256`): 角の 4 方向対角線を 256 ビット並列処理。AVX2 非対応環境ではスカラー実装にフォールバック。

**今後の拡張候補**:
- AVX-512 や ARM NEON 専用パスは現時点では未実装。

## 専用命令と高速ビット演算

現代の CPU は Bitboard 処理を劇的に高速化する専用命令を備えています。[^antenna-bitboard]
rsshogi で関係するものは次の 2 つで、命令の詳細は SIMD 章に譲ります。

- **POPCNT**: セットビット数のカウント。`Bitboard::count()` がこれにコンパイルされ、駒数・遮蔽駒数・候補手数の見積もりに使う。→ [POPCNT: ビットカウント](../simd/instructions.md#popcnt-ビットカウント)
- **BMI2（PEXT/PDEP）**: マスクに従ったビット抽出/挿入。[^cpw-bmi2] Magic/Rotated 方式の補助に使えるが、**rsshogi は Qugiy 方式のため未使用**（AMD Zen2 の PEXT 遅延問題を回避できる）。→ [BMI2: PEXT / PDEP](../simd/instructions.md#bmi2-pext--pdep)

### 実装ポリシー

rsshogi では Rust 標準のビット演算・組込み関数を優先し、コンパイラが可能な限り最適な機械語に落とす方針です。
環境フラグで CPU 機能検出を行い、安全にフォールバックします。

## 手法別の ISA 相性

| 手法 | 長所 | 短所 | ISA 相性 |
|------|------|------|----------|
| Rotated Bitboards | 実装が明快、O(1) 参照 | 更新コスト、テーブル膨張 | SSE2/AVX2 の論理演算で十分 |
| Magic Bitboard | BMI2 不要で高速、移植性が高い | 生成・埋め込みがやや複雑 | 64bit 乗算/シフトが高速な環境で安定 |
| PEXT（BMI2） | Intel Haswell+ と AMD Zen3+ で非常に高速 | AMD Zen2 では極端に遅い | BMI2 必須。実行時機能検出で切替 |
| LZ/TZ | CPU 依存が小さく、安定した高速性 | ビームや補正マスクの工夫が必要 | 汎用命令のみで動作 |
| **Qugiy（Jumpy Effect）** | **テーブル不要、AVX2 並列化に適合** | **byte_reverse の理解が必要** | **rsshogi 採用** |

## 実エンジン事例と検討材料

- 参照実装の実装は現行 CPU を前提とした実用最適解の一つです。
  - 参照: https://github.com/yaneurao/YaneuraOu/tree/eb2856f91e6088e5b8c1216e3d84d0563f7cd85f
- Haswell 以降の最適化や AMD 大コア向けの最適化事例は以下が参考になります。[^yaneura-haswell] [^yaneura-haswell-2] [^yaneura-threadripper]

## ベンチマークとプロファイリング

### criterion によるマイクロベンチマーク

`criterion` クレートを使って、ビットボード演算や指し手生成の回帰を検出します。

```bash
cargo bench --bench movegen
```

CI で前回結果との比較を自動化し、5% 以上の劣化を検出した場合に警告を出す運用が理想的です。

### perft による正確性＋速度の同時検証

`perft(&position, depth)` は指定深度までの全合法手を数え上げる関数で、
正確性（既知の正解値との一致）と速度（NPS）を同時に検証できます。
rsshogi はライブラリクレートのため、専用バイナリではなく関数として呼び出します。

```rust,ignore
use rsshogi::board::perft::{perft, perft_bench};

// 初期局面で深さ 5 の perft（合法手の総数を数える）
let nodes = perft(&position, 5)?;

// NPS も計測したい場合は perft_bench を使う
let stats = perft_bench(&position, 5)?;
```

### プロファイリングツール

```bash
# Linux: perf stat でキャッシュミス率を確認
perf stat -e cache-misses,cache-references cargo bench

# flamegraph でホットスポットを可視化
cargo flamegraph --bench movegen
```

## キャッシュ最適化の指針

| キャッシュ | サイズ | レイテンシ | Position 収容数 |
|-----------|--------|-----------|----------------|
| L1 | 32 KB | ~1 ns | ~80 個 |
| L2 | 256 KB | ~3 ns | ~640 個 |
| L3 | 8-32 MB | ~10 ns | ~20,000-80,000 個 |

**実践的なポイント**:
- `#[repr(align(64))]` でキャッシュライン境界に揃える
- 頻繁にアクセスするフィールドを構造体の先頭に配置
- Zobrist テーブルは L2 に収まるサイズに保つ

## 落とし穴

### AMD Zen2 の PEXT 問題

Intel の PEXT（BMI2）命令は 1 サイクルで完了しますが、AMD Zen2 では**マイクロコード実装のため 18 サイクル以上**かかります。
PEXT を遠方駒の利き計算に使うエンジンでは、実行時に `cpuid` で CPU を判定して Zen2 では Magic Bitboard に
フォールバックする、といった対処が必要になります。**rsshogi はそもそも PEXT を使わず Qugiy 方式を採用している
ため、この問題の影響を受けません**（SIMD 実装の切替自体はビルド時の `target_feature` で決まり、実行時 cpuid 判定は行いません）。

### `target-cpu=native` の罠

`RUSTFLAGS="-C target-cpu=native"` でビルドすると開発マシンでは高速になりますが、
異なる CPU の環境では不正命令（SIGILL）でクラッシュします。
配布バイナリでは `target-cpu=x86-64-v2`（SSE4.2 まで）など安全な設定を使用してください。

### アラインメント違反

`__m128i` を使う場合、16 バイト境界にアラインされていないとパフォーマンスが劣化するか、
一部の環境ではクラッシュします。`#[repr(C, align(16))]` を忘れないでください。

## まとめ

- rsshogi の最適化対象は、探索本体ではなく、探索エンジンが頻繁に呼び出す局面・合法手生成プリミティブ
- SSE2 の 128 ビット演算で Bitboard の AND/OR/XOR が 1 命令
- POPCNT（SSE4.2）と PEXT（BMI2）は強力だが CPU 依存に注意
- `criterion` + `perft` でベンチマークと正確性を同時検証
- キャッシュライン境界のアラインメントと構造体レイアウトが鍵

---

[^cpw-bmi2]: ChessProgramming Wiki, ["BMI2"](https://www.chessprogramming.org/BMI2)
[^antenna-bitboard]: antenna three ["ビットボード解説"](https://speakerdeck.com/antenna_three/bitutobodojie-shuo)
[^yaneura-haswell]: やねうら王ブログ「Haswell以降専用だと何が嬉しいのですか？」（2015-10-09）https://yaneuraou.yaneu.com/2015/10/09/haswell%E4%BB%A5%E9%99%8D%E5%B0%82%E7%94%A8%E3%81%A0%E3%81%A8%E4%BD%95%E3%81%8C%E5%AC%89%E3%81%97%E3%81%84%E3%81%AE%E3%81%A7%E3%81%99%E3%81%8B%EF%BC%9F/
[^yaneura-haswell-2]: やねうら王ブログ「続: Haswell以降専用だと何が嬉しいのですか？」（2015-10-11）https://yaneuraou.yaneu.com/2015/10/11/%e7%b6%9a-haswell%e4%bb%a5%e9%99%8d%e5%b0%82%e7%94%a8%e3%81%a0%e3%81%a8%e4%bd%95%e3%81%8c%e5%ac%89%e3%81%97%e3%81%84%e3%81%ae%e3%81%a7%e3%81%99%e3%81%8b%ef%bc%9f/
[^yaneura-threadripper]: やねうら王ブログ「YaneuraOu Ryzen Threadripper 3990X optimization」（2020-08-02）https://yaneuraou.yaneu.com/2020/08/02/yaneuraou-ryzen-threadripper-3990x-optimization/
