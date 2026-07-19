# Position 構造体

> **前提知識**: [ビットボード](../bitboard/index.md)（Bitboard の集合演算と128ビット表現）

`Position` 構造体は、将棋エンジンにおける局面の中心的な表現です。
盤面の状態、持ち駒、手番、ハッシュ値など、局面を完全に記述するために必要なすべての情報を保持します。

## なぜ Position が重要か

将棋エンジンの探索では、毎秒数百万〜数千万の局面を生成・評価します。
Position はそのすべての起点であり、エンジンの心臓部です。
Position の設計が探索速度を直接左右するため、メモリレイアウトの最適化、
差分更新の効率化、問い合わせ API の設計に細心の注意が払われています。

## このページの要点

- `Position` は局面の実体であり、履歴は `StateStack` が保持します。
- 指し手の適用は `apply_move32(mv)`、巻き戻しは `undo_move32(mv)` を使用します。
- 盤面は `BoardArray`、集合情報は `BitboardSet`、ハッシュは `ZobristKey` で管理します。
- 王手・ピン・遮蔽駒などの cache は current state に紐づき、`current_state_cache()` から narrow view として取得できます。
- API は「高速な問い合わせ」と「整合性の検証」を意識して設計されています。

## 基本設計

### Position の役割

Position 構造体は以下の責務を持ちます：

1. **局面の完全な記述**: 盤面、持ち駒、手番、手数など
2. **指し手の適用と復元**: `apply_move32()` / `undo_move32()` による状態変更
3. **局面の問い合わせ**: 駒の配置、合法性判定、王手判定など
4. **ハッシュ値の管理**: Zobrist hashing による高速な同一性判定

### 基本構造

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/board/position/mod.rs:position_struct}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/board/position/mod.rs#L149-L229)</small>

## 指し手の適用と復元

Position の最も重要な機能は、指し手を適用して局面を更新し、後で元に戻せることです。

### StateStack と apply_move32()/undo_move32()

rsshogi は `StateStack` に差分情報を積みながら、`Position` 本体は常に最新の局面を保持します。
`StateStack` は segmented storage で保持され、stack が伸長しても同一インスタンス内の
live entry の物理アドレスは維持されます。外部 API からは `Position` が
`StateStack` を内部管理するため、通常は `apply_move32(mv)` / `undo_move32(mv)` を
呼ぶだけで十分です。

<details>
<summary>適用/巻き戻しの流れ（擬似コード、クリックで展開）</summary>

```rust,ignore
// 適用
let cache = pos.current_state_cache();
let gives_check = pos.gives_check_move32(mv);

// 内部では StateStack の next slot を準備し、
// board / bitboards / hands / hash / tactical caches を更新する
pos.apply_move32_with_gives_check(mv, gives_check);

// 巻き戻し
pos.undo_move32(mv)?;
```

</details>

実装詳細は `crates/rsshogi/src/board/position/updates.rs` の `apply_move32` /
`undo_move32` を参照してください。

<small>`StateStack` と update flow は `crates/rsshogi/src/board/state_info.rs` / `crates/rsshogi/src/board/position/updates.rs` を参照してください。</small>

### 盤面更新と集合更新の一貫性

盤面配列とビットボード集合を常に同期させるため、`Position` 内部の更新ヘルパは必ず両者を同時に更新します。
外部から直接配列や集合を書き換えない設計にし、整合性と速度を両立します。

## 局面の問い合わせ API

Position は、探索や評価で必要な情報を提供する多くの問い合わせメソッドを持ちます。

### 基本的な情報取得

```rust,ignore
// 高速問い合わせ API（代表）
pos.piece_on(sq);
pos.hand(color);
pos.turn();
pos.key();
pos.bitboards();
pos.current_state_cache();
```

### ビットボードによる高速問い合わせ

```rust,ignore
impl Position {
    /// 指定色の全ての駒のビットボード
    pub fn occupied(&self) -> Bitboard { self.bitboards.occupied() }
    pub fn king_square(&self, color: Color) -> Square { self.bitboards.king_square(color) }
}
```

## 千日手検出

StateInfo のスタックを使って、千日手（同一局面の繰り返し）を検出します。

rsshogi の実装では `apply_move32()` が内部で `repetition_counter` を差分更新し、
`is_repetition(threshold)` は単にカウンタを閾値と比較するだけの O(1) 判定です。

```rust,ignore
// is_repetition の使い方（rules.rs より）
// threshold に指定した回数以上繰り返された局面かを O(1) で判定する
pos.is_repetition(3)  // 3回以上繰り返されていれば true
```

連続王手の千日手や優等/劣等局面の判定には `repetition_state()` を使います
（詳細は [探索エンジンとの統合](./search-integration.md) を参照）。

## 王手判定

```rust,ignore
// 手番側の玉に王手がかかっているか（引数なし）
pos.is_in_check()  // → bool

// 指定マスが指定色（by_color）に攻撃されているか
// 第1引数: 攻撃対象のマス, 第2引数: 攻撃する側の色
pos.attackers_to(sq, occupied)         // → Bitboard（全攻撃駒）
pos.is_attacked_by(sq, by_color)       // → bool（特定色かどうか）
// ※ is_attacked_by(sq, by_color) は attacks.rs で定義
```

