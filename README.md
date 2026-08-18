# rsshogi

[![crates.io](https://img.shields.io/crates/v/rsshogi?style=flat-square&logo=rust&logoColor=white)](https://crates.io/crates/rsshogi) [![docs.rs](https://img.shields.io/docsrs/rsshogi?style=flat-square&logo=rust&logoColor=white)](https://docs.rs/rsshogi) [![Documentation](https://img.shields.io/badge/docs-mdBook-1f6feb?style=flat-square&logo=readthedocs&logoColor=white)](https://nyoki-mtl.github.io/rsshogi/) [![PyPI](https://img.shields.io/pypi/v/rsshogi?style=flat-square&logo=pypi&logoColor=white)](https://pypi.org/project/rsshogi/) [![Python](https://img.shields.io/pypi/pyversions/rsshogi?style=flat-square&logo=python&logoColor=white)](https://nyoki-mtl.github.io/rsshogi/getting-started/installation.html) [![MIT License](https://img.shields.io/badge/license-MIT-blue?style=flat-square&logo=opensourceinitiative&logoColor=white)](https://github.com/nyoki-mtl/rsshogi/blob/main/LICENSE)

`rsshogi` は、将棋局面の表現、合法手生成、棋譜の入出力、定跡・学習データ形式の操作を行う MIT ライセンスの Rust ライブラリおよび Python パッケージです。

## インストール

### Rust

`Cargo.toml` に追加します。

```toml
[dependencies]
rsshogi = "1.2.2"
```

棋譜や定跡を扱う場合は、必要な機能を `features` に追加します。

```toml
[dependencies]
rsshogi = { version = "1.2.2", features = ["records", "book"] }
```

利用できる機能は `records`、`book`、`position-serialization`、`policy-labels`、`svg`、`validation`、`initial-positions`、`hash-128` です。

### Python

Python 3.10 以降で利用できます。

```console
python -m pip install rsshogi
```

AVX2 対応の x86_64 CPU では最適化版を選べます。

```console
python -m pip install rsshogi-avx2
```

どちらも `rsshogi` として読み込むため、環境ごとに一方をインストールします。

## クイックスタート

Rust:

```rust
use rsshogi::board;

let position = board::position_from_sfen(
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
)?;
assert_eq!(position.to_sfen(None),
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1");
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

[マニュアル](docs/book/src/introduction.md) では API、棋譜と定跡の形式、内部実装を詳しく説明します。
[CHANGELOG](CHANGELOG.md) にはバージョンごとの変更点を記録しています。

## ライセンス

MIT
