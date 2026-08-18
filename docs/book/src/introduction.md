# rsshogi

`rsshogi` は、将棋局面の操作、合法手生成、棋譜変換、定跡、学習データ形式を扱う Rust ライブラリおよび Python パッケージです。

## 主な機能

| カテゴリ | 機能 |
|---------|------|
| **盤面操作** | SFEN と USI position の読み書き、指し手の適用と取り消し、合法手生成。 |
| **状態判定** | 王手、千日手、入玉宣言勝ち、一手詰めの判定。 |
| **棋譜処理** | KIF、KI2、CSA、JKF、PACK、SBINPACK の読み書き。 |
| **定跡** | メモリ定跡と静的定跡、DB2016 と SBK の読み書き、YBB の参照。 |
| **学習データ** | HCP、HCPE、PackedSfen、SAZ2、policy label、NumPy dtype。 |
| **Python 補助** | SVG 出力、初期局面、USI の `info` と `bestmove` の解析。 |
| **基本型** | `Move`、`Move32`、`Color`、`PieceType`、`Square`、`Bitboard`、`Hand`。 |

## はじめに

```console
python -m pip install rsshogi
```

```python
from rsshogi.core import Board

board = Board()
board.apply_usi("7g7f")
print(board.to_sfen())
```

Rust から使う場合は `Cargo.toml` に追加します。

```toml
[dependencies]
rsshogi = "1.2.1"
```

詳しい導入方法は [インストール](getting-started/installation.md)、最初の操作は [クイックスタート](getting-started/quickstart.md) を参照してください。

## ドキュメント構成

### 入門

- [インストール](getting-started/installation.md)：Rust と Python のインストール手順。
- [クイックスタート](getting-started/quickstart.md)：局面の作成、合法手、指し手の適用。
- [例とパターン](getting-started/examples.md)：棋譜、定跡、学習データを含むコード例。

### Python API リファレンス

- [概要](python/index.md)：モジュールとクラスの入口。
- [Rust API](https://docs.rs/rsshogi)：Rust の item-level API。

### リファレンス

- [棋譜フォーマット](reference/formats/index.md)：テキスト形式とバイナリ形式の仕様。
- [FAQ](reference/faq.md)：よくある質問と選択指針。

### 内部技術ドキュメント

- [基本型](internals/types/index.md)：座標系、駒、指し手の内部表現。
- [ビットボード](internals/bitboard/index.md)：盤面上の集合と利き計算。
- [局面管理](internals/position/index.md)：差分更新、履歴、Zobrist key。
- [合法手生成](internals/movegen/index.md)：生成モードと合法性判定。
- [パフォーマンス最適化](internals/optimization/index.md)：テーブルと SIMD の使い分け。

## リンク

- [GitHub](https://github.com/nyoki-mtl/rsshogi)
- [PyPI](https://pypi.org/project/rsshogi/)
- [docs.rs](https://docs.rs/rsshogi)
