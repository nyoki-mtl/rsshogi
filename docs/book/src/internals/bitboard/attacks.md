# 利きの計算

> **前提知識**: [基本操作](./operations.md)（マスク、シフト、`pop_lsb()` の使い方）

## このページの要点

- **ステップ駒**（歩・桂・銀・金・玉・成駒）は事前計算テーブルの参照だけで利きが確定する
- **遠方駒**（香・角・飛）は Occupancy 依存のため、Qugiy 方式（差分抽出＋`byte_reverse`）で O(1) 検出する
- **成駒の利き**は小駒（と・成香・成桂・成銀）が金の利き、大駒は「元の駒の利き + 王の利き」（馬 = 角 + 王、龍 = 飛 + 王）で合成する
- 駒打ちでは二歩・行き所のない駒・打ち歩詰めを**ビットマスクで早期フィルタ**する

このページでは、短い利き（歩・桂・銀・金・玉・成駒）と長い利き（香・角・飛）の両方について、ビットボードを使った利き計算の手法を解説します。

## 短い利き（ステップ駒）

ステップ駒は現在地の周囲の固定パターンで攻撃集合が決まります。[^psilord-rep]
したがって、事前計算済みの攻撃テーブルと集合演算だけで疑似合法手を列挙できます。

### 主要ステップ駒の利きパターン

以下のアニメーションで、各ステップ駒の利きパターンを矢印で確認できます。
5五に配置した駒の全利き方向が表示されます。

<div id="atk-step-patterns" style="width:600px;height:660px;margin:1rem auto;border:1px solid #ddd;"></div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('atk-step-patterns');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: false });
  board.animate([
    // 先手の歩（前方1マスのみ）
    {
      sfen: '9/9/9/9/4P4/9/9/9/9 b - 1',
      arrows: [
        { from: '5e', to: '5d', color: 'rgba(0,120,215,0.7)', width: 1.5 },
      ],
      circles: [{ square: '5e', color: 'rgba(0,120,215,0.4)' }],
      duration: 2500,
    },
    // 先手の桂馬（左右前方2マス）
    {
      sfen: '9/9/9/9/4N4/9/9/9/9 b - 1',
      arrows: [
        { from: '5e', to: '4c', color: 'rgba(255,140,0,0.7)', width: 1.5 },
        { from: '5e', to: '6c', color: 'rgba(255,140,0,0.7)', width: 1.5 },
      ],
      circles: [{ square: '5e', color: 'rgba(255,140,0,0.4)' }],
      duration: 2500,
    },
    // 先手の銀（前方3 + 斜め後方2）
    {
      sfen: '9/9/9/9/4S4/9/9/9/9 b - 1',
      arrows: [
        { from: '5e', to: '5d', color: 'rgba(0,180,0,0.7)' },
        { from: '5e', to: '4d', color: 'rgba(0,180,0,0.7)' },
        { from: '5e', to: '6d', color: 'rgba(0,180,0,0.7)' },
        { from: '5e', to: '4f', color: 'rgba(0,180,0,0.5)' },
        { from: '5e', to: '6f', color: 'rgba(0,180,0,0.5)' },
      ],
      circles: [{ square: '5e', color: 'rgba(0,180,0,0.4)' }],
      duration: 2500,
    },
    // 先手の金（前方3 + 横2 + 後方1 = 6方向）
    {
      sfen: '9/9/9/9/4G4/9/9/9/9 b - 1',
      arrows: [
        { from: '5e', to: '5d', color: 'rgba(220,50,50,0.7)' },
        { from: '5e', to: '4d', color: 'rgba(220,50,50,0.7)' },
        { from: '5e', to: '6d', color: 'rgba(220,50,50,0.7)' },
        { from: '5e', to: '4e', color: 'rgba(220,50,50,0.7)' },
        { from: '5e', to: '6e', color: 'rgba(220,50,50,0.7)' },
        { from: '5e', to: '5f', color: 'rgba(220,50,50,0.7)' },
      ],
      circles: [{ square: '5e', color: 'rgba(220,50,50,0.4)' }],
      duration: 2500,
    },
    // 先手の玉（全8方向）
    {
      sfen: '9/9/9/9/4K4/9/9/9/9 b - 1',
      arrows: [
        { from: '5e', to: '5d', color: 'rgba(128,0,255,0.7)' },
        { from: '5e', to: '5f', color: 'rgba(128,0,255,0.7)' },
        { from: '5e', to: '4e', color: 'rgba(128,0,255,0.7)' },
        { from: '5e', to: '6e', color: 'rgba(128,0,255,0.7)' },
        { from: '5e', to: '4d', color: 'rgba(128,0,255,0.7)' },
        { from: '5e', to: '6d', color: 'rgba(128,0,255,0.7)' },
        { from: '5e', to: '4f', color: 'rgba(128,0,255,0.7)' },
        { from: '5e', to: '6f', color: 'rgba(128,0,255,0.7)' },
      ],
      circles: [{ square: '5e', color: 'rgba(128,0,255,0.4)' }],
      duration: 2500,
    },
  ], { loop: true, autoPlay: true });
  window.atkStepPatterns = board;
}
</script>

