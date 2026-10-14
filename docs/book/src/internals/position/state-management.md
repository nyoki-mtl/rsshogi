# StateStack と StateInfo の設計

探索中に指し手を進めて（`apply_move32`）から元の局面へ戻す（`undo_move32`）際、捕獲した駒や Zobrist ハッシュ、千日手カウンタ、ピン情報などは 1 手前の状態へ正確に復元されなければならない。
`rsshogi` では、これらの履歴情報を `crates/rsshogi/src/board/state_info.rs` の `StateStack` と `StateInfo` で管理している。

---

## 1. `StateStack` の 64 要素チャンク分割ストレージ

単純な `Vec<StateInfo>` で履歴スタックを実装すると、探索深さや長手数の棋譜で `Vec` の容量（capacity）を超えた瞬間にヒープの再確保と全要素のメモリコピーが発生し、さらに既存要素への参照アドレスが無効化される。
`rsshogi` の `StateStack` は、固定長 64 要素の配列チャンク `Box<[StateInfo; 64]>`（`STATE_CHUNK_CAPACITY = 64`）を束ねたセグメント構造を採用している。

```rust
pub(crate) const STATE_CHUNK_CAPACITY: usize = 64;
const STATE_CHUNK_SHIFT: usize = 6;
const STATE_CHUNK_MASK: usize = STATE_CHUNK_CAPACITY - 1;

pub(crate) struct StateStack {
    chunks: Vec<Box<[StateInfo; STATE_CHUNK_CAPACITY]>>,
    len: usize,
}
```

### セグメント構造の利点

1. **インデックス計算がビットシフトとマスクのみで完結する**：チャンクサイズが 2 のべき乗（`64 = 2^6`）であるため、深さ `index` の要素は `chunks[index >> 6][index & 63]` で除算なしに直接参照できる。
2. **アドレス安定性とヒープ再確保の排除**：新しいチャンクが必要になった場合でも既存の `Box<[StateInfo; 64]>` は一切移動しない。また、`pop()` 時は `self.len -= 1` とするだけで確保済みのチャンクを解放せずに保持するため、探索木の昇降でヒープアロケーションは一度も発生しない。
3. **履歴末尾の高速部分複製（`clone_suffix`）**：並列探索のワーカー起動や 1 手詰めソルバの作業局面生成で使われる `Position::clone_for_search_bounded(max_back_plies)` は、`StateStack::clone_suffix` を呼び出す。何百手もの全対局履歴をコピーするのではなく、直近の必要な手数分（および `plies_from_null` の範囲）だけを新しい `StateStack` へ切り出してコピーし、巻き戻し不可能な過去のチャンク複製コストを省いている。

---

## 2. `StateHot` と `StateColdPayload`（`UndoPayload`）の分離

1 つの局面状態を表す `StateInfo` は、探索ノードごとに毎回アクセスされる **ホットデータ（`StateHot`）** と、指し手を巻き戻す（`undo_move32`）ときや直前手を参照するときにしか使われない **コールドデータ（`StateColdPayload`）** に内部構造を分離している。

```rust
#[derive(Clone, Copy, Debug)]
pub(crate) struct StateInfo {
    pub(crate) hot: StateHot,
    pub(crate) cold: StateColdPayload,
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
内部的には共通のジェネリック実装 `apply_null_move_internal<const ADVANCE_GAME_PLY: bool>` / `undo_null_move_internal<const ADVANCE_GAME_PLY: bool>` によって分岐なしでコンパイルされる。

| API ペア | 定数パラメータ | 動作の違いと主な用途 |
| :--- | :--- | :--- |
| `apply_null_move` / `undo_null_move` | `ADVANCE_GAME_PLY = true` | `self.ply` を `+1` / `-1` する。王手がかかっている局面での呼び出しはアサーション違反となる。外部からの手番反転テストや汎用パス処理で使用する。 |
| `try_apply_search_null_move` / `undo_search_null_move` | `ADVANCE_GAME_PLY = false` | **`self.ply` を一切変化させない**。王手中は `false` を返して安全に棄却する。探索エンジンの Null Move Pruning において、実対局の手数（`ply`）を汚さずに手番だけを切り替えるために使用する。 |

いずれの経路でも、新しい `StateInfo` が `StateStack` にプッシュされ、`key` と `board_key` には手番ハッシュ `Zobrist::side()` が XOR され、`plies_from_null` と `continuous_check` は `0` にリセットされる。
これにより、Null Move を挟んだ前後の局面で誤って連続王手や不正な千日手遡及が発生しないことが保証される。
