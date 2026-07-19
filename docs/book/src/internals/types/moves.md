# 指し手の表現

> **前提知識**: [駒の定義](./pieces.md)

## このページの要点

- 指し手は **16 ビット** で表現する。移動先(7bit) + 移動元(7bit) + 駒打ちフラグ(1bit) + 成りフラグ(1bit)
- 移動する駒の種類と取る駒の種類は **含まれない**。それらは `Position` から取得する（キャッシュ効率優先）
- 16 ビットにすることで、評価値 `i16` とペアで **32 ビットに収まる**（Stockfish の `ExtMove` 相当の概念）。キャッシュライン効率が最大化される
- 特殊定数 `MOVE_NONE`（手なし）・`MOVE_NULL`（パス）・`MOVE_RESIGN`（投了）・`MOVE_WIN`（宣言勝ち）・`MOVE_END` は値の範囲で区別される

## 設計目標

指し手の表現設計では、以下の要件のバランスを取る必要があります：

1. **メモリ効率**: 探索中に大量の指し手を生成・保持するため、コンパクトな表現が求められる
2. **処理速度**: 指し手の生成・適用・検証は探索全体の性能に直結する
3. **情報の十分性**: 指し手を盤面に適用するために必要な情報を保持する

多くの将棋エンジンは、Stockfishに倣って16ビットエンコーディングを採用しています。
これは、指し手と評価値を32ビット変数にペアで格納できるため、メモリ効率とキャッシュ効率の両面で有利です。

## 16ビットエンコーディング

rsshogiでは、指し手を16ビットで以下のように表現します：

```text
Move (16 bits):
  bits  0-6 : 移動先 (to square)      7 bits → 0-127 (0-80が有効)
  bits  7-13: 移動元 or 打つ駒種      7 bits
  bit  14   : 駒打ちフラグ            1 bit
  bit  15   : 成りフラグ              1 bit
```

### 実装: Move 構造体

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/types/moves.rs:move_struct}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/types/moves.rs#L63-L73)</small>

### 実装: MoveType 種別

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/types/moves.rs:move_type_enum}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/types/moves.rs#L85-L93)</small>

### ビットレイアウトの詳細

`Move` は名前付きフィールドを持つ構造体ではなく `Move(u16)` の newtype です。
以下は各ビットフィールドが何を表すかを示す**概念図**であり、実際の構築は `Move::normal(from, to)` /
`Move::promotion(from, to)` / `Move::drop(piece_type, to)` のコンストラクタを通します。

```text
// 基本的な指し手（概念図。実際は Move::normal(from, to) で構築）
normal_move:  to = 45 (bits 0-6) | from = 36 (bits 7-13) | drop = 0 (bit 14) | promo = 0 (bit 15)

// 成る指し手（Move::promotion(from, to)）
promote_move: to = 12 | from = 21 | drop = 0 | promo = 1 (bit 15 を立てる)

// 駒打ちの指し手（Move::drop(PieceType::LANCE, to)）
drop_move:    to = 55 | from = 2 (= PieceType::LANCE) | drop = 1 (bit 14 を立てる) | promo = 0
```

### 実装: ビット定数とフラグ

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/types/moves.rs:move_bit_constants}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/types/moves.rs#L211-L222)</small>

## 設計上のトレードオフ

### 含まれる情報

16ビット表現には以下の情報が含まれます：

- 移動先マス（to square）
- 移動元マス（from square）または打つ駒種
- 駒打ちかどうか
- 成るかどうか

### 含まれない情報

パフォーマンス優先のため、以下の情報は**意図的に含まれていません**：

- **移動する駒の種類**: 移動元マスを参照すれば取得可能
- **取る駒の種類**: 移動先マスを参照すれば取得可能

これらの情報が必要な場合は、`Position`構造体を参照して取得します。
この設計により、指し手生成のループが高速化されます。

### 実装: アクセサメソッド

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/types/moves.rs:move_accessors}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/types/moves.rs#L304-L352)</small>

## 特殊な指し手定数

指し手の種類を区別するため、いくつかの特殊な定数が定義されています：

### 実装: 特殊手定数

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/types/moves.rs:move_special_constants}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/types/moves.rs#L75-L83)</small>

