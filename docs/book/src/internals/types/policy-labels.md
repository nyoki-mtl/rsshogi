# Policy ラベル

> **前提知識**: [指し手](./moves.md)

## このページの要点

- `labels::policy` は、`Move` / `Move32` と手番から決まる純粋な move label 変換を提供する
- `MoveLabel` は `27 x 81 = 2187` クラスで、`class * 81 + to_sq` の形に並ぶ
- `CompactMoveLabel` は構造的に存在しえない 691 クラスを除いた `1496` クラス
- core はラベル scheme までを担当し、テンソルレイアウト依存の gather index までは持たない

## なぜ `types` ではなく `labels` なのか

`Move` 自体は将棋エンジンや棋譜処理で広く使う基本型です。
一方、policy ラベルは機械学習モデルの出力設計に由来する scheme であり、
将棋の基本ルールそのものではありません。

そこで rsshogi では、次のように責務を分離しています。

- `types`: Move / Move32 / Square など、局面操作の基礎になる型
- `labels`: それらの型を学習・推論向けの固定長ラベルへ写像する純粋変換

この分離により、scheme 固有の知識を基本型へ埋め込まずに済みます。

## 2187 クラスの `MoveLabel`

`MoveLabel` は 27 種類のラベルクラスと 81 個の移動先マスの組です。

```text
raw = class * 81 + to_sq
```

クラス 27 種は以下で構成されます。

- 通常移動 20 種
- 駒打ち 7 種

後手番の指し手は盤を 180 度回転してからラベル化します。
これにより、常に「手番側から見た先手視点」で policy を学習できます。

```rust,ignore
use rsshogi::labels::policy::{CompactMoveLabel, MoveLabel, MoveLabelClass};
use rsshogi::types::{Color, Move, Square};

let mv = Move::from_usi("7g7f").unwrap();
let label = MoveLabel::from_move(mv, Color::BLACK).unwrap();

assert_eq!(label.class(), MoveLabelClass::Up);
assert_eq!(label.to_sq(), Square::from_usi("7f").unwrap());

let compact = CompactMoveLabel::from_move(mv, Color::BLACK).unwrap();
assert_eq!(compact.expand(), label);
```

## 1496 クラスの `CompactMoveLabel`

2187 クラスのすべてが実際に現れるわけではありません。
たとえば、最上段への桂不成や一段目への歩打ちは、どの局面でも構造的に現れません。

`CompactMoveLabel` は、そうした穴を除いた gapless なラベルです。

- `MoveLabel::compact()` で `Option<CompactMoveLabel>` に変換
- `CompactMoveLabel::expand()` で 2187 クラスへ戻す
- `MOVE_LABEL_TO_COMPACT` / `COMPACT_TO_MOVE_LABEL` でテーブル変換できる

ただし、これは「合法手ラベル」ではありません。
局面依存の合法性判定は行わず、あくまで空盤上の移動・成り・駒打ち制約から見て
存在しうるラベルだけを残しています。

## build 時生成

1496 クラスの判定は、手書きの禁止リストではなく build script で生成しています。
空盤上で次を列挙し、「その `class x to_sq` を実現できる手が 1 つでもあるか」を調べます。

- 通常移動の方向
- 成りクラスへの遷移可否
- 駒打ち可能な段

これにより、

- 実装ミスしやすい手書きテーブルを避けられる
- 2187 / 1496 / 691 の件数をテストで固定できる
- Rust / Python など利用側で同じテーブルを再利用できる

## core が提供しないもの

`labels::policy` は再利用しやすい最小単位に留めています。
次のようなモデル実装依存の補助は core に含めません。

- `95 x 27` テンソル用の gather index
- NumPy / TensorFlow 固有の helper
- 日本語表記への変換

これらは downstream の学習パイプライン側で構築する想定です。