### ステップ駒の攻撃テーブル参照

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/board/attack_tables.rs:step_attacks}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/board/attack_tables.rs#L22-L52)</small>

例として先手の歩を考えます。

```text
let us = side_to_move;
let pawns = pieces_cp(us, PAWN);
let empty = !occupied();
let push = shift_forward(pawns, us) & empty;
for to in push { let from = back_from(to, us); emit_move(from, to, promote_if_needed(from, to)); }
```

銀や金、玉も同様に、`attacks = ATTACKS[piece_type][from]` を取り、自駒集合を引いて `targets = attacks & !our_pieces` を列挙すれば十分です。
桂馬は前方二つ×左右一つの固定オフセットで計算します。

成りは「移動元または移動先が成りゾーンに入るか」をビットテストし、必要に応じて 2 手（成・不成）を出力します。

この方式では、短い利きの駒は数命令で候補集合を作り、`pop_lsb` で列挙するだけです。
高速かつ実装が単純で、分岐も少なく保守性に優れます。

## 駒打ち（ドロップ）の生成

持ち駒の種類ごとに「打てる升集合 = 空升 − 禁止条件」を作ります。
二歩、行き所のない駒、打ち歩詰めといった将棋特有の制約は、この段階で早期にフィルタします。[^yaneuraou-nifu]

### 歩の打てるマス（二歩フィルタ）

先手が歩を持っている状態で、5筋に既に歩がある場合の打てるマスを可視化します。
黄色ハイライトが実際に歩を打てる空きマス、赤い丸が 5 筋の自歩によって二歩で除外されるマスです。
緑の矢印は、打てるマスの一例として 3五への歩打ちを示しています。

<div id="atk-drop-demo" style="width:600px;height:660px;margin:1rem auto;border:1px solid #ddd;"></div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('atk-drop-demo');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: true });
  // 5七に先手歩（→ 5筋は二歩で打てない）、持ち駒に歩
  board.setPositionFromSFEN('9/9/9/9/9/9/4P4/9/9 b P 1');
  board.goTo(0);
  const dropTargets = [];
  for (let file = 1; file !== 10; file += 1) {
    if (file === 5) {
      continue;
    }
    for (let rank = 2; rank !== 10; rank += 1) {
      dropTargets.push((file - 1) * 9 + (rank - 1));
    }
  }
  board.highlightSquares(dropTargets);
  // 5筋を赤丸で二歩マーク
  board.setCircles([
    { square: '5a', color: 'rgba(220,50,50,0.4)' },
    { square: '5b', color: 'rgba(220,50,50,0.4)' },
    { square: '5c', color: 'rgba(220,50,50,0.4)' },
    { square: '5d', color: 'rgba(220,50,50,0.4)' },
    { square: '5e', color: 'rgba(220,50,50,0.4)' },
    { square: '5f', color: 'rgba(220,50,50,0.4)' },
    { square: '5h', color: 'rgba(220,50,50,0.4)' },
    { square: '5i', color: 'rgba(220,50,50,0.4)' },
  ]);
  // 持ち駒から打てる先の一例を矢印で示す
  board.setArrows([
    { from: 'S:P', to: '3e', color: 'rgba(0,180,0,0.5)' },
  ]);
  window.atkDropDemo = board;
}
</script>

