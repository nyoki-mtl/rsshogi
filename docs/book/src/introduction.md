# rsshogi

`rsshogi` は、将棋局面の表現、合法手生成、1 手詰め判定、棋譜や定跡の入出力、深層学習向け教師データの操作を提供する MIT ライセンスの Rust ライブラリおよび Python パッケージである。
探索エンジンや学習パイプラインの基盤として再利用できるよう、ヒープ確保を排した高速な局面操作プリミティブと、外部データ形式との厳密なバイナリ互換性を兼ね備える。

## 主な機能

| カテゴリ | 提供する機能 |
|---|---|
| **盤面と合法手** | SFEN と USI `position` の解析および出力、指し手の適用と取り消し、探索用途別の合法手生成（`Legal` / `LegalAll` / `Captures` / `Evasions` / `Checks`）。 |
| **将棋規則と詰み** | 王手とピンの検出、千日手と連続王手の千日手判定、打ち歩詰め判定、入玉宣言勝ちの評価、静的反駁を備えた 1 手詰めソルバ。 |
| **棋譜の入出力** | KIF、KI2、CSA、JKF の相互変換、分岐を含む棋譜ツリー（`Record`）の編集、PACK および SBINPACK（`SBN2`）バイナリの圧縮とストリーミング復号。 |
| **定跡の参照と構築** | インメモリの `MemoryBook`、mmap 対応の `StaticBook`、やねうら王形式（`DB2016` / `YBB`）と ShogiHome 形式（`SBK`）の直接参照、編集用中間表現 `BookDatabase`。 |
| **深層学習データ** | `HuffmanCodedPos`（HCP）、`HuffmanCodedPosAndEval`（HCPE）、`PackedSfen`、`PackedSfenValue`（PSV）、AlphaZero 系自己対局形式 SAZPACK（`SAZ2`）、Policy ラベル（2187 / 1496 次元）、NumPy 構造化 `dtype`。 |
| **Python 補助機能** | 盤面の SVG 描画、平手および駒落ち 14 種の初期局面定義、USI の `info` / `bestmove` / `go` コマンド解析。 |

## 最小コード例

Python では `rsshogi`（または AVX2 対応 x86_64 環境向けの `rsshogi-avx2`）をインストールしてすぐに利用できる。

```console
python -m pip install rsshogi
```

```python
from rsshogi.core import Board

board = Board()
board.apply_usi("7g7f")
print(board.to_sfen())
# => lnsgkgsnl/1r5b1/ppppppppp/9/9/2P6/PP1PPPPPP/1B5R1/LNSGKGSNL w - 2
```

Rust では `Cargo.toml` に `rsshogi` を追加する。
デフォルトではエンジン向けの最小コア（`board`、`mate`、`types`、`movegen`）のみが有効になり、棋譜や定跡などの機能は Cargo feature で選択する。

```toml
[dependencies]
rsshogi = { version = "1.4.0", features = ["records", "book"] }
```

```rust
use rsshogi::board;

let mut pos = board::hirate_position();
pos.init_stack();
let mv = pos.move_from_usi("7g7f").unwrap();
pos.apply_move32(mv);
assert_eq!(
    pos.to_sfen(None),
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/2P6/PP1PPPPPP/1B5R1/LNSGKGSNL w - 2"
);
```

## 目的別の読み進め方

本マニュアルは対象読者の目的に合わせて 4 部構成をとる。

| 読者の目的 | 参照するパート | 主な内容 |
|---|---|---|
| **初めて導入して基本操作を動かす** | [はじめに](getting-started/installation.md) | Python / Rust のインストール、Feature フラグの選び方、[クイックスタート](getting-started/quickstart.md)、[実践レシピ集](getting-started/examples.md)。 |
| **Python で将棋ツールや学習器を書く** | [Python API リファレンス](python/index.md) | `Board`、`Move` / `Move32`、`Record`、`StaticBook`、Policy ラベル、NumPy 連携、SVG 描画など全サブモジュールの API 仕様。 |
| **Rust でエンジンやツールを組み込む** | [全体アーキテクチャ](architecture.md) / [docs.rs](https://docs.rs/rsshogi) | クレート構成、モジュール境界、Feature フラグ、および rustdoc による全公開型と関数のリファレンス。 |
| **棋譜、定跡、教師データの仕様を調べる** | [データモデルとフォーマット仕様](reference/formats/index.md) | [Record データモデル](reference/records.md)、KIF / KI2 / CSA / JKF、[定跡 3 層モデル](reference/formats/book-architecture.md)、[学習バイナリ](reference/formats/training-binaries.md)、[SBINPACK](reference/sbinpack.md)、[SAZPACK](reference/sazpack.md)、[cshogi 互換表](reference/compat.md)。 |
| **内部データ構造と高速化手法を知る** | [内部設計とアルゴリズム](internals/index.md) | 座標系と `Hand` / `Move` のビット配置、[ビットボードと飛び利き](internals/bitboard/index.md)、[`StateStack` と Zobrist ハッシュ](internals/position/index.md)、[合法手生成](internals/movegen/index.md)、[1 手詰めソルバ](internals/mate/index.md)、[256-bit 局面圧縮](internals/serialization/compression.md)。 |

## 外部リンク

- **GitHub リポジトリ**：<https://github.com/nyoki-mtl/rsshogi>
- **crates.io**：<https://crates.io/crates/rsshogi>
- **Rust API ドキュメント（docs.rs）**：<https://docs.rs/rsshogi>
- **PyPI**：<https://pypi.org/project/rsshogi/>
