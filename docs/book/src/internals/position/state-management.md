# ゲームステート管理

> **前提知識**: [Position 構造体](./index.md)（Position の役割と基本設計）

## このページの要点

- 探索中の局面の保存と復元には **StateInfo**（差分情報のみ保持）を使い、Position 全体のコピーを避ける
- StateInfo は `StateStack` でスタック管理し、再帰的な探索木と対応する
- **Make-Unmake パターン**を採用: `apply_move32(mv)` で差分更新し `undo_move32(mv)` で復元する
- rsshogi の `StateStack` は segmented storage を採用し、stack 伸長時も live entry の address stability を保つ
- `StateStack` は `Position` が内部管理するため、外部から `StateInfo` を渡す必要はない
- rsshogi の StateInfo は**ルール判定に必要な情報のみ**を管理する。評価差分は探索エンジン側で管理する（→ [探索エンジンとの統合](./search-integration.md)）

## 歴史的背景: Copy-Make vs Make-Unmake

局面の状態管理には歴史的に 2 つの流派があります。

**Copy-Make**（全体コピー）: 指し手を適用する前に Position 全体をクローンし、探索後に捨てる方式。
実装が単純で正確だが、Position が大きいほどコピーコストが支配的になります。
初期のチェスプログラムや一部の教育的実装で使用されました。

**Make-Unmake**（差分更新+復元）: 変更された情報だけを StateInfo に退避し、
`undo_move32()` で復元する方式。多くの高速エンジンが採用しています。
rsshogi もこの方式です。

## なぜ StateInfo が必要なのか

### 問題：Position全体をコピーすると遅い

探索では、指し手を試すたびに局面を更新・復元する必要があります。
単純な実装では Position 全体をコピーすることになりますが、これには大きな問題があります：

```rust,ignore
// ❌ 非効率な実装
fn search(position: Position, depth: i32) -> i32 {
    for mv in generate_moves(&position) {
        // Position全体をコピー（数百バイト〜数KB）
        let mut new_position = position.clone();
        new_position.apply_move32(mv);

        let score = -search(new_position, depth - 1);
        // ...
    }
}
```

この実装の問題点：

1. **メモリコピーのコスト**: Position構造体は盤面、持ち駒、ハッシュ値など多くの情報を含む
2. **キャッシュ効率の悪化**: 大量のメモリコピーがCPUキャッシュを汚染する
3. **探索の深さに比例して遅くなる**: 深い探索では毎秒数百万回のコピーが発生

### 解決策：差分更新と StateInfo

Position をコピーする代わりに、**変化した情報だけを保存**します。
これが `StateInfo` の役割です。

```rust,ignore
// ✅ 効率的な実装
fn search(position: &mut Position, depth: i32) -> i32 {
    for mv in generate_moves(position) {
        // StateInfo は内部の StateStack で自動管理される
        position.apply_move32(mv);

        let score = -search(position, depth - 1);

        // 保存した情報を使って復元
        position.undo_move32(mv).expect("move applied in search must undo cleanly");
    }
}
```

## StateInfo の設計

### 保存すべき情報

`StateInfo` には、指し手を戻すために必要な情報を保存します：

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/board/state_info.rs:state_info_struct}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/board/state_info.rs#L15-L94)</small>

`StateIndex` は `usize` の型エイリアスで、リンクドリストではなくインデックスベースの参照を使用します。

### なぜスタック形式なのか

探索は再帰的に実行されるため、StateInfo も階層的に管理します：

```text
局面A（初期局面）
  ├─ StateInfo-1（7g7f を適用）
  │   └─ 局面B
  │       ├─ StateInfo-2（3c3d を適用）
  │       │   └─ 局面C
  │       │       └─ ...
  │       └─ StateInfo-3（8c8d を適用）
  │           └─ 局面D
  │               └─ ...
  └─ ...
```

`Position` は current state の index (`st_index`) を保持し、`StateStack` 側は
stack top (`head`) を進めながら履歴を管理します。`undo_move32()` は stack を
1 つ pop し、直前の `StateInfo` から board key / hand key などを直接復元します。

