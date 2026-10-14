# 探索エンジン連携プリミティブ

`Position` は、単にルールに従って局面を進めるだけでなく、Alpha-Beta 探索や MCTS（モンテカルロ木探索）のホットパスで必要となる情報を最小コストで引き出すための専用プリミティブを備えている。
本ページでは、`MoveFacts`、`StateCacheView`、`key_after_move` と投機的プリフェッチ、千日手や連続王手判定、および探索用クローン（`clone_for_search_bounded`）の設計を解説する。

---

## 1. `MoveFacts` と `StateCacheView`

探索ノードで `try_apply_move32_with_facts` を呼び出すと、局面更新の中で自然に判明した属性が `MoveFacts` 構造体として返される。

```rust
pub struct MoveFacts {
    pub move32: Move32,
    pub moved_piece_after: Piece,
    pub captured_piece: Option<Piece>,
    pub gives_check: bool,
    pub is_drop: bool,
    pub is_promotion: bool,
}
```

探索エンジンは、Late Move Reductions（LMR）や Futility Pruning、Check Extension の条件判定において「今の指し手が駒取りだったか」「成りだったか」「王手になったか」を毎回チェックする。
`MoveFacts` を受け取ることで、局面更新後に改めて `Position` の状態や指し手ビットをデコードし直す重複計算を省ける。

また、`Position::state_cache_view()` が返す `StateCacheView` は、現在の `StateHot` に格納されているハッシュ値群（`key`、`board_key`、`pawn_key`、`non_pawn_material`）、戦術ビットボード（`checkers`、`blockers_for_king`、`pinners`）、直前手（`last_move`）、捕獲駒（`captured_piece`）、連続王手カウント（`continuous_check`）、および `plies_from_null` を 1 つの読み取り専用ビューとして提供する。

---

## 2. `key_after_move` と置換表の投機的プリフェッチ（`prefetch`）

大規模な置換表（数 GB〜数十 GB）は CPU の L3 キャッシュに収まらず、メインメモリ（DRAM）へのアクセスには約 100〜200 クロックサイクルのレイテンシがかかる。
`Position::key_after_move(mv)`（または `key_after_move32(mv)`）は、**実際に `apply_move32` で盤面や `BitboardSet` を更新することなく、その指し手を指した後の Zobrist ハッシュ値（`key`）だけを数命令の XOR で先行算出する**。

```rust
let next_key = pos.key_after_move32(mv);
tt.prefetch(next_key); // メモリコントローラへキャッシュラインの先読みを発行

// プリフェッチが裏で進行している間に、王手判定や局面更新を実行する
pos.apply_move32(mv);
```

このように `apply_move32` の重い処理（ビットボードの更新、ピンや王手マスの再計算）を実行する前に `key_after_move` で次局面のキーを求めて `_mm_prefetch` 等を発行することで、メモリアクセスのレイテンシを局面更新の計算時間とオーバーラップさせて隠蔽できる。

---

## 3. 千日手、連続王手、優劣局面の高速判定

`crates/rsshogi/src/board/position/rules.rs` における反復局面判定（`update_repetition` / `repetition_state`）は、盤面ハッシュ `board_key` と `StateHot` の履歴メタデータを組み合わせて高速化されている。

### 遡及範囲の限定と 2 手おきの走査

駒打ちや盤上移動では手番が毎回反転するため、同一手番の同一盤面（`board_key` が等しい局面）は必ず **偶数手前（2 手前、4 手前、6 手前...）** にしか出現しない。
さらに、Null Move を挟んだ過去には千日手や連続王手が成立しないため、探索すべき上限手数は `max_back = state.plies_from_null.min(current_idx)`（および呼び出し側が指定した `max_ply`）に制限される。

```
現局面 (idx) ──[2 手前 (idx - 2)]──[4 手前 (idx - 4)]── ... ──[上限: idx - max_back]
```

### 判定ロジックの優先順位

過去の局面 `prev`（`step = 2, 4, 6, ...`）を遡る際、`prev.board_key == current.board_key` が一致した時点で以下の判定を行う。

1. **完全同一局面（`prev.key == current.key`、すなわち持ち駒も完全に同一）**：
   - `repetition_count` を `prev.repetition_count + 1` に更新する。
   - **連続王手の千日手判定**：現局面までの連続王手手数 `continuous_check[us] * 2 >= step` であれば、手番側が王手をかけ続けて同一局面に達しているため反則負け（`RepetitionState::Lose`）となる。逆に相手側の `continuous_check[them] * 2 >= step` であれば相手側の反則により勝ち（`RepetitionState::Win`）となる。
   - 連続王手でなければ通常の千日手（`RepetitionState::Draw`）となる。なお、公式ルール上の千日手成立（同一局面 4 回目、すなわち `repetition_count >= 3`）は `Position::is_repetition(max_ply)` で判定でき、探索中に 2 回目の出現時点で即座に打ち切る場合は `Position::repetition_state(max_ply)` を用いる。
2. **同一盤面で持ち駒のみが異なる局面（`prev.key != current.key`）**：
   - `Hand` のボロービットを用いた [SWAR 一括優劣比較](../types/pieces.md) により、手番側の持ち駒 `current.hand` と `prev.hand` を比較する。
   - 全駒種で `current.hand >= prev.hand` なら **優等局面（`RepetitionState::Superior`）**、全駒種で `current.hand <= prev.hand` なら **劣等局面（`RepetitionState::Inferior`）** と判定する。

---

## 4. 探索用軽量クローン：`clone_for_search_bounded`

並列探索（Lazy SMP や YBWC）でスレッドごとに `Position` を複製したり、探索ノードの途中で 1 手詰めソルバ等のサブ探索用に作業用 `Position` を作ったりする場合、通常の `Position::clone()` を呼ぶと対局開始からの全履歴チャンクがコピーされてしまう。

`Position::clone_for_search_bounded(max_back_plies)` は、巻き戻しや千日手判定に必要な直近の履歴上限 `keep_back = max_back_plies.max(plies_from_null)` を算出し、`StateStack::clone_suffix(keep_back + 1)` によって末尾の必要な `StateInfo` だけを持つ軽量な `Position` を生成する。
例えば 1 手詰めソルバ（`mate.rs`）では `pos.clone_for_search_bounded(16)` を用いており、対局が 200 手進んでいてもわずか 1 チャンク以内のコピーで作業局面を構築できる。
