# 1.7.3 の変更内容

1.7.3 は後方互換な内部改善を含む。
Rust と Python の公開 API、および各定跡・棋譜フォーマットの保存形式は変更しない。

## Native Book の CRC-32C ハードウェア命令対応

`book` feature 内の Native Book v4 用 CRC-32C（Castagnoli）計算で、対応する CPU 命令による 8 バイト単位の更新を利用する。

- AArch64 環境では ARMv8 の `crc` 拡張命令（`__crc32cd` / `__crc32cb`）を使う。コンパイル対象で `crc` が有効なら直接利用し、それ以外は実行時検出により切り替える。Apple Silicon（`aarch64-apple-darwin`）の標準ビルドでは `crc` が有効になる。
- x86_64 環境では SSE4.2 命令（`_mm_crc32_u64` / `_mm_crc32_u8`）を使う。`rsshogi-avx2` などの `sse4.2` 有効ビルドではコンパイル時に直接利用し、標準ビルドでは実行時検出により切り替える。
- 未対応環境や Miri では従来の 256 要素テーブル実装へフォールバックする。

計算結果と保存バイト列は従来実装と完全に一致する。
外部クレートへの新規依存や Cargo feature の追加はなく、Python でも標準パッケージ `rsshogi` の ARM64 / x86_64 各 wheel にそのまま反映される。

## NEON による利きの有無判定

AArch64 の NEON 対応環境では、指定マスへの歩、桂、銀系、金系の利きをベクトル上でまとめて照合し、最後に有無を判定する。
玉の移動先の安全確認など、合法手生成で繰り返し使う処理が対象となる。
飛び駒の利きの計算と、先後や成り駒の扱いは維持する。

NEON はコンパイル対象の機能が有効な場合に使い、それ以外の環境や Miri ではスカラー実装を使う。
Apple Silicon や Linux AArch64 では標準パッケージ `rsshogi` を利用でき、NEON 専用のパッケージや追加設定は不要である。
導入方法は [インストール](installation.md)、処理の詳細は [基本演算、幾何テーブル、SIMD 命令](../internals/bitboard/operations.md#aarch64-の-neon-による利き判定) を参照。
