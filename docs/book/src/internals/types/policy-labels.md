# Policy ラベル

> **前提知識**: [指し手](./moves.md)

`rsshogi::labels::policy` は指し手を policy 学習用の固定クラスへ変換する。
局面の合法性や特徴量の構築はこのモジュールの責務ではない。

## `MoveLabel`

`MoveLabel` は 27 クラスと 81 個の移動先を組み合わせた 2187 クラスのラベルである。
クラスは通常移動の方向と成りの 20 種、および駒打ちの 7 種からなる。
後手番の手は盤面を 180 度回転して、手番側から見た同じ向きへ正規化する。

```rust,ignore
use rsshogi::labels::policy::{MoveLabel, MoveLabelClass};
use rsshogi::types::{Color, Move};

let mv = Move::from_usi("7g7f").unwrap();
let label = MoveLabel::from_move(mv, Color::BLACK).unwrap();
assert_eq!(label.class(), MoveLabelClass::Up);
```

`MoveLabel::from_move` と `from_move32` は通常の構造を持つ指し手だけをラベル化する。
この変換は局面を受け取らないため、返ったラベルはその局面で合法であることを意味しない。

ラベル値は概念的に `class * 81 + to_square` で並ぶ。
同じ方向クラスの升は連続し、クラスの境界をまたぐと移動方向または駒打ち種別が変わる。
`class()` と `to_sq()` は分解した値を返すので、学習器の出力を解析するときに raw 値を手計算しなくてよい。

## `CompactMoveLabel`

`CompactMoveLabel` は 2187 クラスから、移動パターン、成り、駒打ちの構造上現れないクラスを除いた 1496 クラスである。
`compact()` は圧縮可能な `MoveLabel` を `CompactMoveLabel` に変換する。
`expand()` は圧縮前の `MoveLabel` を返す。

```rust,ignore
use rsshogi::labels::policy::CompactMoveLabel;
use rsshogi::types::{Color, Move};

let mv = Move::from_usi("7g7f").unwrap();
let compact = CompactMoveLabel::from_move(mv, Color::BLACK).unwrap();
assert!(compact.expand().is_structurally_valid());
```

圧縮可能であることも局面で合法であることを表さない。
学習データの手は、先に `Position` と合法手生成で検証してからラベル化する。

通常のラベルと compact ラベルは異なる分類空間である。
モデルの出力次元、訓練データ、復元処理は一方の scheme で統一し、数値を他方の raw 値として扱わない。

## 対称変換

`MoveLabel::mirror_file` と `CompactMoveLabel::mirror_file` は筋反転したラベルを返す。
左右対称 augmentation では局面、指し手、ラベルを同じ変換で揃える。
手番正規化の 180 度回転と、筋反転は別の変換なので混同しない。

## 次に読む

→ [局面](../position/index.md) で、学習データの手を合法性とともに扱う API を確認する。
