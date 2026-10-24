# StateStack と StateInfo の設計

探索中に指し手を進めて（`apply_move32`）から元の局面へ戻す（`undo_move32`）際、捕獲した駒や Zobrist ハッシュ、千日手カウンタ、ピン情報などは 1 手前の状態へ正確に復元されなければならない。
`rsshogi` では、これらの履歴情報を `crates/rsshogi/src/board/state_info.rs` の `StateStack` と `StateInfo` で管理している。

---

## 1. `StateStack` の 64 要素チャンク分割ストレージ

単純な `Vec<StateInfo>` で履歴スタックを実装すると、探索深さや長手数の棋譜で `Vec` の容量（capacity）を超えた瞬間にヒープの再確保と全要素のメモリコピーが発生し、さらに既存要素への参照アドレスが無効化される。
`StateStack` は、固定長 64 要素の配列を `Box` に格納するセグメント構造を採用している。
履歴は添字で参照し、着手で head を進め、undo で戻す。

```rust
pub struct StateStack {
    repetition_index: Option<Arc<RepetitionIndex>>,
    chunks: Vec<Box<[StateInfo; 64]>>,
    head: StateIndex,
}
```

### セグメント構造の利点

1. **添字による直接参照**：深さ `index` の要素をチャンクとチャンク内の位置に分けて参照する。
   千日手判定も同じ添字を使い、同一手番の履歴を直接読む。
2. **アドレス安定性と容量の再利用**：新しいチャンクが増えても既存の配列は移動しない。
   undo 後も確保済みチャンクを保持し、事前確保した範囲では追加確保を行わない。
3. **必要な履歴の独立した複製**：`clone_for_search()` は null move 以降の保持対象を複製する。
   通常の `clone()` は全履歴と予約容量を維持する。

通常の `Position::clone()` は boxed chunk を直接複製し、履歴を私有する独立した局面を作る。
`clone_for_search()` が構築する board key の索引は clone 間で共有するが、履歴本体は共有しない。
索引に一致候補がない場合だけ根以前の走査を省き、一致した場合は従来の詳細判定を行う。

`Position::clone_from(&source)` は複製先の boxed chunk を再利用し、有効な履歴だけを上書きする。
複製元の予約容量を収容できるだけの chunk があれば、新たに確保しない。
不足する chunk は確保し、複製先に余る chunk は以後の再利用のために保持する。
盤面、全履歴、入玉設定、反復索引は複製元に合わせて置き換える。
複製先に以前の view や pointer を保持している場合は、状態の意味が変わるため再取得を要する。

`set_max_repetition_ply()` は 1.6.0 から非推奨で、2.0.0 で削除予定である。
既定値は `Ply::MAX` であり、長い履歴を自動的に短縮しない。
明示的な上限を下げると長周期の千日手や連続王手を見逃し得るため、その近似方針は consumer 側で管理する。
`clone_for_search()` 自体は引き続き提供する。

