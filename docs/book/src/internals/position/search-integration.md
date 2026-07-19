# 探索エンジンとの統合

> **前提知識**: [Position 構造体](./index.md)、[差分更新と StateInfo](./state-management.md)、[Zobrist ハッシング](./zobrist.md)

## このページの要点

- rsshogi は**盤面表現・合法手生成・ルール判定**に特化したコアライブラリであり、評価関数や探索アルゴリズムを含まない
- 探索エンジンは rsshogi の `Position` を**ラップ**して、評価差分更新・置換表・手順管理などの探索固有の状態を独自に管理する
- `apply_move32()` / `undo_move32()` の前後にフックを入れることで、NNUE 差分更新などを正確に同期できる

## 設計思想: なぜ評価状態を分離したか

rsshogi はかつて NNUE 差分更新のための `EvalList`、`PieceList`、評価用 Zobrist キー（`pawn_key`, `material_key` 等）を `Position` 内部に保持していました。しかし、この設計には以下の問題がありました。

1. **責務の混在**: 盤面管理と評価差分更新は本質的に異なる関心事。コアライブラリに評価ロジックが混入すると、評価関数を使わないユースケース（GUI、棋譜解析ツール等）でも不要なオーバーヘッドが発生する
2. **柔軟性の欠如**: 評価関数の種類（NNUE、駒割り、手作り評価など）はエンジンごとに異なる。コアに特定の評価方式を組み込むと、他の方式への切り替えが困難になる
3. **パフォーマンス**: 使わないフィールドの初期化・コピー・差分更新は純粋なオーバーヘッド

現在の rsshogi は**ルール正確性に必要な最小限の状態**のみを管理します。

## ラッパーパターン

探索エンジンは `Position` を直接拡張するのではなく、**構造体でラップ**して使います。

```rust,ignore
use rsshogi::board::{Position, Move32List};
use rsshogi::board::movegen::generate_legal_all_move32;
use rsshogi::types::Move32;

/// 探索用の局面ラッパー
struct SearchPosition {
    /// rsshogi のコア局面
    pos: Position,
    /// NNUE 評価の差分情報（エンジン固有）
    eval_state: NnueState,
    /// 置換表キー（board_key ^ hand_key で取得可能）
    // pos.key() で直接取得できるため、通常はフィールドとして持つ必要はない
}

impl SearchPosition {
    fn apply_move(&mut self, mv: Move32) {
        // 1. 指し手適用前の情報を取得（評価差分に必要な場合）
        let captured = self.pos.piece_on(mv.to());

        // 2. rsshogi のコア局面を更新
        self.pos.apply_move32(mv);

        // 3. 評価状態を差分更新
        self.eval_state.update_after_move(mv, captured);
    }

    fn undo_move(&mut self, mv: Move32) {
        // 1. rsshogi のコア局面を復元
        self.pos.undo_move32(mv).expect("undo should succeed");

        // 2. 評価状態を復元
        self.eval_state.restore();
    }

    fn apply_null_move(&mut self) {
        self.pos.apply_null_move().expect("null move should succeed");
        self.eval_state.push_null();
    }

    fn undo_null_move(&mut self) {
        self.pos.undo_null_move().expect("undo null move should succeed");
        self.eval_state.pop_null();
    }
}
```

このパターンの利点:

- rsshogi のアップデートに追従しやすい（内部構造への依存がない）
- 評価関数の差し替えが容易（`NnueState` を別の実装に変えるだけ）
- コンパイル時に不要な依存が排除される

## rsshogi が提供する情報

探索エンジンが必要とする情報は、すべて `Position` の公開 API から取得できます。

### 局面のハッシュ

```rust,ignore
// 置換表のキーとして使用
let key = pos.key();                    // board_key ^ hand_key
let board_key = pos.board_key();        // 盤面 + 手番のハッシュ
let hand_key = pos.hand_key();          // 持ち駒のハッシュ

// 指し手適用前に、適用後のキーを計算（置換表の事前参照に有用）
let key_after = pos.key_after_move(mv); // Move 版
```

### 王手・ピン情報

`StateInfo` には王手・ピン・遮蔽駒の cache が含まれており、`apply_move32()` のたびに自動更新されます。
探索ホットパスでは `current_state_cache()` から `StateCacheView` を取得すると、
current state の tactical cache を 1 回の borrow でまとめて読めます。

```rust,ignore
let cache = pos.current_state_cache();

// 現在の手番側の玉に王手をかけている駒の集合
let checkers: Bitboard = cache.checkers();

// 玉に対してピンしている敵の大駒
let black_pinners = cache.pinners(Color::BLACK);
let white_pinners = cache.pinners(Color::WHITE);

// 玉へのラインをブロックしている駒
let black_blockers = cache.blockers_for_king(Color::BLACK);
let white_blockers = cache.blockers_for_king(Color::WHITE);

// 各駒種が王手できるマスの集合
let rook_checks = cache.check_square(PieceType::ROOK);

// 履歴・反復情報は Position の公開 getter から読む
let repetition_counter = pos.repetition_counter();
let last_move = pos.last_move();
```

