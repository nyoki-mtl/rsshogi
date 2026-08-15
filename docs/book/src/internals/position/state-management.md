# 差分更新と状態管理

> **前提知識**: [局面](./index.md)

`Position` は手を適用するたびに、盤面、持ち駒、手番、ハッシュ、戦術キャッシュ、反復情報を同期して更新する。
呼び出し側は公開された apply/undo API を対にして使い、内部の state stack を直接操作しない。

## make/unmake の形

探索木の一枝では、局面は次の順で進む。

```text
root position
  apply move A
    apply move B
    undo move B
  undo move A
root position
```

各 apply は新しい current state を積み、各 undo は直前の current state に戻る。
兄弟枝を探索する前に親局面へ戻すことで、盤面全体を枝ごとに複製せずに済む。

## 通常手の apply/undo

`apply_move32` は `Move32` を適用し、`undo_move32` は同じ手を使って直前の局面へ戻す。
`Move` を保存している場合は `apply_move` と `undo_move` の対を使える。
apply は合法性を検査しないため、`Legal` 系の手生成結果か `is_legal_move32` で確認した手を渡す。

```rust,ignore
use rsshogi::board::{movegen::generate_legal_all_move32, Move32List};

let mut next = position.clone();
let mut moves = Move32List::new();
generate_legal_all_move32(&next, &mut moves);
let mv = moves[0];
let before = next.to_sfen(None);

next.apply_move32(mv);
next.undo_move32(mv)?;
assert_eq!(next.to_sfen(None), before);
# Ok::<(), rsshogi::board::MoveError>(())
```

undo が対応する state を持たない場合は `MoveError` を返す。
同じ探索枝では通常手と null move の undo を取り違えない。

`undo_move32_with_delta` は戻した手の順方向 `MoveDelta32` を返す。
評価器が apply 時に記録した値と同じ差分を必要とする場合に、undo 後の局面から再計算せず参照できる。

## 事前計算済みの事実

`apply_move32_with_gives_check(mv, gives_check)` は呼び出し側が求めた王手判定を受け取る。
`apply_move32_with_delta` は `MoveDelta32` を返し、駒打ちか盤上移動か、捕獲と成りの前後情報を表す。
`apply_move32_with_facts` は `MoveApplyFacts` を返し、差分に加えて移動駒、取得駒、王手、更新後キーを一つにまとめる。

評価器や探索スタックはこれらの返り値を使うと、更新後の盤面を再読せずに必要な差分を受け取れる。
返り値は適用した一手に対応する値であり、局面をさらに更新する前に消費する。

`MoveDelta32::Drop` には打った駒種、行き先、適用前の持ち駒枚数が入る。
`MoveDelta32::Board` には移動元、移動先、移動前後の駒、捕獲した駒と捕獲前の持ち駒枚数が入る。
捕獲駒は成りを戻した持ち駒種として加わるため、評価または feature の rollback は `captured` の内容を基準にする。

## null move

`apply_null_move` は盤上と持ち駒を変えずに手番を反転する。
`undo_null_move` は直近の null move を戻す。
探索用の `try_apply_search_null_move` と `undo_search_null_move` は、あらかじめ確保した検索用 state capacity を使う。

null move は将棋の対局手ではないため、棋譜入力や通常の合法手生成では使わない。

## 履歴と反復

`state_history()` は現在局面から利用できる過去 state の読み取り反復を返す。
各 `StateHistoryEntry` から `board_key`、手番側の `hand`、null move からの手数、反復情報を読める。
これは履歴を観察する API であり、履歴を書き換える API ではない。

`repetition_counter()`、`repetition_distance()`、`repetition_times()`、`repetition_type()` は現在 state にキャッシュされた反復情報を返す。
実際の勝敗区分が必要な場合は `repetition_state()` を使う。

`plies_from_null()` は直近の null move から進んだ通常手数を返す。
`continuous_checks()` は色ごとの連続王手の履歴を返し、連続王手千日手の区分に使われる。

## 検索用の容量

通常の `Position` は必要に応じて state stack を伸ばせる。
allocation を避ける検索では `prepare_search_state_capacity` で必要な深さを事前確保し、`try_apply_search_move32_with_facts` を使う。
capacity が足りない場合、この API は `MoveError::StateCapacityExceeded` を返す。

この契約により、検索の hot path が不足した state slot を静かに確保することはない。

`has_prepared_search_state()` は検索用 state が準備済みかを確認する。
通常の解析や棋譜再生では、事前確保ではなく標準の apply/undo API を使う方が単純である。

## state と外部評価の境界

Position の current state は合法性、反復、キー、戦術キャッシュを保つ。
NNUE accumulator、独自の駒割り、探索順序、置換表の entry は Position には格納しない。
これらを外部の探索スタックへ置くことで、同じ `Position` を GUI、棋譜処理、検証、異なる評価器で使える。

## 次に読む

→ [Zobrist ハッシュ](./zobrist.md) で、更新と同時に保たれる局面キーを確認する。
