# 駒の表現

> **前提知識**: [座標系](./coordinates.md)

駒は先後を持たない `PieceType` と、先後を含む `Piece` に分けて表す。
持ち駒は盤上の `Piece` ではなく `HandPiece` と `Hand` で表す。

## `Color`

`Color::BLACK` は先手、`Color::WHITE` は後手を表す。
`flip()` は相手番を返し、`iter()` は両方の色を反復する。
局面の手番は `Position::turn()` から取得する。

## `PieceType`

`PieceType` は駒種だけを表し、`PAWN` から `KING`、成駒の `PRO_PAWN` から `DRAGON` を持つ。
`NONE` は空マスに対応する。
`promote()` と `demote()` は成れる駒だけを変換し、金と玉はそのまま返す。

```rust,ignore
use rsshogi::types::PieceType;

assert_eq!(PieceType::PAWN.promote(), PieceType::PRO_PAWN);
assert_eq!(PieceType::HORSE.demote(), PieceType::BISHOP);
assert_eq!(PieceType::GOLD.promote(), PieceType::GOLD);
```

`is_hand_piece()` が真になるのは歩、香、桂、銀、角、飛、金だけである。
`PieceType::hand_pieces()` はその 7 種を返し、玉と成駒を持ち駒として扱わない。
`GOLD_LIKE` は金相当の利きをまとめる内部分類であり、`PieceType::iter()` には含まれない。

| raw | `PieceType` | SFEN |
|---:|---|---|
| 0 | `NONE` | 空升。 |
| 1..=8 | `PAWN` から `KING` | `P` から `K`。 |
| 9..=12 | `PRO_PAWN` から `PRO_SILVER` | `+P` から `+S`。 |
| 13..=14 | `HORSE`、`DRAGON` | `+B`、`+R`。 |
| 15 | `GOLD_LIKE` | 盤上の通常駒ではない。 |

成り駒の raw 値は元の駒種へ 8 を加えた値である。
この対応により `promote()` と `demote()` は成れる駒を対称に変換できる。

## `Piece`

`Piece` は `Color` と `PieceType` を一つの値にまとめる。
`Piece::from_parts(color, piece_type)` で作り、`color()` と `piece_type()` で分解する。
`promote()`、`demote()`、`base_piece_type()` は色を保ったまま駒種を変換する。

```rust,ignore
use rsshogi::types::{Color, Piece, PieceType};

let bishop = Piece::from_parts(Color::WHITE, PieceType::BISHOP);
assert_eq!(bishop.promote(), Piece::W_HORSE);
assert_eq!(bishop.base_piece_type(), PieceType::BISHOP);
```

SFEN の大文字は先手、小文字は後手を表す。
`Piece` と `PieceType` は `Display` と `FromStr` を実装し、SFEN で使う駒表記を相互変換できる。

`Piece` の raw 値は下位 4 bit が `PieceType`、bit 4 が色である。
先手の `B_PAWN` は 1、後手の `W_PAWN` は 17 であり、raw 値 16 は有効な駒ではない。
この値を color や成りの独自ビット列として再解釈せず、`color()` と `piece_type()` で読む。

## `Hand`

`Hand` は一方の持ち駒の枚数をまとめて保持する値型である。
`count()` で枚数を読み、`add()` と `sub()` で `HandPiece` 単位の枚数を更新する。
局面の持ち駒は `Position::hand(color)` で取得する。

盤上の駒を取ると `Piece::demote()` に対応する持ち駒種へ戻る。
したがって `PieceType::PRO_PAWN` や `PieceType::HORSE` を `Hand` へ直接入れない。

持ち駒の枚数は局面の色ごとに独立している。
`Position::hand(Color::BLACK)` は先手の持ち駒だけを返し、手番と同じ色であるとは限らない。
駒打ちを作るときは、手番の `Hand` にあるかを確認してから `Move::drop` を使う。

## 次に読む

→ [指し手](./moves.md) で、座標と駒打ちを `Move` に格納する方法を確認する。
