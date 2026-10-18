# rsshogi Python パッケージ

Native Book は独立圧縮ブロックを持つ wire version 4 を使用する。

`rsshogi` は、将棋局面の操作、合法手生成、1 手詰め判定、棋譜や定跡の入出力、深層学習データの操作を提供する Python パッケージである。
Rust コアライブラリ `rsshogi` の PyO3 バインディングとして実装されている。

## インストール

Python 3.10 以降に対応する。

```console
python -m pip install rsshogi
```

AVX2 対応の x86_64 CPU 環境では、SIMD 最適化版を選択できる。

```console
python -m pip install rsshogi-avx2
```

どちらも `rsshogi` として import するため、同一環境には一方のみをインストールする。

## クイックスタート

```python
from rsshogi.core import Board, Move

board = Board()
board.apply_move(Move.from_usi("7g7f"))
assert board.turn.name == "WHITE"
print(board.to_sfen())
```

## サブモジュール構成

| サブモジュール | 主な提供クラスと関数 |
|---|---|
| `rsshogi.core` | `Board`、`PositionState`、`ValidationReport`、`Move`、`Move32`、`AperyMove`、`AperyMove32`、`to_move`、USI `position` 解析。 |
| `rsshogi.types` | `Color`、`Square`、`PieceType`、`Piece`、`Bitboard`、`Hand`、`MoveType`、`RepetitionState`。 |
| `rsshogi.record` | `Record`、`RecordEditor`、`RecordBuilder`、`RecordMetadata`、`MoveEntry`、`SpecialMoveEntry`、`GameResult`、KIF / KI2 / CSA / JKF / PACK / SBINPACK の入出力。 |
| `rsshogi.book` | `NativeBook`、`NativeBookEditor`、`NativeBookBuilder`、`YaneuraOuBook`（DB2016）、`SbkBook`（ShogiHome）。 |
| `rsshogi.policy` | 2187 次元 Policy ラベルと 1496 次元コンパクトラベルの相互変換。 |
| `rsshogi.sazpack` | AlphaZero 系自己対局教師データ（`SAZ2`）のエンコードとデコード。 |
| `rsshogi.numpy` | `PackedSfenValue`（PSV）、`HuffmanCodedPos`（HCP）、`HuffmanCodedPosAndEval`（HCPE）の NumPy 構造化 `dtype`。 |
| `rsshogi.svg` | `Svg` クラスによる盤面の SVG 描画（Jupyter Notebook 表示対応）。 |
| `rsshogi.initial_positions` | `InitialPosition`（平手および駒落ち 14 種の SFEN と手合割名の相互変換）。 |
| `rsshogi.usi` | USI プロトコルの `info`、`bestmove`、`go` コマンドおよび評価値の解析。 |

## ドキュメント

- **[Python API リファレンス（mdBook）](https://nyoki-mtl.github.io/rsshogi/python/index.html)**
- **[GitHub リポジトリ](https://github.com/nyoki-mtl/rsshogi)**

## ライセンス

MIT