全有効履歴を残して予約容量だけを引き継ぎたくない場合は `clone_compact()` を使う。
head を含む最終チャンクまでを複製し、末尾の未使用 slot の内容は有効履歴として扱わない。
null move より前の保持履歴も残し、保持範囲内の undo と再分岐を双方で独立して行える。
既存の索引は不変共有するが、索引の新規構築は行わない。
追加着手でチャンクの確保が必要になる場合がある。
4 操作の違いは [独立した探索用コピー](search-integration.md#4-独立した探索用コピー) を参照する。

---

## 2. `StateHot` と `StateColdPayload`（`UndoPayload`）の分離

1 つの局面状態を表す `StateInfo` は、探索ノードごとに毎回アクセスされる **ホットデータ（`StateHot`）** と、指し手を巻き戻す（`undo_move32`）ときや直前手を参照するときにしか使われない **コールドデータ（`StateColdPayload`）** に内部構造を分離している。

```rust
#[derive(Clone, Copy, Debug)]
pub(crate) struct StateInfo {
    pub(crate) cold: StateColdPayload,
    pub(crate) hot: StateHot,
}
```

### `StateHot` と `StateColdPayload` のフィールド一覧

| 区分 | フィールド名 | 型 | 内容とアクセス頻度 |
| :--- | :--- | :--- | :--- |
| **`StateHot`** | `key` | `ZobristKey` | 手番と持ち駒を含む局面全体の完全 Zobrist ハッシュ（毎ノード参照）。 |
| **`StateHot`** | `board_key` | `ZobristKey` | 盤上の駒配置と手番のみのハッシュ（同一盤面の優劣比較や千日手検査で毎ノード参照）。 |
| **`StateHot`** | `partial_keys` | `PartialKeys` | 歩ハッシュ `pawn` と歩以外の駒割ハッシュ `non_pawn_material: [ZobristKey; 2]`。 |
| **`StateHot`** | `repetition_count` | `u8` | 同一局面が出現した回数（千日手判定用）。 |
| **`StateHot`** | `repetition_ply` | `u16` | 前回同一局面が出現した時点からの経過手数。 |
| **`StateHot`** | `plies_from_null` | `u16` | 直近の初期局面または Null Move からの連続手数（履歴遡及の上限）。 |
| **`StateHot`** | `tactical` | `TacticalCache` | 王手駒、絶対ピン、ピンナー、および王手可能マス（指し手生成と合法性検査で毎ノード参照）。 |
| **`StateHot`** | `continuous_check` | `[u8; 2]` | 先後それぞれの連続王手回数（連続王手の千日手判定用）。 |
| **`StateHot`** | `hand` | `Hand` | 現手番側の持ち駒スナップショット（千日手遡及時の優劣比較用）。 |
| **`StateHot`** | `repetition_type` | `RepetitionState` | 現局面の千日手および優劣判定結果キャッシュ。 |
| **`StateColdPayload`** | `captured` | `Piece` | 直前の手で捕獲された駒（`undo_move32` で盤面に戻す際にのみ参照）。 |
| **`StateColdPayload`** | `last_move` | `Move` | 直前に指された 16 ビット指し手。 |
| **`StateColdPayload`** | `last_moved_piece_type` | `PieceType` | 直前に移動または打たれた後の駒種。 |

`apply_move32` で新しい `StateInfo` を構築する際、まず `StateStack::push_Prepared` によって前局面の `hot` から新しい `hot` の骨格を引き継ぎ、捕獲駒や直前手をまとめた `UndoPayload`（`StateColdPayload`）は一括代入で書き込む。
このようにアクセス頻度の異なるフィールドを構造体レベルで分離することで、キャッシュラインの利用効率とコードの見通しを高めている。

---

## 3. `TacticalCache` と `CheckSquares` の 9 スロット圧縮

`StateHot` 内の `tactical: TacticalCache` は、王手回避やピン判定、および王手生成で繰り返し参照される以下の 4 種類のビットボード群を保持する。

| フィールド名 | 型 | 内容 |
| :--- | :--- | :--- |
| `checkers` | `Bitboard` | 現在手番側の玉に王手をかけている敵駒の位置集合。 |
| `blockers_for_king` | `[Bitboard; 2]` | 先後それぞれの玉に対して絶対ピン（間駒が 1 枚のみ）になっている駒の位置集合。 |
| `pinners` | `[Bitboard; 2]` | 先後それぞれの玉に対してピンをかけている敵の飛び駒（飛、角、香、馬、龍）の位置集合。 |
| `check_squares` | `CheckSquares` | 手番側の各駒種が移動したときに、敵玉へ直接王手をかけられる移動先マスの集合。 |

### `CheckSquares([Bitboard; 9])` のスロット共有

`PieceType` は全部で 16 個（実駒 14 種）あるが、**金（`GOLD`）と 4 種類の成金（`PRO_PAWN`、`PRO_LANCE`、`PRO_KNIGHT`、`PRO_SILVER`）は利きが完全に同一であるため、敵玉に対して直接王手をかけられるマスの集合も完全に一致する**。
また、`NONE`、`KING`（玉が敵玉に接近して直接王手することはルール上不可能）、および `GOLD_LIKE` には専用の王手マステーブルを持つ必要がない。

そこで `CheckSquares` は、16 個ではなく **9 個の `Bitboard` 配列（`[Bitboard; 9]`、144 バイト）** のみを保持し、成金 4 種をすべて金のスロット（インデックス `6`）へ集約している。

| `CheckSquares` スロット | 対応する `PieceType` | 計算方法 |
| :--- | :--- | :--- |
| `0`（`CHECK_SQ_PAWN`） | `PAWN`（歩） | 敵玉の位置から逆向き（敵番視点）の `pawn_attacks` |
| `1`（`CHECK_SQ_LANCE`） | `LANCE`（香） | 敵玉の位置から逆向きの `lance_attacks` |
| `2`（`CHECK_SQ_KNIGHT`） | `KNIGHT`（桂） | 敵玉の位置から逆向きの `knight_attacks` |
| `3`（`CHECK_SQ_SILVER`） | `SILVER`（銀） | 敵玉の位置から逆向きの `silver_attacks` |
| `4`（`CHECK_SQ_BISHOP`） | `BISHOP`（角） | 敵玉の位置からの `bishop_attacks` |
| `5`（`CHECK_SQ_ROOK`） | `ROOK`（飛） | 敵玉の位置からの `rook_attacks` |
| `6`（`CHECK_SQ_GOLD`） | `GOLD`, `PRO_PAWN`, `PRO_LANCE`, `PRO_KNIGHT`, `PRO_SILVER` | 敵玉の位置から逆向きの `gold_attacks`（5 駒種で共有） |
| `7`（`CHECK_SQ_HORSE`） | `HORSE`（馬） | スロット `4`（角） `\| king_attacks(enemy_king)` |
| `8`（`CHECK_SQ_DRAGON`） | `DRAGON`（龍） | スロット `5`（飛） `\| king_attacks(enemy_king)` |

この 9 スロット圧縮により、毎ノードの `StateInfo` のメモリサイズを 7 × 16 = 112 バイト削減すると同時に、`update_check_squares` における金の王手マス計算とメモリ書き込みを 5 回から 1 回に減らしている。

---

## 4. Null Move（パス手）の 2 経路設計

手番だけを相手に渡す Null Move（パス手）には、用途に応じて **`ply`（対局手数）を進める経路** と **`ply` を進めない探索専用経路** の 2 種類が実装されている。
内部的には共通のジェネリック実装 `apply_null_move_impl<const ADVANCE_GAME_PLY: bool>` と `undo_null_move_impl<const REWIND_GAME_PLY: bool>` で処理する。

| API ペア | 定数パラメータ | 動作の違いと主な用途 |
| :--- | :--- | :--- |
| `apply_null_move` / `undo_null_move` | `true` | 対局手数を `+1` / `-1` する。王手がかかっていない局面で適用する。戻り値は `Result<(), MoveError>`。 |
| `try_apply_search_null_move` / `undo_search_null_move` | `false` | 対局手数を変えない。適用には予約済み slot と王手がかかっていない局面が必要。戻り値は `Result<(), MoveError>` で、容量不足は `StateCapacityExceeded`。1.6.0 から非推奨で、2.0.0 で削除予定。 |

事前確保した容量内の無確保動作は、通常の `apply_move32` と undo でも検査している。
容量確保 API は維持し、探索専用の着手経路とは廃止対象を分ける。

いずれの経路でも、新しい `StateInfo` が `StateStack` にプッシュされ、`key` と `board_key` には手番ハッシュ `Zobrist::side()` が XOR され、`plies_from_null` と `continuous_check` は `0` にリセットされる。
これにより、Null Move を挟んだ前後の局面で誤って連続王手や不正な千日手遡及が発生しないことが保証される。
