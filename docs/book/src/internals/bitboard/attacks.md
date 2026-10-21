# 利き計算とテーブル生成

将棋の駒の利き（あるマスから移動できるマスの集合）は、周囲の駒の有無に依存しない **近接駒（Step Pieces）** と、途中の遮蔽物（ブロッカー）で利きが止まる **飛び駒（Sliding Pieces：香、角、飛、馬、龍）** の 2 種類に分かれる。
`rsshogi` では、`build.rs` がコンパイル時に `$OUT_DIR/attack_tables.rs` へ出力する定数テーブルと、`crates/rsshogi/src/board/attack_tables.rs` の実行時ビット演算アルゴリズムを組み合わせて利きを算出している。

---

## 1. 近接駒の利きテーブルと王手候補テーブル

歩、桂、銀、金（および成金 4 種）、玉の利きは、盤上の他駒の配置に一切依存しない。
そのため、`build.rs` で全 81 マス分の `Bitboard` を事前計算し、`SquareTable<Bitboard, 81>` として静的に保持している。

| テーブル定数 | 型 | 内容 |
| :--- | :--- | :--- |
| `PAWN_ATTACKS` | `[SquareTable<Bitboard, 81>; 2]` | 先手と後手それぞれの歩の利き（前方 1 マス）。 |
| `KNIGHT_ATTACKS` | `[SquareTable<Bitboard, 81>; 2]` | 先手と後手それぞれの桂馬の利き（前方 2 段、左右 1 筋の最大 2 マス）。 |
| `SILVER_ATTACKS` | `[SquareTable<Bitboard, 81>; 2]` | 先手と後手それぞれの銀の利き（前方 3 マス + 斜め後方 2 マス）。 |
| `GOLD_ATTACKS` | `[SquareTable<Bitboard, 81>; 2]` | 先手と後手それぞれの金および成金（と金、成香、成桂、成銀）の利き（6 マス）。 |
| `KING_ATTACKS` | `SquareTable<Bitboard, 81>` | 先後共通の玉の利き（周囲 8 マス）。 |
| `CHECK_CANDIDATE_BB` | `[[[Bitboard; 2]; 7]; 81]` | 玉の位置 `king_sq` に対し、各駒種および手番が 1 手移動（成と不成を含む）で王手をかけられる移動元候補マスのマスク。 |

近接駒の利き取得関数（`pawn_attacks`、`knight_attacks`、`silver_attacks`、`gold_attacks`、`king_attacks`）は、すべて上記テーブルの 1 回の配列参照（O(1)）のみで完結する。

以下のアニメーションは、`5五` に配置した先手の近接駒（歩、桂、銀、金、玉）の利きパターンを順に表示している。

