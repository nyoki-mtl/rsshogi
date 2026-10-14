# rsshogi

[![crates.io](https://img.shields.io/crates/v/rsshogi?style=flat-square&logo=rust&logoColor=white)](https://crates.io/crates/rsshogi) [![docs.rs](https://img.shields.io/docsrs/rsshogi?style=flat-square&logo=rust&logoColor=white)](https://docs.rs/rsshogi) [![Documentation](https://img.shields.io/badge/docs-mdBook-1f6feb?style=flat-square&logo=readthedocs&logoColor=white)](https://nyoki-mtl.github.io/rsshogi/) [![PyPI](https://img.shields.io/pypi/v/rsshogi?style=flat-square&logo=pypi&logoColor=white)](https://pypi.org/project/rsshogi/) [![Python](https://img.shields.io/pypi/pyversions/rsshogi?style=flat-square&logo=python&logoColor=white)](https://nyoki-mtl.github.io/rsshogi/getting-started/installation.html) [![MIT License](https://img.shields.io/badge/license-MIT-blue?style=flat-square&logo=opensourceinitiative&logoColor=white)](https://github.com/nyoki-mtl/rsshogi/blob/main/LICENSE)

`rsshogi` は、将棋局面の表現、合法手生成、1 手詰め判定、棋譜や定跡の入出力、深層学習データの操作を行う MIT ライセンスの Rust ライブラリおよび Python パッケージである。

## インストール

### Rust

`Cargo.toml` に追加する（Rust 1.95 以降が必要）。

```toml
[dependencies]
rsshogi = "1.3.0"
```

デフォルトでは探索エンジン向けの最小コア（`board`、`mate`、`types`、`movegen`）のみが有効になる。
棋譜（`records`）、定跡（`book`）、Policy ラベル（`policy-labels`）、256-bit 局面圧縮（`position-serialization`）、SVG 出力（`svg`）、局面検証（`validation`）、128-bit Zobrist ハッシュ（`hash-128`）は Cargo の `features` で選択できる。

```toml
[dependencies]
rsshogi = { version = "1.3.0", features = ["records", "book"] }
```

各 Feature の詳細は [rsshogi マニュアルの全体アーキテクチャ](https://nyoki-mtl.github.io/rsshogi/architecture.html) および [docs.rs の Feature flags](https://docs.rs/crate/rsshogi/latest/features) を参照。

### Python

Python 3.10 以降で利用できる。

```console
python -m pip install rsshogi
```

AVX2 対応の x86_64 CPU 環境では最適化ビルドを選択できる。

```console
python -m pip install rsshogi-avx2
```

どちらも `rsshogi` として import するため、同一環境には一方のみをインストールする。

## クイックスタート

Rust:

```rust
use rsshogi::board;

let position = board::position_from_sfen(
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
)?;
assert_eq!(
    position.to_sfen(None),
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"
);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Python:

```python
from rsshogi.core import Board, Move

board = Board()
move = Move.from_usi("7g7f")
board.apply_move(move)
print(board.to_sfen())
```

## ドキュメントと変更履歴

- **[rsshogi マニュアル（mdBook）](https://nyoki-mtl.github.io/rsshogi/)**：Python API リファレンス、棋譜・定跡・教師データのフォーマット仕様、内部データ構造とアルゴリズムの解説。
- **[Rust API リファレンス（docs.rs）](https://docs.rs/rsshogi)**：Rust クレートの全公開型と関数の仕様。
- **[CHANGELOG](CHANGELOG.md)**：バージョンごとの変更履歴。

## 利用例

- [rsshogi-nnue-mini](https://github.com/nyoki-mtl/rsshogi-nnue-mini)：`rsshogi` を基盤とした USI 将棋エンジンの実装例。

## ライセンス

MIT
