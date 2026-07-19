# 拡張命令の歴史

> **前提知識**: [SIMD 概論](index.md)

## このページの要点

- x86 の SIMD 拡張は 1997 年の MMX に始まり、SSE → AVX → AVX-512 と段階的にレジスタ幅が拡大してきた
- Intel が拡張を先導し AMD が追従するパターンが多いが、AMD が先行した事例（3DNow!、FMA3、ABM）もある
- **SSE2 は x86-64 の必須仕様**であり、すべての 64 ビット x86 CPU で利用可能
- BMI2（PEXT/PDEP）は Intel Haswell 以降で高速だが、AMD は Zen3 まで実用的な速度に達しなかった
- バイナリ配布時は x86-64 のマイクロアーキテクチャレベル（v1〜v4）を意識する必要がある

## 年表: x86 SIMD 拡張の変遷

```text
1997  MMX ─────────── 64bit SIMD の幕開け（Intel Pentium MMX）
 │
1999  SSE ─────────── 128bit 浮動小数点（Intel Pentium III）
 │    3DNow! ──────── AMD 独自の浮動小数点 SIMD（AMD K6-2, 1998）
 │
2001  SSE2 ────────── 128bit 整数演算（Intel Pentium 4）
 │                    ★ x86-64 の必須命令セット
 │
2004  SSE3 ────────── 水平演算の追加（Intel Prescott）
 │
2006  SSSE3 ───────── バイトシャッフル（Intel Core 2 Merom）
 │
2007  SSE4.1 ──────── テスト・ブレンド命令（Intel Penryn）
 │
2008  SSE4.2 ──────── 文字列処理・CRC32（Intel Nehalem）
 │    POPCNT ──────── ビットカウント専用命令
 │
2011  AVX ──────────── 256bit 浮動小数点（Intel Sandy Bridge）
 │
2012  FMA3 ─────────── 積和演算 ★AMD Piledriver が先行
 │    BMI1 ─────────── TZCNT, LZCNT, ANDN
 │
2013  AVX2 ─────────── 256bit 整数演算（Intel Haswell）
 │    BMI2 ─────────── PEXT, PDEP（Intel Haswell）
 │
2017  AVX-512 ──────── 512bit 演算（Intel Skylake-X）
 │
2022  AVX-512 ──────── AMD Zen4 が対応
 │
2023  APX/AVX10 ────── 次世代拡張（Intel 計画中）
```

## Intel プロセッサの対応状況

Intel はほぼすべての x86 SIMD 拡張の発案者であり、新命令は Intel の新マイクロアーキテクチャとともに導入されてきました。

| 世代 | マイクロアーキテクチャ | 登場年 | 追加された主要命令セット |
|------|---------------------|--------|----------------------|
| Pentium MMX | P5 | 1997 | MMX |
| Pentium III | P6 (Katmai) | 1999 | SSE |
| Pentium 4 | NetBurst (Willamette) | 2001 | SSE2 |
| Pentium 4 | NetBurst (Prescott) | 2004 | SSE3 |
| Core 2 Duo | Core (Merom) | 2006 | SSSE3 |
| Core 2 Duo | Core (Penryn) | 2007 | SSE4.1 |
| Core i7 (1st) | Nehalem | 2008 | SSE4.2, POPCNT |
| Core i (2nd) | Sandy Bridge | 2011 | AVX |
| Core i (4th) | **Haswell** | 2013 | **AVX2, BMI1, BMI2, FMA3** |
| Core i (6th) | Skylake | 2015 | — (命令セット追加なし) |
| Skylake-X | Skylake-X (HEDT) | 2017 | AVX-512 (F, CD, BW, DQ, VL) |
| Core i (10th) | Ice Lake | 2019 | AVX-512 (VNNI, VBMI2 等) |
| Core i (12th) | Alder Lake | 2021 | — (P-core のみ AVX-512)[^alder-lake] |
| Core Ultra | Meteor Lake | 2023 | AVX-512 無効化、AVX-VNNI |

### Intel における重要な転換点

Haswell（2013）は将棋エンジンにとって最も重要な世代です。
AVX2・BMI1・BMI2・FMA3 が一挙に導入され、ビットボード演算（PEXT）と NNUE 推論（AVX2 + FMA3）の両方が劇的に高速化しました。
参照実装が「Haswell 以降専用」ビルドを用意しているのは、この世代で利用可能になる命令群の恩恵が極めて大きいためです。[^yaneura-haswell]