## apply_move32() と undo_move32() のパターン

### 基本的な使い方

rsshogi では `apply_move32()` は `StateInfo` を内部の `StateStack` で自動管理するため、
外部から `StateInfo` を渡す必要はありません。

```rust,ignore
// 実際の API シグネチャ
impl Position {
    pub fn apply_move32(&mut self, mv: Move32);
    pub fn undo_move32(&mut self, mv: Move32) -> Result<(), MoveError>;
}
```

内部では以下の処理が行われます（概念的な擬似コード）：

```rust,ignore
// 概念的な擬似コード（実際の内部実装を簡略化）
fn apply_move32_internal(&mut self, mv: Move32) {
    // 1. StateStack に next slot を確保し、carry-over が必要な最小フィールドを準備
    //    - plies_from_null
    //    - continuous_check
    //    - captured = Piece::NONE

    // 2. 指し手を適用
    let moved_piece = self.board[mv.from_sq()];
    self.board[mv.from_sq()] = Piece::NO_PIECE;
    self.board[mv.to_sq()] = if mv.is_promotion() {
        moved_piece.promote()
    } else {
        moved_piece
    };

    // 3. 取った駒があれば持ち駒に追加
    let captured = state.captured_piece();
    if captured != Piece::NONE {
        self.add_to_hand(captured.demote());
    }

    // 4. hash / repetition / tactical caches を current state に反映
}

fn undo_move32_internal(&mut self, mv: Move32) {
    // 1. StateStack から情報を取り出す
    // 2. 移動した駒を元の位置に戻す
    // 3. 取った駒があれば盤面に戻す
    // 4. hash などの状態を直前 state から直接復元（逆演算ではなく直接復元）
}
```

### 探索での使用例

```rust,ignore
fn alpha_beta(
    position: &mut Position,
    alpha: i32,
    beta: i32,
    depth: i32
) -> i32 {
    // 終端ノードの評価
    if depth == 0 {
        return evaluate(position);
    }

    let mut best_score = -INFINITY;
    let moves = generate_moves(position);

    for mv in moves {
        // 指し手を適用（StateInfo は内部で自動管理）
        position.apply_move32(mv);

        // 再帰的に探索
        let score = -alpha_beta(position, -beta, -alpha, depth - 1);

        // 指し手を戻す
        position.undo_move32(mv);

        // ベストスコアを更新
        if score > best_score {
            best_score = score;
        }

        // α-βカット
        if score >= beta {
            break;
        }
    }

    best_score
}
```

## パフォーマンス上の利点

### メモリ使用量の比較

Position 全体をコピー:
  約 200-500 バイト × 探索ノード数

```text
StateInfo のみ保存:
  約 32-64 バイト × 探索ノード数
```

探索深度20の場合、数十億ノードを探索するため、この差は非常に大きくなります。

### キャッシュ効率

StateInfo は小さいため、CPUキャッシュに乗りやすくなります：

L1キャッシュ（32KB）に収まる StateInfo の数:
  32KB / 64バイト = 約 500個

```text
L1キャッシュに収まる Position の数:
  32KB / 400バイト = 約 80個
```

差分更新により、同じキャッシュサイズでより多くの情報を保持できます。

## StateStack パターン

rsshogi では、`StateStack` が chunk 単位の segmented storage で `StateInfo` を管理します。
Position は `StateStack` への参照を通じて現在局面と履歴にアクセスします。

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/board/state_info.rs:state_stack_struct}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/board/state_info.rs#L119-L131)</small>

`Vec<StateInfo>` をそのまま伸ばす方式と比べ、segmented storage なら grow 時に
既存 chunk が再配置されません。これにより、同一 `StateStack` インスタンス内の
live entry については address stability を説明できます。

ただし、次は契約外です。

- `pop()` / `undo_*()` で current から外れた entry を current state として使い続けること
- `reset_sfen()` / `reset_with_position()` 後に古い entry を参照すること
- `clone()` / `clone_for_search()` をまたいで pointer identity を期待すること

## Null Move の扱い