<div id="atk-step-patterns" style="width:550px;height:510px;margin:1rem auto;border:1px solid #ddd;"></div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('atk-step-patterns');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: false });
  board.animate([
    {
      sfen: '9/9/9/9/4P4/9/9/9/9 b - 1',
      arrows: [
        { from: '5e', to: '5d', color: 'rgba(0,120,215,0.7)', width: 1.5 },
      ],
      circles: [{ square: '5e', color: 'rgba(0,120,215,0.4)' }],
      duration: 2500,
    },
    {
      sfen: '9/9/9/9/4N4/9/9/9/9 b - 1',
      arrows: [
        { from: '5e', to: '4c', color: 'rgba(255,140,0,0.7)', width: 1.5 },
        { from: '5e', to: '6c', color: 'rgba(255,140,0,0.7)', width: 1.5 },
      ],
      circles: [{ square: '5e', color: 'rgba(255,140,0,0.4)' }],
      duration: 2500,
    },
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

---

## 2. 縦方向の飛び駒（香車と飛車縦）：`FILE_ATTACKS` による 9 ビット直接参照

将棋の飛び駒のうち、**縦方向（同じ筋の上下移動）** は香車、飛車、龍のすべてで頻出する。
ここで、`Bitboard` の [63+18 分割レイアウト](layout.md) が決定的な威力を発揮する。

File-major 座標系（`sq = file * 9 + rank`）と 63+18 分割により、任意の筋 `file` の 9 マス（一段目〜九段目）は、`occupied.packed_bits()` のビット `file * 9 .. file * 9 + 9`（すなわち `p[0]` または `p[1]` 内の連続した 9 ビット）にそのまま並んでいる。
したがって、PEXT 命令（BMI2）やマジックナンバー乗算を一切使わず、**1 回の右シフトと `0x1ff` マスクだけ** でその筋の占有状態 `file_occupied`（`0..512`）を取り出せる。

```rust
const FILE_ATTACKS: [[u16; 512]; 9] = [ ... ]; // 9 段 × 512 通り = 9 KB

pub fn rook_file_attacks(sq: Square, occupied: Bitboard) -> Bitboard {
    let idx = sq.to_index();
    let file_start = (idx / 9) * 9;
    let rank = idx - file_start;
    let file_occupied = ((occupied.packed_bits() >> file_start) & 0x1ff) as usize;
    let file_mask = FILE_ATTACKS[rank][file_occupied] as u128;
    Bitboard::from_u128(file_mask << file_start)
}
```

- **テーブルサイズ**：段の位置 `rank`（`0..9`）× 筋内の 9 ビット占有パターン（`2^9 = 512` 通り）× `u16`（2 バイト）= **9,216 バイト（9 KB）** であり、CPU の L1 データキャッシュ（通常 32 KB〜48 KB）に余裕をもって常駐する。
- **香車の利き（`lance_attacks`）**：`rook_file_attacks(sq, occupied)` で得た上下両方向の縦利きに対し、手番に応じた前方マスク `LANCE_FORWARD_MASKS[color][sq]` との論理積（AND）をとるだけで求まる。

---

## 3. 斜めおよび横方向の飛び駒（角と飛車）：Qugiy（減算と XOR）と AVX2 双方向レイ計算

角（斜め 4 方向）および飛車の横方向を含む全方向利きは、1 つの筋の中に収まらないため、レイ（光線）上の最初のブロッカー（遮蔽駒）をビット演算で特定する **Qugiy（Subtracting a Rook from a Blocking Piece / Hyperbola Quintessence 系）アルゴリズム** を用いている。

以下の盤面は、`5五` の先手角から伸びる 4 方向の斜めレイのうち、右上（`1一`）、右下（`1九`）、左下（`9九`）は盤端まで通り、左上方向のみ `6四` の歩（赤丸）で遮断される様子を示している。

<div id="lz-tz-live-1" style="width:550px;height:510px;margin:1rem auto;border:1px solid #ddd;"></div>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('lz-tz-live-1');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: false });
  board.setPositionFromSFEN('9/9/9/3P5/4B4/9/9/9/9 b - 1');
  board.goTo(0);
  board.setArrows([
    { from: '5e', to: '1a', color: 'rgba(0,120,215,0.6)', width: 1.2 },
    { from: '5e', to: '1i', color: 'rgba(0,120,215,0.6)', width: 1.2 },
    { from: '5e', to: '9i', color: 'rgba(0,120,215,0.6)', width: 1.2 },
    { from: '5e', to: '6d', color: 'rgba(220,50,50,0.6)', width: 1.2 },
  ]);
  board.setCircles([
    { square: '6d', color: 'rgba(220,50,50,0.8)', width: 2.0 },
    { square: '5e', color: 'rgba(0,120,215,0.4)' },
  ]);
  window.lzTzLive1 = board;
}
</script>

### 正方向レイ（インデックス増加方向）の基本原理

あるマスからインデックスが増加する方向へ伸びるレイのビットマスクを `ray_bits`、盤面全体の占有状態を `occupied_bits` とする。
この方向にある遮蔽駒の集合は `blockers = ray_bits & occupied_bits` である。