Alder Lake（2021）以降は注意が必要です。
Intel はハイブリッドアーキテクチャ（P-core + E-core）を採用しましたが、E-core（Gracemont）は AVX-512 を**サポートしません**。
そのため Alder Lake 以降のデスクトップ CPU では、OS レベルで AVX-512 が無効化される場合があります。[^alder-avx512]

## AMD プロセッサの対応状況

AMD は独自路線と Intel 追従を繰り返してきた歴史があります。
特に BMI2（PEXT/PDEP）の実装品質が世代によって大きく異なる点は、将棋エンジン開発者が必ず把握すべき事項です。

| 世代 | マイクロアーキテクチャ | 登場年 | 追加された主要命令セット |
|------|---------------------|--------|----------------------|
| K6-2 | K6 | 1998 | 3DNow! (AMD独自) |
| Athlon XP | K7 | 2001 | SSE |
| Athlon 64 | K8 | 2003 | SSE2, SSE3 (rev E で追加) |
| Phenom II | K10 | 2007 | SSE4a, ABM (POPCNT + LZCNT) |
| FX | **Bulldozer** | 2011 | SSSE3, SSE4.1, SSE4.2, AVX |
| FX | Piledriver | 2012 | FMA3, BMI1 |
| A-series | Excavator | 2015 | AVX2, BMI2 (遅い) |
| Ryzen (1st) | **Zen** | 2017 | AVX2, BMI2 (遅い) |
| Ryzen 3000 | **Zen2** | 2019 | — (BMI2 は依然として遅い) |
| Ryzen 5000 | **Zen3** | 2020 | BMI2 高速化 (PEXT/PDEP が実用速度に) |
| Ryzen 7000 | **Zen4** | 2022 | **AVX-512** |
| Ryzen 9000 | Zen5 | 2024 | AVX-512 改善 |

### AMD の SSSE3 問題

AMD は SSSE3（Supplementary SSE3）の対応が大幅に遅れました。
Intel が 2006 年の Core 2 Merom で導入した SSSE3 を、AMD が対応したのは 2011 年の Bulldozer です。
この 5 年間のギャップにより、Phenom II 世代までの AMD CPU では `_mm_shuffle_epi8`（バイトシャッフル）が使えません。

ビットボードの `byte_reverse` 操作は SSSE3 の `_mm_shuffle_epi8` で効率的に実装できるため、
SSSE3 非対応環境ではスカラーでのフォールバックが必要です。
古い CPU との互換性を重視する場合は、SSSE3 非対応環境でもスカラー実装にフォールバックできることが重要です。

### AMD の BMI2（PEXT/PDEP）問題

これは将棋エンジン開発において最も注意すべき互換性問題です。

AMD は Excavator（2015）世代で BMI2 の CPUID フラグを立てましたが、PEXT と PDEP は**マイクロコード実装**であり、
1 命令あたり **18〜30 サイクル**を要しました。[^amd-pext]
Intel Haswell では同じ PEXT が **1 サイクル**（スループット）で完了するため、性能差は 18〜30 倍です。

| CPU | PEXT レイテンシ | PEXT スループット | 実用性 |
|-----|---------------|-----------------|--------|
| Intel Haswell+ | 3 サイクル | 1 サイクル | 高速 |
| AMD Zen / Zen+ | 18+ サイクル | 18+ サイクル | **実用不可** |
| AMD Zen2 | 18+ サイクル | 18+ サイクル | **実用不可** |
| AMD Zen3+ | 3 サイクル | 1 サイクル | 高速 |

この問題の厄介な点は、CPUID では「BMI2 対応」と報告されるため、機能の有無だけでは判別できないことです。
CPU ベンダーとファミリー番号を組み合わせて判定するか、実測で速度を確認する必要があります。

<details>
<summary>影響を受ける CPU の一覧（クリックで展開）</summary>

**PEXT/PDEP が遅い（マイクロコード実装）AMD CPU:**

- Excavator (A10-8700 系, 2015)
- Zen (Ryzen 1000 系, 2017)
- Zen+ (Ryzen 2000 系, 2018)
- Zen2 (Ryzen 3000 系, Ryzen 4000 APU, 2019–2020)

