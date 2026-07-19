# Summary

- [イントロダクション](introduction.md)
- [全体アーキテクチャ](architecture.md)

---

# はじめに

- [インストール](getting-started/installation.md)
- [クイックスタート](getting-started/quickstart.md)
- [例とパターン](getting-started/examples.md)

---

# Python API リファレンス

- [概要](python/index.md)
  - [Board](python/board.md)
  - [Move](python/move.md)
  - [Move32](python/move32.md)
  - [Apery / hcpe 互換](python/apery.md)
  - [型定義](python/types.md)
  - [Record](python/record.md)
  - [GameResult](python/game_result.md)
  - [定跡 (Book)](python/book.md)
  - [外部定跡（DB2016 / SBK）](python/external-books.md)
  - [Policy ラベル](python/policy.md)
  - [Svg](python/svg.md)
  - [NumPy 連携](python/numpy.md)
  - [初期局面](python/initial_positions.md)
  - [パーサ詳細](python/parser.md)
  - [USI プロトコル](python/usi.md)

---

# リファレンス

- [Record と棋譜の構造](reference/records.md)
- [棋譜フォーマット一覧](reference/formats/index.md)
  - [KIF 形式](reference/formats/kif.md)
  - [KI2 形式](reference/formats/ki2.md)
  - [CSA 形式](reference/formats/csa.md)
  - [JKF 形式](reference/formats/jkf.md)
  - [定跡アーキテクチャ（3 層モデル）](reference/formats/book-architecture.md)
  - [定跡バイナリ](reference/formats/book.md)
  - [外部定跡（DB2016 / YBB / SBK）](reference/formats/external-books.md)
  - [DB2016 形式](reference/formats/yaneuraou.md)
  - [SBK 形式](reference/formats/sbk.md)
  - [学習バイナリ（HCP / HCPE / PackedSfen）](reference/formats/training-binaries.md)
- [sbinpack 仕様](reference/sbinpack.md)
- [sazpack 仕様](reference/sazpack.md)
- [cshogi との API 対応表](reference/compat.md)
- [FAQ](reference/faq.md)

---

# 内部技術ドキュメント

- [はじめに](internals/index.md)
- [基本型](internals/types/index.md)
  - [座標系](internals/types/coordinates.md)
  - [駒](internals/types/pieces.md)
  - [指し手](internals/types/moves.md)
  - [Policy ラベル](internals/types/policy-labels.md)
- [ビットボード](internals/bitboard/index.md)
  - [レイアウト](internals/bitboard/layout.md)
  - [基本操作](internals/bitboard/operations.md)
  - [利きの計算](internals/bitboard/attacks.md)
  - [飛び利きアルゴリズム比較](internals/bitboard/slider-survey.md)
- [局面管理](internals/position/index.md)
  - [差分更新と StateInfo](internals/position/state-management.md)
  - [Zobrist ハッシング](internals/position/zobrist.md)
  - [探索エンジンとの統合](internals/position/search-integration.md)

- [合法手生成](internals/movegen/index.md)
  - [特殊ルール](internals/movegen/special-rules.md)
- [詰み判定](internals/mate/index.md)
- [シリアライゼーション](internals/serialization/index.md)
  - [局面の圧縮](internals/serialization/compression.md)
  - [棋譜フォーマット](internals/serialization/formats.md)
- [SIMD 概論](internals/simd/index.md)
  - [拡張命令の歴史](internals/simd/history.md)
  - [拡張命令リファレンス](internals/simd/instructions.md)
- [パフォーマンス最適化](internals/optimization/index.md)
- [参考資料](references.md)
