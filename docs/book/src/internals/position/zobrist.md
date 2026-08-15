# Zobrist ハッシュ

> **前提知識**: [差分更新と状態管理](./state-management.md)

`Position` は盤上配置、持ち駒、手番を `ZobristKey` にまとめる。
キー幅は既定で 64 bit、`hash-128` feature の有効時は 128 bit になる。
キーは置換表、局面同一性の高速な絞り込み、反復判定の補助に使える。

## XOR で更新できる理由

Zobrist hashing は「駒、升、色」などの状態要素へ決まった値を割り当て、それらを XOR する。
同じ値を二度 XOR すると元に戻るため、駒を消す操作と加える操作は同じ XOR で表せる。
通常手では移動元の駒を外し、移動先の捕獲駒を外し、移動後の駒を加え、手番を反転する。

持ち駒の増減も同じ原理で、変更前枚数の寄与を外して変更後枚数の寄与を加える。
このため全盤面を走査せずに apply/undo のキーを更新できる。

## 二種類のキー

`key()` は盤上配置、持ち駒、手番を含む局面キーを返す。
`board_key()` は盤上配置と手番だけを含み、持ち駒を含まない。
持ち駒を含む同一性が必要な用途では `key()` を使い、持ち駒を別に比較する用途では `board_key()` と `hand(color)` を組み合わせる。

```rust,ignore
let full = position.key();
let board = position.board_key();
assert_ne!(full, board);
```

この例の不一致は一般に期待できるが、持ち駒がない局面では同じ値になりうる。

完全キーは概念的には次の XOR 合成である。

```text
key = board pieces ^ side-to-move ^ hand-count contributions
board_key = board pieces ^ side-to-move
```

各持ち駒の寄与は駒種、色、枚数で決まる。
一枚ずつ同じ乱数を XOR するのではなく、現在枚数に対応する寄与を使うため、同種の複数枚を区別できる。

## 差分更新と先読み

通常の apply/undo はキーを state stack と同期して更新する。
局面を変更せずに更新後のキーを知りたいときは `key_after`、`key_after_move`、`key_after_null` を使う。
盤面と手番だけの更新後キーが必要なときは `board_key_after` または `board_key_after_move` を使う。

```rust,ignore
let after = position.key_after(mv);
let mut next = position.clone();
next.apply_move32(mv);
assert_eq!(after, next.key());
```

先読み API は渡された手の合法性を検査しない。
非合法な手を渡した場合の値は局面キーとして利用しない。

apply/undo の差分更新は `key_after` と `board_key_after` の結果と整合する。
キー処理を変更する場合は、SFEN からの再構築、全合法手の apply/undo、先読み API の一致を同時に検証する。

## 反復と衝突

Zobrist key は完全な局面証明ではなく、異なる局面が同じ値になる衝突の可能性を持つ。
反復の優劣判定のように持ち駒の比較が必要な箇所では、ライブラリは手の枚数も確認する。
外部の永続ストアでキーを同一性の唯一の根拠にするときは、必要な盤面情報や SFEN を併せて保持する。

既定の 64 bit key でも `hash-128` の 128 bit key でも、衝突が起きたかどうかを
値の形式だけから検出することはできない。
テストではキーのランダム性よりも、apply/undo 後の一致、全再構築との一致、持ち駒変更の反映を確認する。

## 部分キー

`partial_keys()` は pawn、minor、non-pawn、material value のキャッシュを返す。
これらは Position の現在 state から読む検索・評価連携向けの値であり、独自の評価関数の代替ではない。
独自評価の状態は `MoveApplyFacts` や `MoveDelta32` を使ってエンジン側で管理する。

## 置換表に渡す手順

探索ノードでは `key()` を読み、エンジン側の置換表を probe する。
子ノードを probe したいときは `key_after(mv)` を求め、必要なら engine 固有の prefetch を発行してから apply する。
Position は key を返すが、置換表の容量、衝突解決、世代管理、置換方針には関与しない。

## 次に読む

→ [探索エンジンとの統合](./search-integration.md) で、キーと差分を検索側へ渡す境界を確認する。
