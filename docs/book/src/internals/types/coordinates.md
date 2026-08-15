# 座標系

> **前提知識**: [基本型](./index.md)

`Square` は将棋盤の 81 マスを `file * 9 + rank` の値で表す。
`File` と `Rank` はどちらも 0 始まりであり、棋譜の数字や文字列とは区別される。

## 盤面の並び

筋は USI の `1` から `9` に対応し、内部値は 0 から 8 である。
段は USI の `a` から `i` に対応し、内部値は 0 から 8 である。
したがって 1 一は `SQ_11 = 0`、5 五は `SQ_55 = 40`、9 九は `SQ_99 = 80` になる。

```text
raw = file * 9 + rank

SQ_11 = 0       SQ_12 = 1       ... SQ_19 = 8
SQ_21 = 9       SQ_22 = 10      ... SQ_29 = 17
...
SQ_91 = 72      SQ_92 = 73      ... SQ_99 = 80
```

この筋優先の並びでは同じ筋のマスが連続する。
先手の前進は rank を 1 減らす操作なので、盤上の隣接マスは `SQ_U = -1` で表せる。

| 棋譜表記 | USI | file | rank | raw | 定数 |
|---|---|---:|---:|---:|---|
| 1 一 | `1a` | 0 | 0 | 0 | `SQ_11` |
| 5 五 | `5e` | 4 | 4 | 40 | `SQ_55` |
| 7 六 | `7f` | 6 | 5 | 59 | `SQ_76` |
| 9 九 | `9i` | 8 | 8 | 80 | `SQ_99` |

## `Square` の生成と変換

`Square::from_file_rank` は型付きの `File` と `Rank` から升を作る。
`Square::from_usi` は `1a` から `9i` の USI 表記を検証して変換する。
`Square::to_usi` は同じ表記へ戻す。

```rust,ignore
use rsshogi::types::{File, Rank, Square, SQ_76};

let square = Square::from_file_rank(File::FILE_7, Rank::RANK_6);
assert_eq!(square, SQ_76);
assert_eq!(square.to_usi(), "7f");
assert_eq!(Square::from_usi("7f"), Some(SQ_76));
```

USI の `f` は六段を表すため、棋譜の 7 六は `7f` である。
棋譜の 1 始まりの数値を raw 値へ直接使わず、`File`、`Rank`、または USI パーサで変換する。

`Square::from_index` と `Square::new` は入力を検証しない。
外部入力の整数を受け取る場合は、`Square::is_valid()` を確認してから盤面アクセスへ使う。
`Square::NONE` は `none` として文字列変換できるが、通常の USI 升ではない。

## 方向と対称変換

隣接マスへの相対方向は `SQ_U`、`SQ_D`、`SQ_R`、`SQ_L` と斜め方向の定数で表す。
これらは盤上にいることを保証しないので、相対演算の結果を参照する前には境界を管理する。

`Square::flip()` は 180 度回転であり、先後を入れ替えた局面の対称変換に対応する。
`Square::mirror_file()` は筋だけを反転し、段を保った左右対称を作る。

```rust,ignore
use rsshogi::types::{SQ_11, SQ_91};

assert_eq!(SQ_11.flip().to_usi(), "9i");
assert_eq!(SQ_11.mirror_file(), SQ_91);
```

`flip()` は先後を反転する学習データや後手の指し手を正規化するときに使う。
`mirror_file()` は左右対称 augmentation のための変換であり、手番は変えない。

## 方向を使う際の境界

`SQ_U`、`SQ_D`、`SQ_R`、`SQ_L` は raw 値の差であり、盤端を越える操作を防がない。
例えば九段から `SQ_D` を足すと次の筋へ回り込むため、相対方向だけで升の有効性を推測しない。
駒の利きは attack table または `Position` の API で求め、方向定数は局所的な座標計算に限る。

## 反復とテーブル

`Square::iter()` は盤上の有効な 81 マスだけを raw 順に返す。
升ごとの配列は `SquareTable<T, N>` を使うと `Square` で添字アクセスできる。
`Square::NONE` は番兵なので、この反復と通常の盤面テーブルには含めない。

## 次に読む

→ [駒](./pieces.md) で、各マスへ置く駒と持ち駒の表現を確認する。