### MOVE_NONE

値が存在しないことを示す定数です。
置換表（Transposition Table）に有効な指し手が保存されていない場合などに使用されます。

```rust,ignore
if tt_move == MOVE_NONE {
    // 置換表から有効な指し手が得られなかった
}
```

### MOVE_NULL

Null Move Pruning で使用される特殊な指し手です。
手番をパスして相手に連続して指させることで、探索の枝刈りを行います。

### MOVE_RESIGN

投了を表す定数です。
合法手が存在しない場合（詰み）に使用されます。

### MOVE_WIN / MOVE_END

- `MOVE_WIN`: 入玉宣言勝ち（USI の `win`）を表す定数です。
- `MOVE_END`: 番兵（sentinel）として使う終端マーカーです。

これらも `MOVE_NONE` / `MOVE_NULL` / `MOVE_RESIGN` と同様、移動先と移動元に同じ小さな値を詰めた特殊値で、
`is_normal()` では通常手から除外されます。

## パフォーマンスへの影響

### メモリ効率

16ビット表現により、以下の最適化が可能です：

```rust,ignore
// 指し手と評価値を32ビットでペア管理する一般的な手法（概念例）
// Stockfish ではこれを ExtMove と呼ぶ。rsshogi 自体はこの型を公開していないが、
// 16 ビット表現はこうしたパッキングを可能にするための設計である。
struct ScoredMove {
    mv: u16,      // 16 bits: 指し手
    score: i16,   // 16 bits: 評価値
}

// 1つのキャッシュライン(64 bytes)に32個の指し手を格納可能
const MOVES_PER_CACHE_LINE: usize = 64 / 2; // = 32
```

## 指し手の生成と適用の流れ

### なぜ駒の情報を含めないのか

指し手生成のループでは、移動する駒の情報は既に分かっています：

```rust,ignore
// 歩の指し手を生成する例
fn generate_pawn_moves(position: &Position, moves: &mut Vec<Move>) {
    let pawns = position.pieces(PAWN, position.side_to_move());

    for from in pawns {
        let to = from + PAWN_DIRECTION;

        // ここで既に「歩」であることが分かっている
        // わざわざ Move に駒種を埋め込む必要はない
        moves.push(Move::normal(from, to));
    }
}
```

## USI プロトコルでの表現

USI（Universal Shogi Interface）プロトコルでは、指し手を人間が読める文字列で表現します：

```text
通常の指し手: 7g7f      (7七から7六へ移動)
成る指し手:   2b2a+     (2二から2一へ移動して成る)
駒打ち:      P*5e      (5五に歩を打つ)
```

## Move と Move32（32ビット）のトレードオフ

rsshogi は 16 ビットの `Move` に加えて、32 ビットの `Move32` も提供しています。

| | Move (16bit) | Move32 (32bit) |
|---|---|---|
| **格納情報** | 移動先・移動元・成り・打ち | 左記＋移動後の駒（`piece_after_move`） |
| **用途** | 探索中の大量生成・置換表キー | 棋譜記録・デバッグ・API公開 |
| **メモリ** | 2 bytes | 4 bytes |
| **駒情報の取得** | `Position` 参照が必要 | 移動後の駒は自己完結（取る駒は `Position` 参照） |
| **互換性** | Stockfish/YaneuraOu 標準 | rsshogi 独自拡張 |

探索のホットパスでは `Move` を使い、棋譜の入出力や Python API では情報を自己完結させた `Move32` を使う、という使い分けが基本です。

### Move32 の2つの形態: 部分と完全

`Move32` には生成方法によって2つの形態があります。

| | 部分 Move32 | 完全 Move32 |
|---|---|---|
| **生成方法** | `Move32::from_usi()` | `Position::move32_from_move()` |
| **上位16bit** | 未設定（0） | 移動後の駒情報 |
| **`piece_after_move()`** | `Piece::NONE` を返す | 正しい駒を返す |
| **`has_piece_info()`** | `false` | `true` |
| **apply/undo** | 動作する | 動作する |
| **CSA/KI2 変換** | 不可 | 可能 |
| **用途** | USI パース | 棋譜記録、API 公開 |