```text
let empties = !occupied();
let files_without_our_pawn = FILE_MASKS & !our_pawns;
let pawn_drop_targets = empties & files_without_our_pawn & not_last_rank(us);
for to in pawn_drop_targets { emit_drop(PAWN, to); }
```

## 長い利き（飛び利き）の難しさ

香・角・飛は途中の駒の有無で利きが動的に変わります。
単純なテーブル参照では済まず、占有（Occupancy）依存の計算が必要です。
つまり、短い利きと同じ感覚では実装できません。

この問題に対しては複数の定番アプローチが知られています。
どれも「占有から一次元の線形表現を取り出し、最近接遮蔽駒までの区間を切り出す」ことを、高速に行う工夫です。

- **Rotated Bitboards**（90°/45° 回転）：ファイルや対角線を一次元化し、`index = rotated_occ & mask` でテーブル参照します。
- **PEXT Bitboards**（BMI2）：`pext` 命令で疎なビット列を圧縮してインデックス化します。
- **Magic Bitboards**：乗算シフトによる完全ハッシュでテーブル参照します。
- **LZ/TZ（Leading/Trailing Zeros）法**：線形に並ぶ各方向で「最近接遮蔽駒」を `ctz/clz` 相当で直接求めます。
- **Qugiy 方式（差分抽出＋byte_reverse）**：減算の借位伝播で遮蔽駒までのビーム区間を一括抽出し、`byte_reverse` で逆方向も処理します。rsshogi はこの手法を採用しています。

各手法の詳細な比較は [飛び利きアルゴリズム比較](./slider-survey.md) を参照してください。

## rsshogi の選択: Qugiy 方式（差分抽出＋byte_reverse）

rsshogi は WCSC31（2021）で Qugiy が発表した**差分抽出法**を採用しています。
減算の借位伝播と `byte_reverse` を組み合わせ、事前テーブルなしで遠方駒の利きを O(1) で算出します。

### 角の対角線に遮蔽駒がある場合

角の斜め利きが味方駒（6四の歩）で遮られる例を表示します。
4方向の対角線ビームのうち、左上方向だけが歩で遮断されています。

<div id="lz-tz-live-1" style="width:640px;height:720px;margin:1rem auto;border:1px solid #ddd;"></div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('lz-tz-live-1');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: false });
  // 5五に先手角、6四に先手歩（左上の対角線をブロック）
  board.setPositionFromSFEN('9/9/9/3P5/4B4/9/9/9/9 b - 1');
  board.goTo(0);
  // 4方向の対角線ビーム
  board.setArrows([
    // 右上（遮蔽なし → 1一まで到達）
    { from: '5e', to: '1a', color: 'rgba(0,120,215,0.6)', width: 1.2 },
    // 右下（遮蔽なし → 1九まで到達）
    { from: '5e', to: '1i', color: 'rgba(0,120,215,0.6)', width: 1.2 },
    // 左下（遮蔽なし → 9九まで到達）
    { from: '5e', to: '9i', color: 'rgba(0,120,215,0.6)', width: 1.2 },
    // 左上（6四の味方歩で遮断）
    { from: '5e', to: '6d', color: 'rgba(220,50,50,0.6)', width: 1.2 },
  ]);
  // 遮蔽駒に赤丸
  board.setCircles([
    { square: '6d', color: 'rgba(220,50,50,0.8)', width: 2.0 },
    { square: '5e', color: 'rgba(0,120,215,0.4)' },
  ]);
  window.lzTzLive1 = board;
}
</script>

- 🔵 **青い矢印**: 通過可能な対角線ビーム（遮蔽駒なし）
- 🔴 **赤い矢印/丸**: 遮蔽駒（味方歩）で遮断される方向

### 概要

