# 探索エンジンとの統合

> **前提知識**: [局面](./index.md) と [差分更新と状態管理](./state-management.md)

`rsshogi` は局面、ルール、手生成を提供し、探索アルゴリズム、評価関数、置換表は所有しない。
探索側は `Position` を保持し、評価器と探索スタックの状態を別に管理する。

## 分離する理由

局面の合法性と評価値は異なる責務である。
局面ライブラリが特定の評価方式を保持すると、GUI や棋譜解析のように評価を使わない消費者までその状態に依存する。
探索側が評価 state を持てば、同じ Position に対して異なる評価器、置換表、時間管理を組み合わせられる。

## 基本ループ

探索では `LegalAll` または `Legal` で手を生成し、各手を apply/undo の対で探索する。
全ての合法手がなく、手番側が王手中なら詰みである。
`Position::is_mated()` はこの判定を一度で行う読み取り API である。

```rust,ignore
use rsshogi::board::{movegen::generate_legal_all_move32, Move32List};

let mut moves = Move32List::new();
generate_legal_all_move32(&position, &mut moves);
if moves.is_empty() {
    let score_is_mate = position.is_in_check();
    // 探索側の終端スコアへ変換する。
}
```

`LegalAll` は通常省略される不成も含むので、完全性が必要な探索、検証、詰み判定に適する。
通常の対局手だけでよい経路では `Legal` を使う。

現在ノードで一手詰めを調べる場合、読み取り専用の局面には `mate::solve_mate_in_one` を使う。
探索が変更可能な `Position` を保持している場合は、`mate::solve_mate_in_one_in_place` を使うと作業局面の複製を省略できる。
in-place API は正常終了時に局面を復元するが、呼び出し前に state stack が現局面と同期している必要がある。

探索が pseudo-legal mode を使って候補を絞る場合は、玉の安全を確認する段階を別に持つ。
terminal 判定、PV の確定、外部へ返す手には `Legal` または `LegalAll` の契約を使う。

## 評価差分の同期

評価器は apply 前後の盤面を独自に持つ必要はない。
`apply_move32_with_facts` の `MoveApplyFacts` には移動駒、取得駒、成り、駒打ち、更新後キーが含まれる。
評価器はこの値を使って自分の差分 state を更新し、undo 時には探索側の評価スタックを戻す。

```rust,ignore
let facts = position.apply_move32_with_facts(mv, gives_check);
evaluator.apply(facts);
let score = search(position, evaluator);
position.undo_move32(mv)?;
evaluator.undo();
```

`gives_check` は `position.gives_check_move32(mv)` で事前に求められる。
評価器の undo と Position の undo は同じ枝の LIFO 順序で行う。

捕獲駒は apply 後には移動先から消えているため、捕獲の特徴量を更新する評価器は `facts.captured_piece` を読む。
成りによる特徴変化は `facts.moved_piece_before` と `facts.moved_piece_after` を比較できる。

## 置換表と反復

置換表のキーには `position.key()` を使える。
子局面を先読みして置換表を参照する場合は、`key_after(mv)` を先に求める。
置換表そのものの容量、置換方針、prefetch は探索エンジンの責務である。

探索の終端では `repetition_state()` を確認し、エンジンのスコア規約へ変換する。
`RepetitionState::Win` と `RepetitionState::Lose` は手番側から見た連続王手千日手の結果なので、色固定の評価へ変換するときは手番を取り違えない。

## tactical cache の利用

`current_state_cache()` から得る view は、チェック、ピン、王手候補をまとめて読むために使える。
view を保持したまま `Position` を mutable に更新することは Rust の借用規則でできない。
子局面を apply または undo した後は、新しい current state から view を取得し直す。

| Position の情報 | 探索での典型的な用途 |
|---|---|
| `key()` | 現在ノードの置換表 probe。 |
| `key_after(mv)` | 子ノードの事前 probe。 |
| `repetition_state()` | 反復終端のスコア化。 |
| `checkers()` | 王手延長や終端の補助。 |
| `last_move()` | continuation history の索引。 |
| `MoveApplyFacts` | 評価差分と検索スタックの更新。 |

## 容量を固定する探索

検索開始前に `prepare_search_state_capacity` を呼ぶと、指定した深さまで state slot を準備できる。
その後は `try_apply_search_move32_with_facts` と null move 用 API を使い、容量不足を `MoveError` として扱える。
この方式は allocation の時点を検索ループの外へ出したいエンジンに適する。

## 次に読む

→ [合法手生成](../movegen/index.md) で、探索目的に応じた generator mode を選ぶ。