Null Move Pruning では、手番をパスする特殊な指し手を使います。
この場合も `StateInfo` を使って状態を保存します。実装では current state を clone した
next slot を用意し、null move で必ず確定するフィールドを stack 側で初期化します。

```rust,ignore
impl Position {
    /// Null Move（手番パス）を適用
    pub fn apply_null_move(&mut self) -> Result<(), MoveError> {
        // 1. next slot を準備し、null move 固有の不変条件を初期化
        //    - plies_from_null = 0
        //    - captured = Piece::NONE
        //    - checkers = Bitboard::EMPTY
        //    - repetition_* = 0 / None

        // 2. 手番だけを切り替え、board_key と zobrist を更新
        self.flip_side_to_move();

        // 3. check_squares と continuous_check を current state に反映
        Ok(())
    }

    /// Null Move を戻す
    pub fn undo_null_move(&mut self) -> Result<(), MoveError> {
        // StateStack を pop し、直前 state の hash と turn を復元
        Ok(())
    }
}
```

## 将棋固有の考慮事項

チェスの Make-Unmake と比較して、将棋には以下の追加要素があります。

| 項目 | チェス | 将棋 |
|------|--------|------|
| 持ち駒 | なし | あり → StateInfo に持ち駒の変化も記録 |
| 成り | Pawn のみ → 変身先が複数 | 6 駒種 → 成り/不成の選択が StateInfo に影響 |
| 打ち歩詰め | なし | あり → apply_move32 時の追加チェック |
| 千日手 | 50手ルールで打ち切り | ハッシュ履歴の完全保持が必要 |

特に持ち駒の管理は、チェスにはない将棋固有の複雑さです。
駒を取ったとき成駒を元に戻す（demote）処理と、`undo_move32` 時に持ち駒から戻す処理が必要になり、
StateInfo に保存すべき情報量が増えます。

## 落とし穴

### StateInfo の生存期間

StateInfo は `apply_move32()` から対応する `undo_move32()` まで有効でなければなりません。
safe Rust では `current_state_cache()` などの state 由来の borrow 中に mutation できません。
一方、raw pointer として保持する場合は「物理アドレスの安定性」と「current state を表す意味」を
分けて扱う必要があります。stack 伸長でアドレスは維持されても、`apply_move32()` /
`apply_null_move()` 後は以前の entry は current state ではありません。

### apply/undo の非対称性

`apply_move32()` は Zobrist ハッシュを差分更新しますが、`undo_move32()` は StateInfo から**直接復元**します（逆演算ではない）。
このため、apply と undo では処理が対称ではありません。undo で XOR を逆順に適用するような実装は誤りです。

### Null Move 後に通常の undo_move32 を呼ぶ

`apply_null_move()` は盤面を変更せず手番だけを切り替えます。
`undo_null_move()` ではなく通常の `undo_move32()` で戻そうとすると、存在しない駒の移動を復元しようとしてパニックします。

## まとめ

StateInfo パターンは、将棋エンジンの探索効率を大幅に向上させる重要な設計です：

- **メモリ効率**: Position 全体ではなく変更点だけを保存（400 bytes → 64 bytes）
- **高速な復元**: 保存した情報から元の状態を再構築
- **キャッシュ効率**: 小さいデータ構造でCPUキャッシュを有効活用
- **実装の明確さ**: apply_move32/undo_move32 のペアで状態管理が明確
- **将棋固有**: 持ち駒の管理が追加の複雑さをもたらす

## 次に読む

→ **[Zobrist Hashing](./zobrist.md)**: StateInfo で保存・差分更新されるハッシュ値の仕組みを解説します。

→ **[探索エンジンとの統合](./search-integration.md)**: rsshogi をラップして探索エンジンを構築する方法を解説します。

## 参考資料

- [やねうら王mini連載 6日目: StateInfo構造体](https://yaneuraou.yaneu.com/2015/12/12/) - 状態管理の設計理由
- [Chess Programming Wiki - Copy-Make](https://www.chessprogramming.org/Copy-Make) - Position全体をコピーする手法
- [Chess Programming Wiki - Unmake Move](https://www.chessprogramming.org/Unmake_Move) - 差分更新による復元