Qugiy 方式は**減算の借位伝播**と **`byte_reverse`（バイト反転）** の 2 つを組み合わせます。[^sugyan-lztz]
やねうら王の開発者による[完全解説記事](https://yaneuraou.yaneu.com/2021/12/03/qugiys-jumpy-effect-code-complete-guide/)では、このアルゴリズムが 4 段階のアイデアの積み上げであることが解説されています。[^yaneuraou-qugiy-guide]

1. **加算による繰り上がり**: `occ + pawnStepEffect(sq)`（`pawnStepEffect` はやねうら王の呼称。rsshogi の `*_step_attacks` に相当）で最近接遮蔽駒まで繰り上がりが伝播する性質を利用
2. **減算への改良**: `(occ − 1) ^ occ` に書き換えることで、参照テーブルが不要になる
3. **`byte_reverse` で逆方向処理**: [SSSE3 の `pshufb`](../simd/instructions.md#ssse3-バイトシャッフル) でバイト順を反転し、同じ減算トリックで逆方向の利きも一括処理
4. **128bit decrement の並列化**: [AVX2](../simd/instructions.md#avx2-256-ビット整数演算) の unpack 命令で lo/hi を分離し、`Bitboard256` で 4 方向を同時に `decrement` 処理

CPU 依存性が小さく、[AMD Zen2 の PEXT 問題](../simd/history.md#amd-の-bmi2pextpdep問題)を回避しつつ多様な環境で安定して高速です。
Magic Bitboard のようなテーブル探索が不要で、実装が簡潔な点も採用理由です。

### ビーム＆遮蔽駒検出の流れ

飛車の4方向ビームを段階的に可視化するアニメーションです。
各フレームで1方向ずつビームを表示し、遮蔽駒の検出を示します。

<div id="atk-lztz-step" style="width:600px;height:660px;margin:1rem auto;border:1px solid #ddd;"></div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('atk-lztz-step');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: false });
  // 5五に先手飛、3五に後手歩（右方向遮蔽駒）、5二に後手銀（上方向遮蔽駒）
  const sfen = '9/4s4/9/9/4R1p2/9/9/9/9 b - 1';
  board.animate([
    // Step 1: 上方向ビーム → 5二の銀で停止
    {
      sfen: sfen,
      arrows: [
        { from: '5e', to: '5b', color: 'rgba(0,120,215,0.8)', width: 1.5 },
      ],
      circles: [
        { square: '5b', color: 'rgba(220,50,50,0.7)', width: 2.0 },
      ],
      duration: 2000,
    },
    // Step 2: 下方向ビーム → 端まで到達
    {
      sfen: sfen,
      arrows: [
        { from: '5e', to: '5b', color: 'rgba(0,120,215,0.4)', width: 0.8 },
        { from: '5e', to: '5i', color: 'rgba(0,180,0,0.8)', width: 1.5 },
      ],
      duration: 2000,
    },
    // Step 3: 右方向ビーム → 3五の歩で停止
    {
      sfen: sfen,
      arrows: [
        { from: '5e', to: '5b', color: 'rgba(0,120,215,0.4)', width: 0.8 },
        { from: '5e', to: '5i', color: 'rgba(0,180,0,0.4)', width: 0.8 },
        { from: '5e', to: '3e', color: 'rgba(255,140,0,0.8)', width: 1.5 },
      ],
      circles: [
        { square: '3e', color: 'rgba(220,50,50,0.7)', width: 2.0 },
      ],
      duration: 2000,
    },
    // Step 4: 左方向ビーム → 端まで到達
    {
      sfen: sfen,
      arrows: [
        { from: '5e', to: '5b', color: 'rgba(0,120,215,0.4)', width: 0.8 },
        { from: '5e', to: '5i', color: 'rgba(0,180,0,0.4)', width: 0.8 },
        { from: '5e', to: '3e', color: 'rgba(255,140,0,0.4)', width: 0.8 },
        { from: '5e', to: '9e', color: 'rgba(128,0,255,0.8)', width: 1.5 },
      ],
      duration: 2000,
    },
    // Step 5: 全方向の合成結果
    {
      sfen: sfen,
      arrows: [
        { from: '5e', to: '5b', color: 'rgba(0,120,215,0.7)', width: 1.2 },
        { from: '5e', to: '5i', color: 'rgba(0,120,215,0.7)', width: 1.2 },
        { from: '5e', to: '3e', color: 'rgba(0,120,215,0.7)', width: 1.2 },
        { from: '5e', to: '9e', color: 'rgba(0,120,215,0.7)', width: 1.2 },
      ],
      circles: [
        { square: '5b', color: 'rgba(220,50,50,0.7)', width: 1.5 },
        { square: '3e', color: 'rgba(220,50,50,0.7)', width: 1.5 },
      ],
      duration: 3000,
    },
  ], { loop: true, autoPlay: true });
  window.atkLztzStep = board;
}
</script>

**Qugiy 方式のステップ（飛車の場合）**:
1. **縦方向**: 香車の利き（`lance_attacks`）を先手・後手の両方向で呼び出して合成
2. **横方向**: 占有を `byte_reverse` して順方向・逆方向を揃え、減算（`decrement_pair`）→ XOR で区間を抽出
3. **4方向を OR で合成** → 最終的な飛車の利き

### 実装の仕組み

rsshogi の遠方駒の利き計算は 3 つの関数で構成されます。

#### 香車（`lance_attacks`）

1方向のみに利く直線の遠方駒（香）で、先手（北向き）と後手（南向き）で処理が異なります。
香車は 128 ビットの `Bitboard` 単体で処理するため、SSE2 の基本命令のみで完結します。

```rust,ignore
// 後手（南向き）: 減算の借位伝播
let em = occ & mask;           // ビーム上の占有ビットを抽出
let t = em.wrapping_sub(1);    // 最下位の遮蔽駒まで借位が伝播
let result = (em ^ t) & mask;  // XOR で遮蔽駒までの区間を得る

// 先手（北向き）: leading_zeros で MSB を検出
//   BMI1 の LZCNT 命令で 1 サイクル実行
let msb = 63 - (mocc | 1).leading_zeros();
let fill = (!0u64) << msb;     // MSB 以上を全て 1 に
let result = fill & mask;       // ビームでマスクして利きを得る
```
<small>[`lance_attacks` のソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/board/attack_tables.rs#L80-L117)</small>

#### 角（`bishop_attacks`）の Bitboard256 並列処理

4方向の対角線を `Bitboard256`（[AVX2](../simd/instructions.md#avx2-256-ビット整数演算) の `__m256i`、256 ビット SIMD レジスタ）で並列に処理します。
`Bitboard256` は 128 ビットの `Bitboard` を 2 つ束ねたラッパーで、AVX2 対応 CPU では単一の 256 ビットレジスタで演算し、非対応環境では `[u64; 4]` のスカラーフォールバックに切り替わります（[SIMD 概論](../simd/index.md)参照）。

```rust,ignore
fn bishop_attacks(sq: Square, occupied: Bitboard) -> Bitboard {
    let mask_lo = QUGIY_BISHOP_MASK[sq][0];  // 事前計算マスク
    let mask_hi = QUGIY_BISHOP_MASK[sq][1];

    // splat: 同じ Bitboard を 256 ビットレジスタの上下に複製
    //   AVX2 では _mm256_broadcastsi128_si256 に相当
    let occ2 = Bitboard256::splat(occupied);
    // byte_reverse: SSSE3 の pshufb でバイト順を反転し、逆方向の処理を実現
    let rocc2 = Bitboard256::splat(occupied.byte_reverse());
    let (hi, lo) = Bitboard256::unpack(rocc2, occ2);

    // マスク適用 → 減算（decrement）→ XOR → マスク
    //   AVX2 では _mm256_and_si256, _mm256_sub_epi64, _mm256_xor_si256 を使用
    let hi = hi.and(mask_hi);
    let lo = lo.and(mask_lo);
    let (t1, t0) = Bitboard256::decrement(hi, lo);
    let t1 = t1.xor(hi).and(mask_hi);
    let t0 = t0.xor(lo).and(mask_lo);

    // 逆方向の結果を byte_reverse で戻して合成
    let (hi, lo) = Bitboard256::unpack(t1, t0);
    hi.byte_reverse().or(lo).merge()
}
```
<small>[`bishop_attacks` のソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/board/attack_tables.rs#L119-L138)</small>

#### 飛車（`rook_attacks`）

縦方向は香車 2 方向の合成、横方向は `byte_reverse`（[SSSE3 `pshufb`](../simd/instructions.md#byte_reverse-への応用)）+ `decrement_pair` で処理します。
角と異なり `Bitboard256` は使わず、128 ビットの `Bitboard` で完結します。

```rust,ignore
fn rook_attacks(sq: Square, occupied: Bitboard) -> Bitboard {
    // 縦方向: 香車の先手・後手ビームを合成
    let mut attacks = lance_attacks(sq, occupied, Color::BLACK);
    attacks |= lance_attacks(sq, occupied, Color::WHITE);

    // 横方向: byte_reverse + 差分抽出
    let mask_lo = QUGIY_ROOK_MASK[sq][0];
    let mask_hi = QUGIY_ROOK_MASK[sq][1];
    let occ_rev = occupied.byte_reverse();
    let (hi, lo) = Bitboard::unpack(occ_rev, occupied);
    let hi = hi.and_raw(mask_hi);
    let lo = lo.and_raw(mask_lo);
    let (t1, t0) = Bitboard::decrement_pair(hi, lo);
    let t1 = t1.xor_raw(hi).and_raw(mask_hi);
    let t0 = t0.xor_raw(lo).and_raw(mask_lo);
    let (hi, lo) = Bitboard::unpack(t1, t0);
    attacks | hi.byte_reverse() | lo
}
```
<small>[`rook_attacks` のソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/board/attack_tables.rs#L140-L159)</small>

### 事前計算マスク（build.rs）

`QUGIY_BISHOP_MASK` と `QUGIY_ROOK_MASK` は `build.rs` でビルド時に生成されます。
各マスごとに、対角線や段方向のビームを `byte_reverse` 対応形式でパックしたマスクです。
`LANCE_BEAMS` は筋方向の先手・後手ビームを事前計算しています。

## 将棋盤を用いた可視化

理解を助けるため、任意の局面を表示できる簡易な将棋盤レンダラを用意しています。
`docs/book/src/assets/shogi-board.ts` を参照してください。

このレンダラは画像の作成やアニメーションの作成に適しており、ドキュメント内の図示を容易にします。

## 落とし穴

### 香車の方向間違い

香車は先手なら上方向、後手なら下方向にのみ移動します。
`lance_attacks(sq, occ, color)` に渡す `color` を間違えると、逆方向に利きが生成され、合法手が大量に誤生成されます。
テストでは先手・後手の両方の香車を必ず検証してください。

### 桂馬の成りゾーン

桂馬は先手なら 1-2 段目に移動した場合**必ず成る必要があります**（行き所のない駒になるため）。
3 段目への移動は成り/不成の選択があります。
成り判定を忘れると、1-2 段目に不成の桂馬が生成される不正な状態になります。

### 成駒の利き合成

馬の利きは「角の利き + 王の利き」、龍の利きは「飛の利き + 王の利き」です。
角の利きだけで馬の利きとしてしまうと、隣接 4 マスへの移動が欠落します。

#### 龍の利き = 飛車 + 王

龍（成り飛車）の利きは飛車の直線利きと王の隣接8マスの OR 合成です。
青い矢印が飛車成分（直線利き）、オレンジの矢印が王成分（隣接斜め4マス）です。

<div id="atk-dragon-demo" style="width:600px;height:660px;margin:1rem auto;border:1px solid #ddd;"></div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('atk-dragon-demo');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: false });
  // 5五に先手龍
  board.setPositionFromSFEN('9/9/9/9/4+R4/9/9/9/9 b - 1');
  board.goTo(0);
  // 飛車成分（直線利き）
  board.setArrows([
    { from: '5e', to: '5a', color: 'rgba(0,120,215,0.6)', width: 1.2 },
    { from: '5e', to: '5i', color: 'rgba(0,120,215,0.6)', width: 1.2 },
    { from: '5e', to: '1e', color: 'rgba(0,120,215,0.6)', width: 1.2 },
    { from: '5e', to: '9e', color: 'rgba(0,120,215,0.6)', width: 1.2 },
    // 王成分（斜め隣接4マス）
    { from: '5e', to: '4d', color: 'rgba(255,140,0,0.7)', width: 1.0 },
    { from: '5e', to: '6d', color: 'rgba(255,140,0,0.7)', width: 1.0 },
    { from: '5e', to: '4f', color: 'rgba(255,140,0,0.7)', width: 1.0 },
    { from: '5e', to: '6f', color: 'rgba(255,140,0,0.7)', width: 1.0 },
  ]);
  board.setCircles([
    { square: '5e', color: 'rgba(0,120,215,0.4)' },
  ]);
  window.atkDragonDemo = board;
}
</script>

```rust,ignore
// 龍の利き = 飛車の利き | 王の利き
let dragon_attacks = rook_attacks(sq, occ) | KING_ATTACKS[sq];
// 馬の利き = 角の利き | 王の利き（直交4マス）
let horse_attacks = bishop_attacks(sq, occ) | KING_ATTACKS[sq];
```

## SIMD 命令の利用マップ

利きの計算で使われる主な SIMD/拡張命令の対応関係です。
各命令の詳細は [拡張命令リファレンス](../simd/instructions.md) を参照してください。

| 処理 | 使用命令 | ISA | 備考 |
|------|---------|-----|------|
| Bitboard の AND/OR/XOR | `_mm_and_si128` 等 | [SSE2](../simd/instructions.md#sse2-128-ビット整数論理演算) | 全演算の基盤 |
| `byte_reverse` | `_mm_shuffle_epi8` | [SSSE3](../simd/instructions.md#ssse3-バイトシャッフル) | Qugiy 方式の逆方向処理 |
| `pop_lsb()` | `tzcnt` | [BMI1](../simd/instructions.md#bmi1-tzcnt--lzcnt--andn) | ビット列挙 |
| `count()` | `popcnt` | [POPCNT](../simd/instructions.md#popcnt-ビットカウント) | 駒数カウント |
| `Bitboard256` 演算 | `_mm256_*` | [AVX2](../simd/instructions.md#avx2-256-ビット整数演算) | 角の 4 方向並列 |
| `lance_attacks` の MSB 検出 | `lzcnt` | [BMI1](../simd/instructions.md#lzcntleading-zero-count) | 先手香の遮蔽駒検出 |

> **フォールバック**: AVX2 非対応環境では `Bitboard256` がスカラー実装（`[u64; 4]`）に切り替わります。
> SSE2 は x86-64 の必須仕様のため、基本的な 128 ビット演算は常に利用可能です。

## まとめ

- ステップ駒: テーブル参照 1 回 + マスク演算 → 数命令で完了
- 遠方駒: Qugiy 方式（差分抽出＋`byte_reverse`）で方向ごとの利きを O(1) 算出
- 駒打ち: 空マスから二歩・行き所のない駒をビットマスクで除外
- 成駒の利きは元の駒 + 王の利きで合成（見落としやすいバグ源）
- SIMD は利き計算の全レイヤーに関与: SSE2（基本演算）、SSSE3（`byte_reverse`）、AVX2（角の並列処理）

---

## 次に読む

→ **[飛び利きアルゴリズム比較](./slider-survey.md)**: Qugiy, Magic, PEXT, Rotated, LZ/TZ の詳細比較に進みます。

## 参考文献

[^psilord-rep]: psilord, ["Representation of a Chess Board with a Bitboard"](https://pages.cs.wisc.edu/~psilord/blog/data/chess-pages/rep.html) — ステップ駒の攻撃テーブル参照
[^sugyan-lztz]: すぎゃーんメモ, ["Bitboardでleading/trailing zerosを使って遠方駒の利きを求める"](https://memo.sugyan.com/entry/2022/06/17/001400) — LZ/TZ 法と差分抽出法の日本語解説
[^yaneuraou-qugiy-guide]: やねうら王, ["Qugiyの飛び利きのコード、完全解説"](https://yaneuraou.yaneu.com/2021/12/03/qugiys-jumpy-effect-code-complete-guide/) (2021) — 加算→減算→byte_reverse→128bit decrement の 4 段階アイデア発展と、やねうら王への統合経緯
[^yaneuraou-nifu]: やねうら王, ["縦型Bitboardの唯一の弱点を克服する"](https://yaneuraou.yaneu.com/2015/10/15/%E7%B8%A6%E5%9E%8Bbitboard%E3%81%AE%E5%94%AF%E4%B8%80%E3%81%AE%E5%BC%B1%E7%82%B9%E3%82%92%E5%85%8B%E6%9C%8D%E3%81%99%E3%82%8B/) (2015) — 二歩判定のための PEXT + 加算トリック。縦型 Bitboard で歩の打てるマスを高速判定する手法