**PEXT/PDEP が高速（ハードウェア実装）AMD CPU:**

- Zen3 (Ryzen 5000 系, 2020)
- Zen3+ (Ryzen 6000 APU, 2022)
- Zen4 (Ryzen 7000 系, 2022)
- Zen5 (Ryzen 9000 系, 2024)

</details>

### 検出方法

```rust,ignore
// CPUID によるベンダー判定 + ファミリー番号での推定
fn is_fast_pext() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        if !is_x86_feature_detected!("bmi2") {
            return false;
        }
        // AMD の場合はファミリー番号で判定
        // Family 0x17 (Zen, Zen+, Zen2) → 遅い
        // Family 0x19 (Zen3, Zen4) → 速い
        // Intel の場合は BMI2 対応なら常に速い
        // ...
    }
    false
}
```

実際の将棋エンジンでは、ビルド時にターゲット CPU を指定するか、実行時に分岐する仕組みを設けます。

## x86-64 マイクロアーキテクチャレベル

2020 年に策定された x86-64 のマイクロアーキテクチャレベル[^psABI]は、
バイナリ配布時のターゲット指定として実用的な指標です。

| レベル | 必須命令セット | 代表的な CPU |
|--------|--------------|-------------|
| **x86-64** (v1) | SSE2 | Athlon 64, Pentium 4 以降すべて |
| **x86-64-v2** | + SSSE3, SSE4.2, POPCNT, CMPXCHG16B | Core 2 以降 (Intel), Bulldozer 以降 (AMD) |
| **x86-64-v3** | + AVX2, BMI1, BMI2, FMA3, LZCNT, MOVBE | Haswell 以降 (Intel), Zen 以降 (AMD) |
| **x86-64-v4** | + AVX-512 (F, BW, CD, DQ, VL) | Skylake-X 以降 (Intel), Zen4 以降 (AMD) |

### 将棋エンジンにおける使い分け

配布向け（最大互換性）  : x86-64-v2  ← POPCNT が使える最低ライン
高速版（Intel/AMD 共通）: x86-64-v3  ← AVX2 + BMI2 が使える
最速版（AVX-512 対応）  : x86-64-v4  ← NNUE 推論が最速

```text
開発時: -C target-cpu=native  ← 自分の CPU で最速
```

**注意**: `target-cpu=native` でビルドしたバイナリは、開発マシンと異なる CPU で不正命令（SIGILL）を起こす可能性があります。
配布バイナリには明示的なレベル指定を使用してください。

Rust での指定方法:
```bash
# x86-64-v2 でビルド
RUSTFLAGS="-C target-cpu=x86-64-v2" cargo build --release

# x86-64-v3 でビルド
RUSTFLAGS="-C target-cpu=x86-64-v3" cargo build --release
```

## 統合対応表: Intel / AMD / ARM

各命令セットの対応開始時期を一覧にまとめます。

| 命令セット | Intel | AMD | ARM (AArch64) |
|-----------|-------|-----|---------------|
| SSE2 | Pentium 4 (2001) | Athlon 64 (2003) | — (NEON で代替) |
| SSSE3 | Core 2 (2006) | Bulldozer (2011) | — |
| SSE4.1 | Penryn (2007) | Bulldozer (2011) | — |
| SSE4.2 | Nehalem (2008) | Bulldozer (2011) | — |
| POPCNT | Nehalem (2008) | Phenom II (2007)[^abm] | CNT (必須) |
| AVX | Sandy Bridge (2011) | Bulldozer (2011) | — |
| BMI1 | Haswell (2013) | Piledriver (2012) | — |
| BMI2 | Haswell (2013) | Excavator (2015)[^bmi2-slow] | — |
| AVX2 | Haswell (2013) | Excavator (2015) | — |
| FMA3 | Haswell (2013) | Piledriver (2012) | FMLA (必須) |
| AVX-512 | Skylake-X (2017) | Zen4 (2022) | SVE/SVE2 (別規格) |

## 非対応環境のまとめ

バイナリ配布や CI テストで注意すべき、主要命令セットの**非対応 CPU**をまとめます。

### SSE2 非対応

