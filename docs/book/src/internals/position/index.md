# Position と状態管理

`Position`（`crates/rsshogi/src/board/position/mod.rs`）は、将棋の盤面状態、持ち駒、手番、手数、および局面履歴を統合管理する中心的な構造体である。
探索エンジンが毎秒数百万ノードの速度で指し手の適用（`apply_move32`）と巻き戻し（`undo_move32`）を繰り返せるよう、内部データは用途別に特化した 5 つのレイヤーで多重管理されている。

---

## `Position` の 5 層データモデル

`Position` は単一の盤面配列だけを持つのではなく、クエリの種類ごとに計算量が O(1) となるよう以下の 5 つのデータ構造を同期させている。

```
Position
 ├── 1. board: SquareTable<Piece, 81>        (マス sq から駒 Piece を引く O(1) 配列)
 ├── 2. bitboards: BitboardSet               (駒種・陣営・複合キャッシュからマス集合を引く O(1) 集合)
 ├── 3. hands: [Hand; 2]                     (先後の持ち駒枚数を 32 ビット整数 2 個で管理)
 ├── 4. king_squares: [Option<Square>; 2]    (先後の玉の位置をキャッシュ、玉不在の詰将棋にも対応)
 └── 5. state_stack: StateStack              (ハッシュ値、王手・ピン情報、巻き戻し情報を保持する履歴スタック)
      (+ side_to_move: Color, ply: u16)
```

| レイヤー | フィールド名と型 | 解決するクエリと役割 |
| :--- | :--- | :--- |
| **1. 盤面配列** | `board: SquareTable<Piece, 81>` | 「指定したマス `sq` にどの駒があるか（`piece_on(sq)`）」を 1 回のメモリアクセスで取得する。 |
| **2. ビットボード集合** | `bitboards: BitboardSet` | 「どのマスに指定の駒種や陣営の駒があるか」を 128 ビット集合として保持し、6 種類の複合キャッシュで利き計算を高速化する。 |
| **3. 持ち駒配列** | `hands: [Hand; 2]` | 先手と後手の全持ち駒を各 32 ビット整数にパックし、加減算と SWAR 優劣比較を行う。 |
| **4. 玉位置キャッシュ** | `king_squares: [Option<Square>; 2]` | 王手判定やピン検出の基点となる両玉の座標を常時キャッシュする。片玉の詰将棋局面も表現できるよう `Option<Square>` としている。 |
| **5. 状態履歴スタック** | `state_stack: StateStack` | 現局面の Zobrist ハッシュ、戦術キャッシュ（王手駒、ピン情報、王手可能マス）、および `undo_move32` 用の捕獲駒情報を保持する。 |

これら 5 つのレイヤーは、`put_piece`、`remove_piece`、`move_piece` の内部プリミティブを通じてアトミックに更新される。

---

## 指し手適用の 2 系統：`apply_move32` と `try_apply_move32`

`Position` 上で指し手を進める API には、呼び出し元の信頼境界に応じてパニック系と `Result` 返却系の 2 系統が用意されている。

| メソッド名 | 戻り値 | 特徴と用途 |
| :--- | :--- | :--- |
| `apply_move(mv)` / `apply_move32(mv)` | `()` | 指し手が合法であることを前提とする探索ループ向け API。内部で `try_apply_move32` を呼び、不正な指し手に対しては `.expect(...)` でパニックする。 |
| `try_apply_move(mv)` / `try_apply_move32(mv)` | `Result<(), MoveError>` | 外部入力や棋譜再生など、不正な指し手が混入し得る境界向け API。盤面を変更する前に移動元、移動先、持ち駒、上位駒ビットの整合性を検証し、違反時は `Err(MoveError)` を返す。 |
| `try_apply_move32_with_facts(mv)` | `Result<MoveApplyFacts, MoveError>` | 局面更新と同時に、捕獲駒、王手や成りのフラグ、更新後のキー、巻き戻し用の差分などをまとめた `MoveApplyFacts` を返す。 |
| `apply_move32_with_gives_check(mv, gives_check)` | `()` | 呼び出し側（指し手生成や `gives_check` 事前判定）でその手が王手になるか否かがすでに判明している場合に、局面更新時の王手再計算を省略する高速パス。 |

---

## 内部整合性の検証（`validation` feature）

5 つのデータレイヤーと `StateStack` のキャッシュが完全に同期しているかを検査するため、`validation` Cargo feature が有効なときには以下の検証メソッドが公開される。

- **`Position::validate(&self) -> Result<(), ValidationError>`**：現在の局面について、`board` 配列と `bitboards` の一致、玉の枚数と `king_squares` の一致、持ち駒枚数の上限とボロービット異常の有無、行き所のない駒や二歩の有無、手番でない側の玉に王手がかかっていないこと、および Zobrist ハッシュや戦術キャッシュの再計算結果との一致を検証する。
- **`Position::validate_all(&self) -> Result<(), ValidationError>`**：現局面だけでなく、`StateStack` に積まれている過去の全履歴を巻き戻しながら各局面の整合性と反復情報（千日手カウンタ）の正確性を網羅検査する。
- **`Position::is_valid(&self) -> bool`**：`validate().is_ok()` の真偽値を返す。

---

## 本セクションの構成

- **[StateStack と StateInfo の設計](state-management.md)**：64 要素チャンクによる `StateStack` のヒープ再確保回避、`StateHot` と `StateColdPayload` の分離、`TacticalCache` と `CheckSquares` の 9 スロット圧縮、および Null Move の 2 経路を解説する。
- **[Zobrist ハッシュと差分更新](zobrist.md)**：`StateHot` を唯一の真実の源（Single Source of Truth）とする O(1) 巻き戻し、128 ビットハッシュとの乱数列互換性を保つ 4 回抽選 PRNG、および 72 スロット持ち駒テーブルを解説する。
- **[探索エンジン連携プリミティブ](search-integration.md)**：`MoveApplyFacts`、`StateCacheView`、consumer 側のキー予測と prefetch の責務、千日手や連続王手の判定、および `clone_for_search()` による独立した探索用コピーを解説する。