ここで `blockers` から `1` を減算（`blockers.wrapping_sub(1)`）すると、**最も手前にある遮蔽駒（最下位ビット LSB）が `1` から `0` に変わり、それより手前のビットがすべて `0` から `1` に反転する**。
一方、最初の遮蔽駒より奥にあるビットは減算の影響を一切受けない。

```
ray_bits      : 0010_0100_1000_0000  (レイ上のマス)
blockers      : 0010_0100_0000_0000  (2 箇所に駒がある)
blockers - 1  : 0010_0011_1111_1111  (最初のブロッカー LSB から手前がすべて反転)
blockers ^ (blockers - 1)
              : 0000_0111_1111_1111  (最初のブロッカー自身を含み、それより手前がすべて 1)
ray_bits & (blockers ^ (blockers - 1))
              : 0000_0100_1000_0000  (最初のブロッカーのマスでピタリと止まる利き)
```

この性質を利用した `increasing_ray_attacks` は、条件分岐を一切含まない 3 つのビット演算で正方向の利きを確定させる。

```rust
const fn increasing_ray_attacks(ray_bits: u128, occupied_bits: u128) -> u128 {
    let blockers = ray_bits & occupied_bits;
    let diff = blockers ^ blockers.wrapping_sub(1);
    ray_bits & diff
}
```

### 負方向レイ（インデックス減少方向）と `reverse_bits`

インデックスが減少する方向へ伸びるレイでは、最も手前にある遮蔽駒が最下位ビット（LSB）ではなく **最上位ビット（MSB）** になるため、そのままでは `blockers - 1` が使えない。
そこで `rsshogi` は、128 ビット全体のビット順序を逆転させる `reverse_bits(u128) -> u128` を用いる。

1. `build.rs` において、負方向レイのマスク（角の `sw_reversed` / `nw_reversed`、飛車の `n_reversed` / `w_reversed`）をあらかじめ **`raw_bits()` レイアウト上で 128 ビット反転させた定数** として生成しておく。
2. 実行時には、盤面の占有ビットボード `occupied.raw_bits()` を 1 回だけ `reverse_bits` でビット反転させた `reversed_occupied` を作る。
3. ビット順序が逆転した空間では、負方向レイも「インデックスが増加する正方向レイ」と完全に同型になるため、同じ `increasing_ray_attacks` を適用できる。
4. 得られた結果を再度 `reverse_bits` で元のビット順序に戻し、正方向の結果と OR 合成する。

以下のアニメーションは、`5五` の先手飛車から上下左右の 4 方向へ伸びるレイが、`5二` の後手銀および `3五` の後手歩の位置で止まり、最終的に 4 方向の利きとして OR 合成される過程を示している。

