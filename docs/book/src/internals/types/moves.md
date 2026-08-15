# 指し手の表現

> **前提知識**: [駒の表現](./pieces.md)

`Move` は局面から独立して保存できる 16 bit の指し手である。
`Move32` は同じ下位 16 bit に移動後の駒を加えた 32 bit の指し手である。

## `Move` のビット配置

`Move` の bit 0 から 6 は移動先である。
bit 7 から 13 は通常手の移動元、または駒打ちの駒種である。
bit 14 は駒打ち、bit 15 は成りを表す。

```text
bits  0..=6   to square
bits  7..=13  from square or dropped piece type
bit       14  drop
bit       15  promotion
```

`Move::normal`、`Move::promotion`、`Move::drop` は有効な通常手の構築に使う。
`from_sq()` は駒打ちでは升ではなく符号化された駒種を返すため、駒打ちでは `dropped_piece()` を使う。

```rust,ignore
use rsshogi::types::{Move, PieceType, Square};

let normal = Move::from_usi("7g7f").expect("valid USI move");
let drop = Move::drop(PieceType::PAWN, Square::from_usi("5e").unwrap());
assert!(!normal.is_drop());
assert_eq!(drop.dropped_piece(), Some(PieceType::PAWN));
```

`Move::is_normal()` は bit 配置が通常手または駒打ちとして成立するかを判定する。
これは局面の合法性を保証しないため、二歩、王手放置、持ち駒の不足は `Position` で判定する。

## USI と特殊手

`from_usi` と `to_usi` は通常手、成り、駒打ちを USI 文字列へ変換する。
特殊値は `none`、`null`、`resign`、`win`、`end` として相互変換する。
特殊値を盤面更新へ渡さず、用途に応じて通常手かを `is_normal()` で確認する。

`MOVE_NONE`、`MOVE_NULL`、`MOVE_RESIGN`、`MOVE_WIN`、`MOVE_END` は `Move` と `Move32` の両方で対応する定数を持つ。

## raw 値と wire format

`Move::raw()` は上記の 16 bit 表現をそのまま返す。
`Move32::raw()` の下位 16 bit は必ず対応する `Move::raw()` であり、bit 16 から 20 は移動後の `Piece` である。
この配置は public raw-value contract なので、raw 値を保存または wire format に使う消費者は `Move` と `Move32` を混在させない。

```rust,ignore
use rsshogi::types::{Move, Move32, Piece, SQ_76, SQ_77};

let mv = Move::normal(SQ_77, SQ_76);
let completed = Move32::normal(SQ_77, SQ_76, Piece::B_PAWN);
assert_eq!(mv.raw(), 0x1e3b);
assert_eq!(completed.raw(), 0x0001_1e3b);
assert_eq!(completed.to_move(), mv);
```

`AperyMove` と `AperyMove32` は別の wire 表現である。
相互変換には `Move::to_apery`、`Move32::to_apery(position)`、`Position::move32_from_apery_move32` を使う。
生の整数を別形式の指し手として再解釈しない。

## 部分 `Move32` と完全な `Move32`

`Move32::from_usi` は USI が持たない移動後の駒情報を埋めないため、部分 `Move32` を返す。
`Position::move32_from_move` は現在局面の移動元または駒打ちから駒を補い、完全な `Move32` を返す。
`has_piece_info()` で両者を区別できる。

| 作り方 | `has_piece_info()` | 主な用途 |
|---|---|---|
| `Move32::from_usi` | 偽。 | USI の構文解析。 |
| `Move32::normal`、`promotion`、`drop` | 真。 | 局面が分かる単発構築。 |
| `Position::move32_from_move` | 真。 | `Move` の局面依存拡張。 |

`apply_move32` は下位 16 bit の指し手として局面を更新する。
CSA のように移動後の駒を出力する API では、完全な `Move32` を渡す。

## `Move32` と局面依存情報

`Move32::normal`、`Move32::promotion`、`Move32::drop` は移動後の `Piece` を埋め込む。
一方で `Move32::from_usi` と `Move` からの単純変換は、移動後の駒を含まない。
局面に適用する完全な `Move32` が必要なら `Position::move32_from_move` を使う。

```rust,ignore
use rsshogi::board::Position;
use rsshogi::types::Move;

let position = Position::from_sfen(
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
)?;
let mv32 = position.move32_from_move(Move::from_usi("7g7f").unwrap());
assert!(mv32.has_piece_info());
assert_eq!(mv32.to_usi(), "7g7f");
# Ok::<(), rsshogi::board::parser::SfenError>(())
```

`Move32::to_csa()` は移動後の駒情報を必要とし、不完全な `Move32` では `None` を返す。
`Move32::to_ki2(position)` は必要なら局面から駒情報を補う。
取得駒、捕獲判定、from-to index が必要な処理は `Position::move32_metadata` または `classify_move32` を使う。

## 16 bit と 32 bit の使い分け

手生成の基本出力は `Move` であり、手リストや定跡のように大量保持する用途に適する。
`Move32` は手の適用、棋譜出力、駒種が必要な検索処理に適する。
両型を受け取る対称 API では `*_move` が `Move`、`*_move32` が `Move32` を受け取る。

## 次に読む

→ [Policy ラベル](./policy-labels.md) で、`Move` を学習用の固定クラスへ変換する。
