# 基本操作

`Bitboard` はマス集合なので、基本操作は集合演算として読める。

```rust,ignore
let occupied = black | white;
let empty = Bitboard::ALL & !occupied;
let capturable = enemy & attacks;
let quiet_targets = attacks.and_not(occupied);
```

`!` と `Bitboard::not` は盤外ビットを必ず 0 にする。

`and_not(a, b)` は `a & !b` を表し、対象集合から除外集合を引くホットパス用の操作である。

## マスの操作

`from_square` は一つだけビットが立つ集合を作る。

可変の集合には `set`、`clear`、`test` を使う。

`test_index` は検証済みの生インデックスを持つホットパス向けであり、通常は `Square` を受け取る `test` を使う。

いずれも盤上の `Square` を前提とし、開発時には `debug_assert` が範囲外利用を検出する。

## 走査

`lsb` と `msb` はそれぞれ最小・最大の `Square` を `Option` で返す。

`pop_lsb` は最小ビットを返して集合から消すため、候補集合を消費しながら列挙できる。

```rust,ignore
let mut targets = attacks & !own;
while let Some(to) = targets.pop_lsb() {
    // to ごとの候補手を処理する。
}
```

`pop_lsb_unchecked` は空でないことを呼び出し側が保証するときだけ使う。

`BitIter` と `IntoIterator for &Bitboard` は同じ LSB 順の走査を提供する。

`count`、`any`、`is_empty`、`more_than_one` は候補数による分岐に使う。

## 盤面マスクと変換

`file_mask` と `rank_mask` は静的に構築された 9 個ずつのマスクを返す。

`promotion_zone` は色ごとの三段を返すため、成りの可否を判定する集合式に使える。

`flip` は 180 度回転、`mirror_file` は筋方向の反転を返す。

これらは各盤上マスを走査する変換であり、飛び利きの内部ビット反転とは役割が異なる。

`byte_reverse`、`unpack`、`decrement`、`decrement_pair` は内部の二レーン計算を補助する操作である。

これらを局面の対称変換や保存形式として解釈せず、必要なときは意図に対応する `flip` または `mirror_file` を選ぶ。

## SIMD とフォールバック

AND、OR、XOR、AND NOT は、x86_64 で SSE2 が有効なら 128 ビット intrinsic を使う。

それ以外では二つの `u64` に同じ意味の演算を適用する。

`intersects` は SSE4.1 が有効なら `ptest` を使い、それ以外では二ワードの交差判定に戻る。

どちらの経路でも API の結果とビットの意味は同一である。

## 集合式の定石

ビット演算は優先順位よりも意味が読めることを優先し、中間集合に名前を付ける。

```rust,ignore
let own = bitboards.by_color(us);
let enemy = bitboards.by_color(!us);
let occupied = own | enemy;
let targets = rook_attacks(from, occupied).and_not(own);
```

この形なら、利きの計算が占有を必要とし、最終的な移動先では自駒を除くことが明確になる。

`attacks & enemy` は捕獲候補、`attacks.and_not(occupied)` は空きマスへの候補になる。

捕獲可能な駒を先に取り出すか、移動先を先に取り出すかは後続処理が必要とする情報で選ぶ。

## 破壊的走査の注意

`pop_lsb` は受け手を変更する。

元の集合を後で使うなら、値コピーを別のローカル変数に置いてから走査する。

```rust,ignore
let attackers = position.attackers_to(king, occupied);
let mut remaining = attackers;
while let Some(from) = remaining.pop_lsb() {
    // attackers は元の集合として残る。
}
```

`Bitboard` は `Copy` なので、このコピーにヒープ割り当てはない。

空集合で `pop_lsb_unchecked` を呼ぶことは unsafe 契約違反であり、候補が一つ以上あることを別の条件で確立した後だけに限定する。

## 更新の形

駒を一マス動かすときは、色別集合、駒種別集合、全占有のすべてで始点を消し終点を立てる。

捕獲があれば、捕獲された色と駒種の集合から終点を消してから移動駒を置く。

この操作は XOR だけで一般化できるように見えても、駒種の変化や捕獲があるため、局面更新の既存 API を通す方が安全である。