<div id="atk-lztz-step" style="width:550px;height:510px;margin:1rem auto;border:1px solid #ddd;"></div>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('atk-lztz-step');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: false });
  const sfen = '9/4s4/9/9/4R1p2/9/9/9/9 b - 1';
  board.animate([
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
    {
      sfen: sfen,
      arrows: [
        { from: '5e', to: '5b', color: 'rgba(0,120,215,0.4)', width: 0.8 },
        { from: '5e', to: '5i', color: 'rgba(0,180,0,0.8)', width: 1.5 },
      ],
      duration: 2000,
    },
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

### AVX2 による 2 方向同時レイ計算（`paired_increasing_attacks`）

`x86_64` で AVX2 および SSE2 が利用可能な環境では、`attack_tables.rs` 内の SIMD 高速パスが有効になる。

1. **SSSE3 / AVX2 シャッフルによる高速 `reverse_bits`**：
   128 ビットのビット反転は、`_mm_shuffle_epi8`（`pshufb`）で 16 バイトの並び順を逆転させると同時に、4 ビット（ニブル）単位の 16 要素ルックアップテーブル `[0, 8, 4, 12, 2, 10, 6, 14, 1, 9, 5, 13, 3, 11, 7, 15]` を `_mm_shuffle_epi8` で並列引きすることで、わずか数クロックで完了する。
2. **256 ビット `__m256i` による 2 方向同時計算（`paired_increasing_attacks`）**：
   角の正方向 2 本（`ne` と `se`）や負方向 2 本（`sw_reversed` と `nw_reversed`）、あるいは飛車の正方向 2 本（`e_raw` と `s_raw`）と負方向 2 本（`n_reversed` と `w_reversed`）は、それぞれ 2 本の 128 ビットレイを 1 個の 256 ビット AVX2 レジスタ（`__m256i`）にパックして **2 方向同時に `ray & (blockers ^ (blockers - 1))` を計算** する。
   このとき、AVX2 には 128 ビット整数単位の減算命令が存在しないため、64 ビット単位の加算 `_mm256_add_epi64(blockers, -1)` を行った上で、下位 64 ビットレーン（`p[0]`）が `0` だった場合にのみ上位 64 ビットレーン（`p[1]`）からボロー（桁借り）を引く処理を `_mm256_cmpeq_epi64(blockers, zero)` と `_mm256_slli_si256::<8>` の組み合わせで分岐なしに実現している。

---

## 4. 飛車、角、馬、龍の利き関数の使い分け

`attack_tables.rs` が提供する飛び駒の利き関数は、呼び出し元の文脈に応じて最適なアルゴリズムを選択できるよう以下の通り構成されている。

| 関数名 | 計算アルゴリズム | 主な用途 |
| :--- | :--- | :--- |
| `lance_attacks(color, sq, occupied)` | `rook_file_attacks` + 前方マスク `LANCE_FORWARD_MASKS` | 香車の利き計算。 |
| `bishop_attacks(sq, occupied)` | 正方向 2 本（`ne`, `se`）と負方向 2 本（`sw_reversed`, `nw_reversed`）を `paired_increasing_attacks` + `reverse_bits` で計算 | 角の 4 方向利き計算。 |
| `rook_attacks(sq, occupied)` | 正方向 2 本（`e_raw`, `s_raw`）と負方向 2 本（`n_reversed`, `w_reversed`）を `paired_increasing_attacks` + `reverse_bits` で計算 | 飛車の 4 方向一括利き計算。 |
| `rook_file_attacks(sq, occupied)` | `(occupied.packed_bits() >> file_start) & 0x1ff` による `FILE_ATTACKS` テーブル参照 | 飛車の縦方向のみの利き抽出（1 手詰めソルバの差分利き計算等）。 |
| `rook_rank_attacks(sq, occupied)` | `rook_attacks(sq, occupied) & Bitboard::rank_mask(sq.rank())` | 飛車の横方向のみの利き抽出。 |
| `horse_attacks(sq, occupied)` | `bishop_attacks(sq, occupied) \| king_attacks(sq)` | 馬の利き計算。 |
| `dragon_attacks(sq, occupied)` | `rook_attacks(sq, occupied) \| king_attacks(sq)` | 龍の利き計算。 |

大駒の成駒である馬（`HORSE`）と龍（`DRAGON`）の利きは、専用のテーブルを持たず、元の飛び駒の利きと `king_attacks(sq)` の 1 回の論理和（OR）で合成される。
以下の盤面は、`5五` の龍の利きが飛車の直線利き（青矢印）と玉の斜め隣接 4 マス（オレンジ矢印）の合成で表される様子を示している。

<div id="atk-dragon-demo" style="width:550px;height:510px;margin:1rem auto;border:1px solid #ddd;"></div>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('atk-dragon-demo');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: false });
  board.setPositionFromSFEN('9/9/9/9/4+R4/9/9/9/9 b - 1');
  board.goTo(0);
  board.setArrows([
    { from: '5e', to: '5a', color: 'rgba(0,120,215,0.6)', width: 1.2 },
    { from: '5e', to: '5i', color: 'rgba(0,120,215,0.6)', width: 1.2 },
    { from: '5e', to: '1e', color: 'rgba(0,120,215,0.6)', width: 1.2 },
    { from: '5e', to: '9e', color: 'rgba(0,120,215,0.6)', width: 1.2 },
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
