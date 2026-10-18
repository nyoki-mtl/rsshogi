# 目次

- [イントロダクション](introduction.md)
- [全体アーキテクチャ](architecture.md)

---

# はじめに

- [インストール](getting-started/installation.md)
- [クイックスタート](getting-started/quickstart.md)
- [実践レシピ集](getting-started/examples.md)

---

# Python API リファレンス

- [パッケージ概要](python/index.md)
  - [Board と局面検証](python/board.md)
  - [指し手（Move / Move32 / AperyMove）](python/move.md)
  - [基本型（Color / Square / Piece / Bitboard / Hand）](python/types.md)
  - [棋譜と対局結果（Record / GameResult / RecordEditor）](python/record.md)
  - [定跡（Native Book v4 / 外部定跡）](python/book.md)
  - [Policy ラベル](python/policy.md)
  - [自己対局教師データ（SAZ2）](python/sazpack.md)
  - [NumPy 連携と学習バイナリ出力](python/numpy.md)
  - [SVG 盤面描画](python/svg.md)
  - [初期局面と手合割](python/initial_positions.md)
  - [USI プロトコル補助](python/usi.md)

---

# データモデルとフォーマット仕様

- [棋譜データモデル（Record）](reference/records.md)
- [フォーマット総覧と変換ハブ](reference/formats/index.md)
  - [KIF 形式](reference/formats/kif.md)
  - [KI2 形式](reference/formats/ki2.md)
  - [CSA 形式](reference/formats/csa.md)
  - [JKF 形式](reference/formats/jkf.md)
  - [定跡アーキテクチャ](reference/formats/book-architecture.md)
  - [ネイティブ定跡バイナリ（NativeBook）](reference/formats/book.md)
  - [やねうら王定跡形式（DB2016 / YBB）](reference/formats/yaneuraou.md)
  - [ShogiHome 定跡形式（SBK）](reference/formats/sbk.md)
  - [学習バイナリ（HCP / HCPE / PSV / PACK）](reference/formats/training-binaries.md)
- [SBINPACK 仕様（SBN2）](reference/sbinpack.md)
- [SAZPACK 仕様（SAZ2）](reference/sazpack.md)
- [cshogi との API 対応表](reference/compat.md)
- [FAQ](reference/faq.md)

---

# 内部設計とアルゴリズム

- [内部設計の全体像](internals/index.md)
- [基本型の表現](internals/types/index.md)
  - [座標系（Color / File / Rank / Square）](internals/types/coordinates.md)
  - [駒と持ち駒（PieceType / Piece / Hand）](internals/types/pieces.md)
  - [指し手と記法（Move / Move32 / AperyMove / Ki2Notation）](internals/types/moves.md)
  - [Policy ラベルの算術写像](internals/types/policy-labels.md)
- [ビットボードと利き計算](internals/bitboard/index.md)
  - [メモリ配置と筋優先座標](internals/bitboard/layout.md)
  - [ビット演算と幾何テーブル](internals/bitboard/operations.md)
  - [利きテーブルと飛び利き計算](internals/bitboard/attacks.md)
- [局面管理と探索基盤](internals/position/index.md)
  - [StateStack と差分状態管理](internals/position/state-management.md)
  - [Zobrist ハッシュ](internals/position/zobrist.md)
  - [探索エンジンとの統合](internals/position/search-integration.md)
- [合法手生成と将棋規則](internals/movegen/index.md)
  - [王手回避、ピン判定、特殊ルール](internals/movegen/special-rules.md)
- [1 手詰めソルバ](internals/mate/index.md)
- [文字列表現と局面圧縮](internals/serialization/index.md)
  - [ビットストリームと 256-bit 局面圧縮](internals/serialization/compression.md)
- [参考資料](references.md)