`StateCacheView` は取得時点の current state を表す immutable borrow です。
`apply_move32()` / `apply_null_move()` / `undo_*()` を挟いだ後は再取得してください。

### 千日手判定

```rust,ignore
use rsshogi::types::RepetitionState;

// 千日手の状態を取得
let rep = pos.repetition_state();
match rep {
    RepetitionState::None => { /* 千日手ではない */ }
    RepetitionState::Draw => { /* 引き分け */ }
    RepetitionState::Win  => { /* 手番側の勝ち（相手の連続王手千日手）*/ }
    RepetitionState::Lose => { /* 手番側の負け（自分の連続王手千日手）*/ }
    RepetitionState::Superior => { /* 手番側の優等局面 */ }
    RepetitionState::Inferior => { /* 手番側の劣等局面 */ }
}

// 直接カウンタを参照（高度な用途）
let counter = pos.repetition_counter();   // 同一局面の出現回数
let distance = pos.repetition_distance(); // 直近の同一局面までの手数
```

### 合法手生成

```rust,ignore
use rsshogi::board::{Move32List, movegen::generate_legal_all_move32};
use rsshogi::movegen::{CapturePlusPro, Legal, QuietsProMinus};
use rsshogi::board::movegen::generate_moves_move32;

// 全合法手を生成
let mut moves = Move32List::new();
generate_legal_all_move32(&pos, &mut moves);

// 取る手 + 成る手のみ（静止探索用）
let mut captures = Move32List::new();
generate_moves_move32::<CapturePlusPro>(&pos, &mut captures);

// 取らない手（非取り手）のみ
let mut quiets = Move32List::new();
generate_moves_move32::<QuietsProMinus>(&pos, &mut quiets);
```

外部クレート側で独自コンテナへ直接書き込みたい場合は、
`Move32Sink` を実装して `generate_legal_all_move32_into()` /
`generate_moves_move32_into::<T>()` を使います。`Move32List` を一度経由せずに
駒情報付きの `Move32` を受け取れます。`Move32Sink` は `rsshogi::board::movegen` から公開されています。

### 王手判定の事前計算

`apply_move32_with_gives_check()` を使うと、指し手が王手かどうかの情報を渡して `StateInfo` の初期化を最適化できます。

```rust,ignore
// 事前に王手判定を計算
let gives_check = pos.gives_check_move32(mv);

// 王手情報付きで適用（内部でキャッシュ計算をスキップできる）
pos.apply_move32_with_gives_check(mv, gives_check);
```

## 探索ループの実装例

以下は、rsshogi を使った Alpha-Beta 探索の骨格です。

```rust,ignore
fn alpha_beta(
    sp: &mut SearchPosition,
    mut alpha: i32,
    beta: i32,
    depth: i32,
) -> i32 {
    // 千日手チェック
    let rep = sp.pos.repetition_state();
    if rep != RepetitionState::None {
        return repetition_score(rep);
    }

    // 深さ 0 で静止探索へ
    if depth <= 0 {
        return quiescence(sp, alpha, beta);
    }

    // 置換表の参照
    let key = sp.pos.key();
    if let Some(entry) = tt.probe(key) {
        // ... 置換表カットオフ
    }

    // Null Move Pruning
    if can_do_null_move(sp, beta, depth) {
        sp.apply_null_move();
        let score = -alpha_beta(sp, -beta, -beta + 1, depth - 3);
        sp.undo_null_move();
        if score >= beta {
            return beta;
        }
    }

    // 合法手の生成
    let mut moves = Move32List::new();
    generate_legal_all_move32(&sp.pos, &mut moves);

    if moves.is_empty() {
        return if sp.pos.is_in_check() { -MATE + sp.pos.game_ply() as i32 } else { 0 };
    }

    let mut best_score = -INFINITY;

    for &mv in moves.iter() {
        sp.apply_move(mv);
        let score = -alpha_beta(sp, -beta, -alpha, depth - 1);
        sp.undo_move(mv);

        if score > best_score {
            best_score = score;
            if score > alpha {
                alpha = score;
            }
        }
        if alpha >= beta {
            break;
        }
    }

    // 置換表に保存
    tt.store(key, best_score, depth, /* ... */);

    best_score
}
```

## 注意点とベストプラクティス

### apply/undo は必ずペアで呼ぶ

`apply_move32()` を呼んだら、必ず対応する `undo_move32()` を呼んでください。
`apply_null_move()` には `undo_null_move()` が対応します。混在させるとパニックします。

```rust,ignore
// ✅ 正しい
pos.apply_move32(mv);
// ... 探索 ...
pos.undo_move32(mv).expect("undo");

// ✅ 正しい
pos.apply_null_move().expect("null move");
// ... 探索 ...
pos.undo_null_move().expect("undo null");

// ❌ 間違い: null move を通常の undo で戻そうとする
pos.apply_null_move().expect("null move");
pos.undo_move32(some_move); // パニック!
```

