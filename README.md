# rsshogi

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

## 公開サーフェス

- Rust: `board`、`mate`、`types`、`movegen`、および機能で有効化する `records`、`book`、`labels`。
- Python: `rsshogi.core`、`types`、`record`、`book`、`policy`、`sazpack`、`numpy`、`svg`、`usi`、`initial_positions`。
- 形式: SFEN と USI position text、KIF、KI2、CSA、JKF、PACK、SBINPACK、packed SFEN、Huffman-coded position、DB2016、YBB、SBK、SAZ2。

## 互換性と削除

公開 `Bitboard256` サーフェスと `peta_shock` API は削除されました。
これらを使うコードは、文書化された `Bitboard`、board、move API へ移行してください。
互換エイリアスはありません。

手の生成が保証するのは合法手の**集合**であり、出力順ではありません。
幾何、局面符号化、外部形式のバイト列・テキスト契約が互換性の境界です。

HCP、PackedSfen、PACK、HCPE、YBB、SBK、Zobrist key は 1.1.1 の表現を維持します。
既存データは 1.2.0 への更新だけを理由に再生成する必要はありません。
完全な合法手集合には Rust の `LegalAll` または Python の `Board.legal_moves()` を使ってください。

## ライセンス

MIT。
`LICENSE` を参照してください。
