# ビットボード

> **前提知識**: [基本型](../types/index.md)（Square, PieceType, Piece の定義）

## このページの要点

- Bitboard は「盤面のマス集合をビット列で表現する」データ構造。集合演算がビット演算に直結する
- 将棋では 81 マスのため **128 ビット表現**（`[u64; 2]` + [SSE2](../simd/instructions.md#sse2-128-ビット整数論理演算) `__m128i`）を使用
- Bitboard は盤面のすべてを置き換えるものではなく、**集合操作が頻繁な情報**（駒の配置、利き）に使い、**個別アクセスが必要な情報**（各マスの駒種）には配列を併用する
- `pop_lsb()` により、セットされたビットだけを O(駒数) で列挙できる（81マスのループは不要）

## なぜ Bitboard か

81 人が座る教室を想像してください。「眼鏡をかけている人」を探したいとき、1 人ずつ確認すると最悪 81 回のチェックが必要です。
しかし、「眼鏡リスト」（眼鏡=1, それ以外=0 のビット列）を持っていれば、リストを見るだけで一瞬です。
さらに「眼鏡かつ男性」を知りたければ、「眼鏡リスト AND 男性リスト」で 1 命令で交差集合が得られます。

Bitboard はまさにこれと同じ原理です。
「先手の歩の位置」「飛車の利き範囲」「空きマス」といった集合を 128 ビットの値として保持し、AND/OR/XOR で高速に操作します。

### 歴史的背景

Bitboard の概念は 1960 年代のソビエトのチェスプログラム Kaissa に遡ります。[^rustic-bitboards]
チェスは 64 マスなので 1 つの `u64` に収まり、単一レジスタで盤面全体を操作できるため普及しました。

将棋への適用は 2000 年代の Bonanza が先駆的です。
Reijer Grimbergen（山形大学）が 2007 年に将棋ビットボードの体系的な研究を発表しました。[^grimbergen-bitboards]
その後 参照実装が [SIMD 命令](../simd/index.md)を活用した 128 ビット実装を確立し、現在の将棋エンジンの標準となっています。[^yaneuraou-magic]
rsshogi もこの流れを継承しています。

## Bitboard とは

Bitboard は「盤上のマス = ビット」を対応付ける集合表現です。
将棋では 9×9 = 81 マスのため 128 ビット表現を採用します。

各マスを 1 ビットに割り当て、駒の位置や攻撃範囲を集合演算で操作します。

### 実装詳細

```rust
{{#include ../../../../../crates/rsshogi/src/types/bitboard.rs:bitboard_struct}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/types/bitboard.rs#L108-L129)</small>

```text
Bitboard: P(Squares) → {0,1}^n
  ここで P(S) は集合 S の冪集合、n は盤面のマス数
```

## ビット演算 = 集合演算

集合演算はビット演算へ直接マッピングできます。

| 集合演算 | ビット演算 | 用途例 |
|---------|----------|--------|
| 和集合 A ∪ B | `a \| b` | 自駒と敵駒を統合 |
| 積集合 A ∩ B | `a & b` | 攻撃範囲と敵駒の交差 |
| 差集合 A \ B | `a & !b` | 合法手から自駒を除外 |
| 対称差 A △ B | `a ^ b` | 局面の差分更新 |
| 補集合 Ā | `!a` | 空きマスの抽出 |

### ミニ例: オセロの裏返し

```c
bitboard_black ^= move | flipped;  // 差集合 + 和集合
bitboard_white ^= flipped;         // 対称差
```

このように、配列ループを伴う処理が 1-2 命令のビット演算で表現できます。

## 駒の配置と利き

まずは 5 筋に 3 枚だけ置き、駒の向きと座標を確認します。
5九の香と5三の歩は先手、5一の香は後手です。盤面上では先手駒は読者側を向き、後手駒は反対向きに表示されます。
丸はこの例で注目する駒、矢印は「先手の歩が5三から5二へ利く」ことだけを示しています。
香の長い利きは次の [レイアウト](./layout.md) で扱うため、ここでは表示していません。

<div id="basics-live-1" style="width:600px;height:660px;margin:1rem auto;border:1px solid #ddd;"></div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('basics-live-1');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: false });
  // 5九に先手香、5三に先手歩、5一に後手香。
  board.setPositionFromSFEN('4l4/9/4P4/9/9/9/9/9/4L4 b - 1');
  board.goTo(0);
  // 先手歩の利き（前方1マス）。香の長い利きはここでは省略する。
  board.setArrows([
    { from: '5c', to: '5b', color: 'rgba(0,120,215,0.7)', width: 1.2 },
  ]);
  // このミニ例で注目する3枚。
  board.setCircles([
    { square: '5i', color: 'rgba(255,140,0,0.7)', width: 1.5 },
    { square: '5c', color: 'rgba(0,120,215,0.7)', width: 1.5 },
    { square: '5a', color: 'rgba(220,50,50,0.7)', width: 1.5 },
  ]);
  window.basicsLive1 = board;
}
</script>

## AND でフィルタする

ビットボードの核心は「集合演算 = ビット演算」です。
たとえば「先手の歩」は、`by_piece[PAWN]`（歩があるマス集合）と `by_color[BLACK]`（先手駒があるマス集合）の共通部分です。
AND は方向を持つ操作ではないため、矢印では表しません。下の3枚の盤では、左から順に入力集合、入力集合、結果集合を黄色で示します。

<div id="basics-live-and" style="display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:0.75rem;margin:1rem auto;max-width:960px;">
  <div>
    <p style="text-align:center;margin:0 0 0.35rem;">by_piece[PAWN]</p>
    <div id="basics-live-and-pawns" style="width:100%;height:360px;border:1px solid #ddd;"></div>
  </div>
  <div>
    <p style="text-align:center;margin:0 0 0.35rem;">by_color[BLACK]</p>
    <div id="basics-live-and-black" style="width:100%;height:360px;border:1px solid #ddd;"></div>
  </div>
  <div>
    <p style="text-align:center;margin:0 0 0.35rem;">AND result</p>
    <div id="basics-live-and-result" style="width:100%;height:360px;border:1px solid #ddd;"></div>
  </div>
</div>
<script src="../../assets/shogi-board.js"></script>
<script src="../../assets/bitboard-index.js"></script>

```rust,ignore
// 先手の歩だけを抽出（AND 演算 1 命令）
let black_pawns = bitboards.by_piece[PAWN] & bitboards.by_color[BLACK];
//                 ^^^^^^^^^^^^^^^^^^^^^^^^   ^^^^^^^^^^^^^^^^^^^^^^^
//                 全歩の集合                  先手駒の集合
//                 → 交差 = 先手の歩のみ
```

## OR で利きを合成して王手を判定する

複数駒の利きを `OR` で合成し、王の位置と `AND` で王手判定します。
赤い矢印が飛車の利き、オレンジの矢印が角の利き、青い丸が王の位置です。

<div id="basics-live-check" style="width:600px;height:660px;margin:1rem auto;border:1px solid #ddd;"></div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('basics-live-check');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: false });
  // 5五に後手玉、5一に先手飛、9九に先手角
  board.setPositionFromSFEN('4R4/9/9/9/4k4/9/9/9/B8 b - 1');
  board.goTo(0);
  // 飛車から王への利き（縦方向の王手）
  board.setArrows([
    { from: '5a', to: '5e', color: 'rgba(220,50,50,0.7)', width: 1.5 },
    { from: '9i', to: '5e', color: 'rgba(255,140,0,0.7)', width: 1.5 },
  ]);
  // 王の位置を丸で強調
  board.setCircles([
    { square: '5e', color: 'rgba(0,120,215,0.8)', width: 2.0 },
  ]);
  window.basicsLiveCheck = board;
}
</script>

```rust,ignore
// 王手判定: 飛車と角の利きを OR で合成し、王の位置と AND
let attackers =
    (rook_attacks(king_sq, occ)   & enemy_rooks)   |  // 飛車の利き
    (bishop_attacks(king_sq, occ) & enemy_bishops);    // 角の利き
let in_check = attackers.any();  // 1 命令で判定完了
```

## 速習スニペット

以下はビット演算の最小例です。

```rust,ignore
// 先手の歩だけを抽出
let black_pawns = bitboards.by_piece[PAWN] & bitboards.by_color[BLACK];

// 最下位ビットを取り出して列挙
let mut moves = black_pawns;
while moves.any() {
    let to = moves.pop_lsb();
    emit_move(to);
}
```

## 全データを Bitboard で表現するのか？

いいえ。必要な情報だけを選択的に Bitboard 化します。
Bitboard は「盤面の全て」を置き換えるデータ構造ではありません。

実際のエンジンでは以下のように使い分けます：

```rust,ignore
struct Position {
    // Bitboard: 集合演算が必要な情報
    bitboards: BitboardSet,  // 駒種別・先後別の配置

    // 配列: ランダムアクセスが必要な情報
    board: [Piece; 81],      // 各マスの駒（「5五に何がある？」）
    hand: [Hand; 2],         // 持ち駒

    // スカラー: 単一の値
    side_to_move: Color,     // 手番
    ply: u16,                // 手数
}
```

**Bitboard が得意**:
- 「先手の歩が何処にあるか？」（集合演算）
- 「飛車の利きと敵駒の交差」（AND演算）
- 「王手している駒の列挙」（ビット走査）

**配列が得意**:
- 「5五のマスに何の駒があるか？」（O(1) アクセス）
- 「持ち駒の枚数」（駒種ごとのカウンタ）

**使い分けの原則**:
- **集合操作** → Bitboard
- **個別アクセス** → 配列

<details>
<summary>よくある誤解：「Bitboard は利きの一括生成だけが速い？」（クリックで展開）</summary>

真の強みは集合演算のワンライナー化とビット列挙の高速性です。
置換表や評価、差分更新との連携が全体の速度を左右します。
滑り利きは Qugiy 方式（差分抽出＋`byte_reverse`）で O(1) 算出しています。

### 誤解 1：配列版とほぼ同じで、利きを作る部分だけが速い

実際には、Bitboard の高速化の本質は集合演算にあります。

#### 配列ベースの王手判定（概念例）

```rust,ignore
fn is_in_check_array(position: &ArrayPosition) -> bool {
    let king_sq = position.king_square;

    // 全敵駒をループ (平均 20-40 駒)
    for piece in position.enemy_pieces() {
        if piece.attacks(position).contains(king_sq) {
            return true;  // 王手発見
        }
    }
    false
}
```

#### Bitboard ベースの王手判定（概念例）

```rust,ignore
fn is_in_check_bitboard(position: &Position) -> bool {
    let king_sq = position.king_square;
    let enemy_color = position.side_to_move.flip();
    let bitboards = position.bitboards();
    let occupied = bitboards.occupied;

    // 王から見た攻撃範囲と敵駒の交差を一括チェック
    let attackers =
        (rook_attacks(king_sq, occupied) & bitboards.by_piece[ROOK]) |
        (bishop_attacks(king_sq, occupied) & bitboards.by_piece[BISHOP]) |
        (GOLD_ATTACKS[king_sq][position.side_to_move] & bitboards.by_piece[GOLD]) |
        // ... 他の駒種
        ;

    (attackers & bitboards.by_color[enemy_color]).any()  // 1-2 命令
}
```

高速化しやすい理由:
- 駒種ごとの集合をまとめて扱える
- AND/OR 演算で候補集合を合成できる
- セットされたビットだけを列挙できるため、81 マス全走査を避けられる

### 誤解 2：「Bitboard から手を読み出す際に 81 マス全部チェックする」

実際には駒がある場所だけを O(駒数) で巡回します。

#### 素朴な実装（遅い）
```rust,ignore
for sq in 0..81 {
    if moves.is_set(sq) {  // 81 回ループ
        emit_move(from, sq);
    }
}
```

#### popcount/trailing zeros を使った実装（速い）
```rust,ignore
while moves.any() {  // 駒数回だけループ
    let to = moves.pop_lsb();  // trailing_zeros + clear
    emit_move(from, to);
}
```

</details>

## 読了順序

- [レイアウト](./layout.md)：縦型インデックス、ビット順序、ファイル/ランク定義
- [基本操作](./operations.md)：マスク、シフト、between/line、`pop_lsb()` 等
- [利きの計算](./attacks.md)：ステップ駒・遠方駒の利き生成
- [飛び利きアルゴリズム比較](./slider-survey.md)：Qugiy, PEXT, Magic, LZ/TZ 法の比較

## 参考文献

[^rustic-bitboards]: Rustic Chess, ["Bitboards"](https://rustic-chess.org/board_representation/bitboards.html) — Rust でのビットボード実装入門
[^grimbergen-bitboards]: Reijer Grimbergen (2007). "Using Bitboards for Move32 Generation in Shogi". ICGA Journal Vol. 30, No. 1, pp. 25-34.
[^yaneuraou-magic]: やねうら王 ["magic bitboard論争に終止符を"](https://yaneuraou.yaneu.com/2015/10/13/magic-bitboard%E8%AB%96%E4%BA%89%E3%81%AB%E7%B5%82%E6%AD%A2%E7%AC%A6%E3%82%92/)