### init_stack() の呼び出し

`position_from_sfen()` で生成した局面は `init_stack()` を呼ばないと `StateInfo` が初期化されません。探索を開始する前に必ず呼んでください。

```rust,ignore
let mut pos = position_from_sfen(sfen).expect("valid sfen");
pos.init_stack();  // ← これを忘れると checkers/pinners 等が未初期化
```

### 駒割り・評価の差分計算は自前で

rsshogi は駒割り計算や評価関数を提供しません。探索エンジン側で差分管理してください。

```rust,ignore
impl SearchPosition {
    fn apply_move(&mut self, mv: Move32) {
        // 適用前に取られる駒を確認
        let captured = self.pos.piece_on(mv.to());

        self.pos.apply_move32(mv);

        // 駒割りを差分更新（エンジン側で駒価値テーブルを定義）
        if captured != Piece::NONE {
            self.material += capture_delta(self.pos.turn().opponent(), captured);
        }
        if mv.is_promotion() {
            self.material += promotion_delta(/* ... */);
        }
    }
}
```

### Move32 を使う

rsshogi の主要 API は `Move32`（駒情報付き 32bit 指し手）を使います。
`Move`（16bit）は軽量な保存用で、適用時には `Move32` への変換が必要です。

```rust,ignore
// Move32 で直接操作（推奨）
pos.apply_move32(mv32);

// Move から Move32 への変換が必要な場合
let mv32 = pos.move32_from_move(mv16);
pos.apply_move32(mv32);
```

### StateInfo は内部実装として扱う

`StateInfo` / `StateStack` は `Position` が make-unmake のために内部管理する差分状態です。
探索エンジンから直接保持・差し替え・走査するための API ではありません。履歴やキャッシュの更新は
`apply_move32()` / `undo_move32()` に任せ、必要な情報だけ `Position` の getter から読んでください。

読み取り例:

```rust,ignore
let checkers = pos.checkers();
let rook_checks = pos.check_square(PieceType::ROOK);
let black_blockers = pos.blockers_for_king(Color::BLACK);
let last_move = pos.last_move();
let last_piece = pos.last_moved_piece_type();
let captured = pos.captured_piece();
let repetition_counter = pos.repetition_counter();
let repetition_distance = pos.repetition_distance();
```

`StateInfo` が保持する情報のうち、外部から安定して使ってよいものは `Position` の公開 getter として薄く出します。
探索エンジン固有の履歴、評価差分、置換表用の付帯情報まで rsshogi 側に載せると、ライブラリの責務が大きくなりすぎます。
そのため、エンジン側で独自に持つべき状態はラッパーや探索スタックで管理してください。

## Position の読み取り API

| API | 型 | 用途 |
|-----------|-----|------|
| `board_key()` | `ZobristKey` | 盤面 + 手番のハッシュ |
| `hand_key()` | `ZobristKey` | 持ち駒のハッシュ |
| `key()` | `ZobristKey` | 置換表などで使う局面キー |
| `checkers()` | `Bitboard` | 王手している駒の集合 |
| `pinners(color)` | `Bitboard` | 指定色の玉に対してピンしている敵の大駒 |
| `blockers_for_king(color)` | `Bitboard` | 指定色の玉へのラインをブロックする駒 |
| `check_square(piece_type)` | `Bitboard` | 指定駒種が王手できるマス |
| `last_move()` | `Move32` | 直前の指し手 |
| `last_moved_piece_type()` | `PieceType` | 直前に動いた駒種 |
| `captured_piece()` | `Piece` | 直前の指し手で取った駒。取っていなければ `Piece::NONE` |
| `repetition_counter()` | `i32` | 同一局面の出現回数 |
| `repetition_distance()` | `i32` | 直近の同一局面までの距離 |
| `repetition_state()` | `RepetitionState` | 千日手の種類 |

## まとめ

rsshogi は盤面管理に特化したライブラリとして、探索エンジンに以下を提供します:

- 高速な合法手生成（perft depth 5 で約 2000 万ノード/秒）
- 正確な千日手・連続王手千日手の判定
- 王手・ピン・遮蔽駒のキャッシュ（`StateInfo` 経由）
- Zobrist ハッシュの差分更新（置換表に直接利用可能）

評価関数（NNUE、駒割り）、SEE（静的交換評価）、置換表、手順管理、時間制御などの**探索固有のロジック**はエンジン側で実装してください。ラッパーパターンにより、rsshogi のコア機能とエンジン固有のロジックを明確に分離できます。

## 参考資料

- [Chess Programming Wiki - Search](https://www.chessprogramming.org/Search) - 探索アルゴリズムの概要
- [Chess Programming Wiki - Evaluation](https://www.chessprogramming.org/Evaluation) - 評価関数の設計
- [やねうら王 解説記事](https://yaneuraou.yaneu.com/) - 将棋エンジンの実装詳細