#### apply/undo と上位16bitの関係

`apply_move32` は移動する駒を **盤面から取得** するため、局面遷移の正しさに上位16bitは不要です:

```rust,ignore
// apply_move32_with_gives_check 内部
let moved_piece = self.board.get(from);  // 盤面参照 — 上位16bitは不使用
```

同様に `undo_move32` は取った駒を `StateInfo.captured` から復元します。

**ただし**、`apply_move32` は引数の `Move32` を `StateInfo.last_move` にそのまま格納します。
部分 Move32 で apply すると `last_move.piece_after_move()` が正しい値を返しません。
このため、公開 API の `apply_move(Move)` は内部で `move32_from_move`（完全化）を経由します。

#### 使い分けガイド

- **内部処理・合法性判定**: `Move` をそのまま使用（`is_legal_move(Move)` 等）
- **棋譜出力・Python API**: `Position::move32_from_move()` で完全 Move32 に変換が必要
- **判別**: `has_piece_info()` で部分/完全を確認可能

## 落とし穴

### `from` フィールドの二重目的

通常の指し手では `from`（bits 7-13）は移動元マス（0〜80）を表しますが、
駒打ちの場合は同じフィールドに **打つ駒種**（`PieceType` の値 1〜7）が格納されます。

```rust,ignore
// ❌ 駒打ちの from を Square として解釈してしまう
let origin = mv.from();  // 駒打ちなら PieceType の値が返る！

// ✅ まず種別を確認してから解釈する
if mv.is_drop() {
    let pt = mv.drop_piece_type();
} else {
    let from_sq = mv.from();
}
```

### `MOVE_NONE` と `MOVE_NULL` の混同

- `MOVE_NONE`: 「指し手が存在しない」。置換表に有効な手がない場合など
- `MOVE_NULL`: 「パスする」。Null Move Pruning で使用する有効な操作

両者は意味が異なりますが、どちらも「通常の指し手ではない」ため混同しやすいです。
置換表から取得した手が `MOVE_NONE` なら「手がなかった」、`MOVE_NULL` なら「Null Move として記録されていた」と判断します。

## Policy 学習向けラベル

`Move` は探索や局面更新に最適化された内部表現ですが、ニューラルネットワークの policy 学習では
固定長クラスへ写像したラベルが必要になることがあります。

rsshogi では、この用途を `types` ではなく `labels::policy` に分離しています。
`Move` / `Move32` と手番から 2187 クラスの `MoveLabel`、さらに 1496 クラスの
`CompactMoveLabel` へ変換できます。

- `MoveLabel`: `class * 81 + to_sq` で表される 27x81 ラベル
- `CompactMoveLabel`: 構造的に存在しえない 691 クラスを取り除いた gapless label
- `MOVE_LABEL_TO_COMPACT` / `COMPACT_TO_MOVE_LABEL`: 双方向変換テーブル

詳細は **[Policy ラベル](./policy-labels.md)** を参照してください。

## まとめ

指し手の16ビット表現は、将棋エンジンの性能を左右する設計決定です。

- **コンパクトさ**: 16ビットで必要十分な情報を表現
- **速度優先**: 不要な情報を持たないことで生成を高速化
- **実用的**: 移動元・移動先の Position 参照で駒種を取得可能
- **二重目的の `from`**: 通常手は Square、駒打ちは PieceType。種別チェック必須

この設計思想は、「探索中に毎秒数百万〜数千万の指し手を扱う」という将棋エンジンの性質に最適化されています。

## 次に読む

→ **[ビットボード](../bitboard/index.md)**: 座標・駒・指し手の語彙が揃ったので、次は盤面の集合演算を高速に行う Bitboard のコンセプトに進みます。

## 参考資料

- [やねうら王mini連載 4日目: 指し手の表現](https://yaneuraou.yaneu.com/2015/12/10/) - 16ビットエンコーディングの設計理由
- [Stockfish - Move Representation](https://github.com/official-stockfish/Stockfish) - チェスエンジンでの類似実装
- [Chess Programming Wiki - Move Encoding](https://www.chessprogramming.org/Encoding_Moves) - さまざまなエンコーディング手法