王手をかけている駒の Bitboard は `pos.checkers()` で取得できます。
これらは `apply_move32()` のたびに StateInfo に自動キャッシュされます。

## 合法手判定

`is_legal_move32(mv)` は `(pseudo_legal + 成り条件 + 自殺手チェック)` を一括で行います。
探索ホットパスでは「生成 → 適用後に `is_legal_after_pseudo_move32()` 」の 2 段階が効率的です。

```rust,ignore
// 完全な合法性判定
pos.is_legal_move32(mv)  // → bool

// pseudo-legal 確認のみ（生成済みの手に使用）
// 第2引数 generate_all_legal_moves: 不成も生成する全合法手モードか（通常は false）
pos.is_pseudo_legal_move32(mv, false)  // → bool

// 自殺手チェックのみ（split legality 後段）
pos.is_legal_after_pseudo_move32(mv)  // → bool
```

### パフォーマンス vs 正確性

完全な合法性判定は高コストです。
そのため、指し手生成では「疑似合法手（pseudo-legal moves）」を生成し、最後に `is_legal_move32()` でフィルタする戦略が一般的です：

```rust,ignore
use rsshogi::movegen::{Legal, generate_moves};
use rsshogi::board::MoveList;

let mut moves = MoveList::new();
generate_moves::<Legal>(&pos, &mut moves);
// Legal は生成時に合法性フィルタを内包しているため、追加フィルタ不要
```

## デバッグ機能

### 局面の表示

`Position` は `Display` / `Debug` トレイトを実装しています。
専用の `print()` メソッドは存在しません。

```rust,ignore
println!("{}", pos);    // Display: SFEN 形式などの人間可読出力
println!("{:?}", pos);  // Debug: デバッグ用の詳細出力
```

## 落とし穴

### `BoardArray` と `BitboardSet` の不整合

Position は同じ情報を配列（`board[sq]`）とビットボード（`BitboardSet`）の両方で保持します。
一方だけを更新してもう一方を忘れると、王手判定や合法手生成で不正な結果が生じます。
必ず `Position` の内部メソッドを通じて更新し、直接フィールドを書き換えないでください。

### `undo_move32` の呼び忘れ

`apply_move32()` を呼んだ後に `undo_move32()` を忘れると、StateStack が崩れ、
以降のすべての操作が不正になります。探索コードでは `apply_move32` / `undo_move32` が
常にペアで呼ばれることをレビュー時に確認してください。

### Null Move32 後の undo 間違い

`apply_null_move()` には対応する `undo_null_move()` があります。
通常の `undo_move32()` で戻そうとすると、駒の移動がないにもかかわらず盤面を復元しようとしてパニックします。

## 不変条件（Invariants）

Position 構造体は、常に以下の不変条件を満たす必要があります：

1. **盤面とビットボードの一致**: `board[sq]` と `pieces_bb` / `color_bb` が常に同期している
2. **玉の存在**: 両者の玉が必ず1つずつ存在する
3. **持ち駒の整合性**: 持ち駒の合計が駒の総数を超えない
4. **ハッシュ値の一致**: `state().hash` が局面の実際のハッシュ値と一致する

### デバッグビルドでの検証

`validate()` は最初のエラーで停止し、`validate_all()` は全問題を収集して返します
（validation.rs に実装）。

```rust,ignore
// 最初のエラーで停止（Result を返す）
pos.validate()?;

// 全問題を収集（ValidationReport を返す）
let report = pos.validate_all();
if !report.is_valid() {
    for issue in report.issues() {
        eprintln!("{issue:?}");
    }
}

// 内部状態の整合性チェック（bool を返す）
assert!(pos.is_valid());
```

## まとめ

Position 構造体は将棋エンジンの中核であり、以下の役割を果たします：

- **局面の完全な記述**: 盤面、持ち駒、手番、履歴のすべてを管理
- **効率的な状態遷移**: `apply_move32(mv)` / `undo_move32(mv)` による高速な更新・復元
- **豊富な問い合わせ API**: 探索・評価に必要な情報を提供
- **不変条件の維持**: `validate()` / `validate_all()` で整合性を検証可能

適切に設計された Position は、将棋エンジンの性能と保守性を大きく左右します。

## 次に読む

→ **[ゲームステート管理](./state-management.md)**: Position の差分更新を支える StateInfo の設計パターンを解説します。

## 参考資料

- [やねうら王mini連載 6日目: Position と StateInfo](https://yaneuraou.yaneu.com/2015/12/12/) - apply_move32/undo_move32 パターン
- [Chess Programming Wiki - Board Representation](https://www.chessprogramming.org/Board_Representation) - 盤面表現の各種手法
- [Stockfish - Position Class](https://github.com/official-stockfish/Stockfish) - チェスエンジンでの実装例
