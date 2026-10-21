# rsshogi

`rsshogi` は、将棋局面の操作、合法手生成、1 手詰め判定、棋譜や定跡の入出力を提供する Rust ライブラリである。

```rust
use rsshogi::board;

let position = board::position_from_sfen(
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
)?;
println!("{}", position.to_sfen(None));
# Ok::<(), Box<dyn std::error::Error>>(())
```

## モジュール構成と Cargo Features

常時有効なモジュールは [`board`]、[`mate`]、[`types`]、およびクレートルートに再エクスポートされた [`movegen`] である。
外部依存を伴う棋譜処理や補助機能は、用途に応じて Cargo feature で有効化する。

| Feature | 有効化されるモジュール / 機能 |
|---|---|
| `records` | `records` モジュール（KIF、KI2、CSA、JKF、PACK、SBINPACK、SAZPACK、HCPE の入出力）。 |
| `book` | `book` モジュール（`NativeBook`、`NativeBookEditor`、`YaneuraOuBook`、`YbbBook`、`SbkBook`、`BookDatabase`）。 |
| `policy-labels` | `labels` モジュール（2187 ラベルと 1496 コンパクトラベルの算術変換）。 |
| `position-serialization` | `PackedSfen`、`HuffmanCodedPos`（HCP）、`PackedSfenValue`（PSV）による局面圧縮。 |
| `svg` | `Position::to_svg` による盤面の SVG 出力。 |
| `validation` | `Position::validate`、`validate_all`、`is_valid` による局面の整合性検証。 |
| `initial-positions` | `board::InitialPosition`（平手および駒落ち 14 種の初期局面 SFEN）。 |
| `hash-128` | `ZobristKey` を 64-bit から 128-bit へ拡張する（下位 64 bit は 64-bit ビルドと同一）。 |
| `python-data` | `book`、`records`、`position-serialization`、`svg`、`validation` を一括で有効化する。 |

## 主要な型と契約

- **局面と履歴（[`board::Position`]）**：SFEN の解析は入力エラーを伴い得るため `Result` を返す。指し手の適用（`apply_move32`）や千日手判定を行う前に `init_stack()` で状態スタックを初期化する。
- **合法手生成（[`movegen`]）**：不成を含む全合法手には `LegalAll`（または `generate_legal_all`）、探索向けに一部の不成を省く生成には `Legal` を使う。王手回避（`Evasions`）、駒取り（`Captures`）、静かな手（`Quiets`）、王手生成（`Checks`）も同様に型パラメータで指定する。`MoveList` と `Move32List` は固定容量のスタック配列であり、ヒープ領域を確保しない。
- **指し手と記法（[`types::Move`] / [`types::Move32`] / [`types::Ki2Notation`]）**：`Move` は移動元、移動先、成り、駒打ちを表す 16-bit 型（やねうら王の `Move16` と同一レイアウト）であり、`Move32` は移動後の色付き駒を加えた 32-bit 型である。cshogi / Apery 互換の 16-bit / 32-bit 表現には [`types::AperyMove`] と [`types::AperyMove32`] を使う。`Ki2Notation` は 1 手分の KI2 表記を `u32` にパックし、ヒープ確保なしでフォーマットと合法手解決を行う。
- **座標と基本型（[`types::Square`] / [`types::Hand`]）**：`Square` の raw 値は `file_index * 9 + rank_index`（`0..=80`、`SQ_NONE = 81`）で定義される。外部入力から基本型を作る際は検証付きコンストラクタ（`from_raw` / `from_usi` など）を使用する。
- **対称変換**：[`types::Move::flip`] と [`types::Move32::flip`] は先後反転、`mirror_file` は左右反転を返す。`Move32::flip` は駒色も反転する。`position-serialization` feature の `Position::to_packed_sfen_flipped` は、SFEN 文字列を経由せずに先後反転した局面を保存する。
- **1 手詰め判定（[`mate::solve_mate_in_one`]）**：手番側が合法な王手で相手玉を 1 手で詰ませられる場合にその `Move32` を返し、詰まない場合は `None` を返す。探索中に可変な `Position` を保持している場合は、局面の複製を行わない [`mate::solve_mate_in_one_in_place`] を利用できる。

データフォーマット仕様と内部設計の詳細は [rsshogi マニュアル](https://nyoki-mtl.github.io/rsshogi/) を参照。

## ライセンス

MIT。
