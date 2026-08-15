# 拡張命令の歴史

x86 の SIMD は MMX から SSE 系へ発展し、128 ビット整数レジスタを使う SSE2 が x86-64 の基礎的な機能になった。

SSSE3 はバイトシャッフルを、SSE4.1 は整数比較とテスト操作を追加した。

AVX と AVX2 はベクトルレジスタ幅を 256 ビットへ広げ、AVX2 は整数ベクトル演算を拡張した。

BMI1 と BMI2 はビット走査、ビット抽出・配置などを補助する別系統の拡張である。

この概要は Intel の Intel 64 and IA-32 Architectures Software Developer's Manual[^intel] と AMD64 Architecture Programmer's Manual[^amd] の命令セット章に基づく。

## rsshogi との関係

rsshogi が現在直接使うのは SSE2、SSSE3、SSE4.1、AVX2、BMI1 の一部である。

AVX-512、BMI2 の PEXT/PDEP、ARM NEON はこの crate の現在の実装経路では使わない。

命令セットの歴史は CPU 要件や配布戦略を決める背景にはなるが、実装の有無はソースの `cfg(target_feature)` を正本とする。

新しい命令セットを使う場合は、機能条件、非対応ビルドの同値経路、対象 CPU での測定を一緒に設計する。

[^intel]: Intel, [Intel 64 and IA-32 Architectures Software Developer's Manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html).

[^amd]: AMD, [AMD64 Architecture Programmer's Manual](https://www.amd.com/en/support/tech-docs.html).

## 命令セットを分けて考える

SSE2、SSSE3、SSE4.1、AVX2 はベクトル整数演算の拡張であり、前の世代をすべて含む単一の名称ではない。

BMI1 と BMI2 はビット操作の拡張であり、SIMD の幅とは独立している。

したがって「AVX2 対応 CPU」だけから SSSE3、SSE4.1、BMI1、BMI2 の有無や採用可能性を決めず、実際の feature 条件を確認する。

実装が必要とするのは、その機能が速いという一般論ではなく、コンパイル済みの命令を実行できることと、対象ワークロードで利益があることである。

## 一般的な移植性の原則

CPU 機能を追加する経路には、条件付きコンパイル、同値な代替経路、各経路を通すテストが必要になる。

古い CPU への対応を残すかは配布対象の判断であり、すべての命令セットを同時に使うことを目的にしない。

ARM の NEON など別アーキテクチャの SIMD は、命令名が似ていても `std::arch::x86_64` の intrinsic を移植できることを意味しない。

アーキテクチャを増やす作業では、データ表現とスカラー契約を先に固定し、各 intrinsic を個別に置き換える。