x86-64（64 ビットモード）では SSE2 が必須仕様です。
したがって、64 ビット OS を動かしている限り SSE2 非対応の問題は発生しません。
32 ビットの x86 環境（Pentium III 以前）でのみ問題になりますが、現在はほぼ考慮不要です。

### POPCNT 非対応

- Intel: Nehalem (2008) より前の Core 2 世代
- AMD: Phenom I 以前（K10 初期）

POPCNT 非対応環境は一般的な開発・実行環境では少ないものの、古い CI 環境や特殊な仮想化環境では遭遇する可能性があります。

### AVX2 非対応

- Intel: Haswell (2013) より前の Ivy Bridge 以前
- AMD: Excavator / Zen (2015/2017) より前の Bulldozer〜Steamroller

組み込みシステムやサーバーの長期運用環境では、Ivy Bridge 世代の CPU が稼働している場合があります。

### AVX-512 非対応

- Intel: Skylake-X (2017) より前。**Alder Lake (2021) 以降は E-core の都合で無効化される場合がある**
- AMD: Zen4 (2022) より前

デスクトップ向けでは 2024 年時点でも AVX-512 対応率は高くありません。
AVX-512 を必須とするバイナリは配布に不向きです。

## ARM NEON の歴史

ARM 側の SIMD 拡張も簡潔にまとめます。

| 拡張 | 登場年 | レジスタ幅 | 備考 |
|------|--------|-----------|------|
| NEON (ARMv7) | 2005 | 128 bit | オプション（Cortex-A8 等） |
| **NEON (AArch64)** | 2011 | 128 bit | **ARMv8-A 必須仕様** |
| SVE | 2017 | 128–2048 bit | 可変長ベクトル（Fujitsu A64FX） |
| SVE2 | 2019 | 128–2048 bit | 汎用向け拡張（ARMv9） |

将棋エンジンの ARM 対応では、AArch64 の NEON を前提とすれば十分です。
Apple Silicon（M1 以降）は NEON 性能が非常に高く、x86-64 の SSE2〜SSE4.2 相当の処理を同等以上の速度で実行できます。

ただし、ARM には x86 の BMI2（PEXT/PDEP）に相当する命令がありません。
PEXT ベースの飛び利き計算を ARM で使うことはできないため、Magic Bitboard や他のアルゴリズムが必要です。

## まとめ

- x86 の SIMD は MMX（1997）から 25 年以上の歴史を持ち、レジスタ幅は 64 → 128 → 256 → 512 bit と拡大してきた
- SSE2 は x86-64 の必須仕様であり、128 ビットビットボードの基盤として安心して使える
- Haswell（2013）世代は AVX2 + BMI2 の登場により将棋エンジンの性能が飛躍的に向上した転換点
- AMD の BMI2（PEXT/PDEP）は Zen3 以降でようやく実用的な速度になったため、CPU 判定が必須
- バイナリ配布時は x86-64 マイクロアーキテクチャレベル（v1〜v4）を基準に選択する
- ARM AArch64 では NEON が必須仕様であり、PEXT 以外の命令は概ね代替手段がある

---

[^yaneura-haswell]: やねうら王ブログ「Haswell以降専用だと何が嬉しいのですか？」（2015-10-09）

[^alder-lake]: Intel Alder Lake では P-core（Golden Cove）が AVX-512 をサポートするが、E-core（Gracemont）は非対応。混在環境では OS が AVX-512 を無効化する場合がある。

[^alder-avx512]: Linux カーネル 5.18 以降では、E-core を無効化した場合に AVX-512 を有効にできる場合があるが、一般的な運用では無効化される。

[^amd-pext]: Agner Fog, "Instruction tables" — AMD Zen/Zen2 の PEXT/PDEP レイテンシは 18 サイクル以上と計測されている。

[^psABI]: System V ABI, AMD64 Architecture Processor Supplement, "x86-64 Microarchitecture Feature Levels" (2020)

[^abm]: AMD は POPCNT を ABM（Advanced Bit Manipulation）拡張の一部として Phenom II（Barcelona コア）で先行導入した。Intel は SSE4.2 の一部として Nehalem で導入。

[^bmi2-slow]: AMD Excavator〜Zen2 では BMI2 の CPUID フラグは立つが、PEXT/PDEP はマイクロコード実装で極端に遅い。
