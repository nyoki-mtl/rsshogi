# 基本型

> **前提知識**: [内部技術ドキュメントの概要](../index.md)

`rsshogi::types` は盤面、駒、指し手を小さな値型として表現する。
型を分けることで、同じ整数を使う低水準の処理でも座標と駒種を取り違えにくくする。

## この章の地図

- [座標系](./coordinates.md) は `File`、`Rank`、`Square` と盤上の向きを説明する。
- [駒](./pieces.md) は `Color`、`PieceType`、`Piece`、`Hand` を説明する。
- [指し手](./moves.md) は `Move` と盤面情報付きの `Move32` を説明する。
- [Policy ラベル](./policy-labels.md) は学習器へ渡す手番正規化済みラベルを説明する。

## newtype を使う理由

`Square`、`PieceType`、`Piece`、`Move` はそれぞれ `#[repr(transparent)]` の newtype である。
そのため配列添字、ビットボード、手生成のような低水準処理で値として扱える。
同時に API の引数型が意味を表すため、`Square` を `PieceType` の位置へ渡す誤りはコンパイル時に検出される。

```rust,ignore
use rsshogi::types::{File, Rank, Square};

let square = Square::from_file_rank(File::FILE_7, Rank::RANK_6);
assert_eq!(square.to_usi(), "7f");
```

## 値の有効性と境界

各 newtype は raw 値を作る `new` と読み出す `raw` を持つ。
raw 値を使うのはテーブル構築、デコード、テストなど、境界を明示できる箇所に限る。
通常の呼び出し側は定数、パーサ、または変換関数で作った有効な値を渡す。

`Square::NONE` は盤外を表す番兵値であり、盤面配列や `Square::iter()` の対象ではない。
`PieceType::GOLD_LIKE` は金と金相当の駒をまとめる内部分類であり、通常の駒種反復には含まれない。

## 型を選ぶ基準

| 扱いたいもの | 基本型 | 作成方法 |
|---|---|---|
| 一つの盤上升 | `Square` | 単独で作成。 |
| 駒種だけ | `PieceType` | 単独で作成。 |
| 先後を含む盤上の駒 | `Piece` | 単独で作成。 |
| 一方の持ち駒の枚数 | `Hand` | 単独で作成。 |
| 保存・比較する指し手 | `Move` | 単独で作成。 |
| 移動後の駒を含む指し手 | `Move32` | 局面から作成。 |

`Move` から盤面情報を推測する処理は、必ず対応する `Position` を受け取る。
この境界により、定跡や棋譜に保存した手と、特定局面で解決した駒情報を区別できる。

## 型間の責務

`Move` は移動元、移動先、駒打ち、成りだけを保持する。
移動後の駒種や取得駒種のような局面依存情報は `Position` が解決する。
`Move32` は移動後の駒を保持できるため、CSA や KI2 のように駒名を必要とする出力に使える。

型の raw 値はパーサ、packed record、外部形式変換の基礎になる。
raw 値を手作業で組み立てるより、通常は `from_usi`、`from_file_rank`、`Move::normal` などの構築 API を使う。

```rust,ignore
use rsshogi::board::Position;
use rsshogi::types::Move;

let position = Position::from_sfen(
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
)?;
let mv = Move::from_usi("7g7f").expect("valid USI move");
let mv32 = position.move32_from_move(mv);
assert!(mv32.has_piece_info());
# Ok::<(), rsshogi::board::parser::SfenError>(())
```

## 次に読む

→ [座標系](./coordinates.md) で盤上の 81 マスと USI 座標の対応を確認する。
