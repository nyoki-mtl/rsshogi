# rsshogi

Rust で実装された将棋ライブラリです。Python バインディングも提供しています。

## 主な機能

| カテゴリ | 機能 |
|---------|------|
| **盤面操作** | SFEN/USI 形式での読み書き、指し手の適用・取り消し、合法手生成 |
| **状態判定** | 王手、詰み（1手詰め solver）、千日手、入玉宣言勝ちの判定 |
| **棋譜処理** | KIF/KI2/CSA 形式の読み書き、対局情報（棋戦名、対局者など）・終局情報・コメントの保持 |
| **定跡** | 大規模定跡の高速参照（Zobrist ハッシュ）、外部定跡（DB2016 / YBB / SBK）の直接参照 |
| **USI 補助** | USI の `info` / `bestmove` 文字列のパースと値オブジェクト変換（`rsshogi.usi`） |
| **学習向けラベル** | Rust / Python の両方で policy 学習向け 27x81 / 1496 ラベル変換を提供 |
| **型システム** | Move/Move32、Color、PieceType、Square、Bitboard など |

## はじめに

```bash
pip install rsshogi
```

```python
from rsshogi.core import Board

board = Board()
board.apply_usi("7g7f")
print(board.to_sfen())
```

詳しくは [インストール](getting-started/installation.md) と [クイックスタート](getting-started/quickstart.md) をご覧ください。

## ドキュメント構成

### 入門

- [インストール](getting-started/installation.md) - Python/Rust のインストール手順
- [クイックスタート](getting-started/quickstart.md) - 基本的な使い方
- [例とパターン](getting-started/examples.md) - 実践的なコード例

### Python API リファレンス

- [概要](python/index.md) - クラス・メソッドの詳細
- [Rust API](https://docs.rs/rsshogi) - docs.rs で閲覧

### リファレンス

- [棋譜フォーマット](reference/formats/index.md) - KIF/CSA/KI2/JKF の仕様
- [FAQ](reference/faq.md) - よくある質問

### 内部技術ドキュメント

- [基本型](internals/types/index.md) - 座標系、駒、指し手の内部表現
- [ビットボード](internals/bitboard/index.md) - 盤面表現と合法手生成の実装
- [パフォーマンス最適化](internals/optimization/index.md) - SIMD 最適化

## リンク

- [GitHub](https://github.com/nyoki-mtl/rsshogi)
- [PyPI (rsshogi)](https://pypi.org/project/rsshogi/)
- [docs.rs](https://docs.rs/rsshogi)
