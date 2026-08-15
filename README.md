# rsshogi

[![crates.io](https://img.shields.io/crates/v/rsshogi?style=flat-square&logo=rust&logoColor=white)](https://crates.io/crates/rsshogi) [![docs.rs](https://img.shields.io/docsrs/rsshogi?style=flat-square&logo=rust&logoColor=white)](https://docs.rs/rsshogi) [![Documentation](https://img.shields.io/badge/docs-mdBook-1f6feb?style=flat-square&logo=readthedocs&logoColor=white)](https://nyoki-mtl.github.io/rsshogi/) [![PyPI](https://img.shields.io/pypi/v/rsshogi?style=flat-square&logo=pypi&logoColor=white)](https://pypi.org/project/rsshogi/) [![Python](https://img.shields.io/pypi/pyversions/rsshogi?style=flat-square&logo=python&logoColor=white)](https://nyoki-mtl.github.io/rsshogi/python-api.html) [![MIT License](https://img.shields.io/badge/license-MIT-blue?style=flat-square&logo=opensourceinitiative&logoColor=white)](https://github.com/nyoki-mtl/rsshogi/blob/main/LICENSE)

`rsshogi` は、将棋局面の表現、合法手生成、棋譜の入出力、定跡・学習データ形式の操作を行う MIT ライセンスの Rust ライブラリおよび Python パッケージです。

本書はバージョン 1.2.0 の公開 API とワイヤ形式の互換契約を説明します。

## インストール

Rust 利用者は `Cargo.toml` に core crate を追加します。

```toml
[dependencies]
rsshogi = "1.2.0"
```

core crate はデータ形式機能を既定で有効にしません。
用途に応じて `book`、`records`、`position-serialization`、`policy-labels`、`svg`、`validation`、`initial-positions` を選択してください。
`python-data` は Python バインディング向けのデータ機能群を有効にします。

Python は 3.10 以降が必要です。
通常版は `rsshogi`、AVX2 対応 x86_64 CPU 向けの最適化版は `rsshogi-avx2` です。
両方を同じ環境へインストールしないでください。

```console
python -m pip install rsshogi
```

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

[マニュアル](docs/book/src/README.md) では API 群、局面の観測可能な意味論、棋譜・定跡形式、Python 固有の補助機能を説明します。
1.1.1 から更新する場合は [1.2.0 への移行](docs/book/src/migration.md) と [CHANGELOG](CHANGELOG.md) を確認してください。

## ライセンス

MIT
