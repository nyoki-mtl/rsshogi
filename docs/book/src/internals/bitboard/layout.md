# レイアウト（盤サイズとビット順序）

> **前提知識**: [Bitboard の概要](./index.md)（集合演算の動機と基本構造）

## このページの要点

- 将棋は 81 マスのため 64 ビットに収まらず、**128 ビット表現**（`[u64; 2]`）を採用する
- ビット順序は「**縦型（file-major）**」で、同一筋のマスが連続ビットに並ぶ
- 縦型の利点は、香車の利き計算・歩の一括シフト・筋マスクの簡潔さに直結する
- `[u64; 2]` の lo/hi 分割境界は **bit 62 と bit 63 の間**（7筋と8筋の境界）にある

## なぜ 128 ビットが必要か

チェスは 8×8 = 64 マスなので、1 つの `u64` で盤面全体を表現できます。[^psilord-rep]
しかし将棋は 9×9 = **81 マス**であり、64 ビットでは 17 マス分不足します。

```text
チェス: 64 マス → u64 (1 レジスタ)
将棋:  81 マス → u64 では 17 マス足りない → u128 or [u64; 2] が必要
```

rsshogi は `[u64; 2]`（lo: 下位 63 ビット、hi: 上位 18 ビット）を採用し、合計 81 ビットを使用します。
x86_64 + [SSE2](../simd/instructions.md#sse2-128-ビット整数論理演算) 環境では `__m128i` との union も定義し、AND/OR/XOR/シフトなどの論理演算を 128 ビット幅で 1 命令実行できます。
SSE2 は x86-64 仕様に含まれるため、64 ビット x86 CPU であれば必ず利用可能です（[SIMD の歴史](../simd/history.md)参照）。

### 他エンジンとの比較

| エンジン | 表現 | レイアウト | 備考 |
|---------|------|-----------|------|
| **rsshogi** | `[u64; 2]` + SSE2 union | 縦型（file-major） | 互換 |
| **参照実装** | `[u64; 2]` + SSE2 union | 縦型 | 現代将棋エンジンの標準 |
| **Apery** | `[u64; 2]` | 縦型 | 同系統の設計 |
| **Bonanza** | 独自 128bit | 横型（rank-major） | 歴史的に重要な先駆者 |

### ビットインデックスの全体像

以下の盤面で、1一（bit 0）と 9九（bit 80）をハイライトしています。
矢印は bit 0 → bit 80 へのインデックスの増加方向を示しています。
縦型レイアウトなので、**まず段方向（下）に進み、筋が変わると左隣の筋へジャンプ**します。

<div id="layout-grid-0" style="width:600px;height:660px;margin:1rem auto;border:1px solid #ddd;"></div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('layout-grid-0');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: false });
  board.setPositionFromSFEN('9/9/9/9/9/9/9/9/9 b - 1');
  board.highlightSquares([0, 80]);
  // bit 0(1一) から bit 8(1九) への方向（段方向のインデックス増加）
  board.setArrows([
    { from: '1a', to: '1i', color: 'rgba(0,120,215,0.6)', width: 1.2 },
    { from: '1i', to: '2a', color: 'rgba(255,140,0,0.5)', width: 0.8 },
    { from: '2a', to: '2i', color: 'rgba(0,120,215,0.6)', width: 1.2 },
  ]);
  // 起点と終点に丸
  board.setCircles([
    { square: '1a', color: 'rgba(0,180,0,0.7)', width: 2.0 },
    { square: '9i', color: 'rgba(220,50,50,0.7)', width: 2.0 },
  ]);
  board.goTo(0);
  window.layoutGrid0 = board;
}
</script>

- 🟢 **緑の丸**: bit 0（1一）= LSB
- 🔴 **赤の丸**: bit 80（9九）
- 🔵 **青の矢印**: 同一筋内のインデックス増加方向（段方向）
- 🟠 **オレンジの矢印**: 筋をまたぐジャンプ（1九 → 2一）

## ビット順序（1一 → 9九）

インデックスは `file * 9 + rank` の縦型レイアウトで 0..80 にマッピングします。
下位ビットから順に １一 → １二 → ... → １九 → ２一 → ... → ９九 と進みます。

```text
Square(0) = １一, Square(8) = １九, Square(9) = ２一, ..., Square(80) = ９九
```

主要な API:
- `Bitboard::from_square(sq)`: 単一ビットを立てる。
- `Bitboard::pop_lsb()`: 最下位ビットを取り出して除去。
- `for sq in bitboard`: Iterator で全ビットを列挙。

## なぜ縦型（file-major）レイアウトか

縦型レイアウトでは、同一筋（file）のマスが連続するビット位置に配置されます。
これが将棋エンジンに適する理由は 3 つあります。

### 1. 香車の利き計算が単純になる

香車は筋方向にのみ移動します。
縦型なら香車の利き範囲が**連続ビットの区間**として表現できるため、マスク演算やシフトだけで利きを生成できます。
横型だと 9 ビット間隔で飛び飛びにチェックする必要があり、演算が複雑化します。

<div id="layout-lance-demo" style="width:600px;height:660px;margin:1rem auto;border:1px solid #ddd;"></div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('layout-lance-demo');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: false });
  // 5九に先手香、5四に先手歩（味方遮蔽駒）
  board.setPositionFromSFEN('9/9/9/4P4/9/9/9/9/4L4 b - 1');
  board.goTo(0);
  // 香の利き方向を矢印で表示（5九→5五まで、5四の味方歩でブロック）
  board.setArrows([
    { from: '5i', to: '5e', color: 'rgba(0,120,215,0.7)', width: 1.5 },
  ]);
  // 連続ビットの区間をハイライト（5五〜5八）
  const lanceRange = [];
  for (let r = 4; r < 8; r++) lanceRange.push(4 * 9 + r);  // 5筋の5段〜8段
  board.highlightSquares(lanceRange);
  // 遮蔽駒に丸
  board.setCircles([
    { square: '5d', color: 'rgba(220,50,50,0.7)', width: 1.5 },
  ]);
  window.layoutLanceDemo = board;
}
</script>

縦型レイアウトでは、香車の利き（5八→5五）が **bit 40, 41, 42, 43** の連続区間になります。
`trailing_zeros` で最初の遮蔽駒を検出し、マスクだけで利きを切り出せます。[^healeycodes-viz]

### 2. 歩の一括シフトが 1 命令で済む

歩は全て筋方向に 1 マス進みます。
縦型なら全歩のビットボードに `>> 1`（先手の場合）を適用するだけで、全歩の移動先を一括生成できます。
横型では `>> 9` が必要ですが、段境界を跨ぐビットの扱いが煩雑です。

<div id="layout-pawn-shift" style="width:600px;height:660px;margin:1rem auto;border:1px solid #ddd;"></div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('layout-pawn-shift');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: false });
  // 先手の歩が七段に並んでいる
  board.setPositionFromSFEN('9/9/9/9/9/9/PPPPPPPPP/9/9 b - 1');
  board.goTo(0);
  // 全歩の移動先（六段）を矢印で示す
  board.setArrows([
    { from: '1g', to: '1f', color: 'rgba(0,180,0,0.6)', width: 1.0 },
    { from: '3g', to: '3f', color: 'rgba(0,180,0,0.6)', width: 1.0 },
    { from: '5g', to: '5f', color: 'rgba(0,180,0,0.6)', width: 1.0 },
    { from: '7g', to: '7f', color: 'rgba(0,180,0,0.6)', width: 1.0 },
    { from: '9g', to: '9f', color: 'rgba(0,180,0,0.6)', width: 1.0 },
  ]);
  window.layoutPawnShift = board;
}
</script>

`pawn_targets = black_pawns >> 1`。たった 1 命令で全 9 枚の歩の移動先が得られます。

### 3. 筋マスクがシンプル

縦型では「n 筋のマス集合」が **9 ビット連続のブロック** になるため、`0x1FF << (n * 9)` で生成できます。
横型では 9 ビット間隔に散在するパターンとなり、定数テーブルが必要です。

<div id="layout-file-mask" style="width:600px;height:660px;margin:1rem auto;border:1px solid #ddd;"></div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('layout-file-mask');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: false });
  board.setPositionFromSFEN('9/9/9/9/9/9/9/9/9 b - 1');
  // 5筋のマスクをハイライト（全9マス）
  const file5 = [];
  for (let r = 0; r < 9; r++) file5.push(4 * 9 + r);
  board.highlightSquares(file5);
  // 5筋のマスクを丸で強調
  board.setCircles([
    { square: '5a', color: 'rgba(0,120,215,0.5)' },
    { square: '5i', color: 'rgba(0,120,215,0.5)' },
  ]);
  board.goTo(0);
  window.layoutFileMask = board;
}
</script>

`Bitboard::file_mask(File::FILE_5)`（内部テーブル `FILE_MASKS[4]`）= `0x1FF << 36`。5筋のマスクは連続 9 ビットのブロックです。
二歩判定はこのマスクと歩の AND + POPCNT で **3 命令** です。[^nereuxofficial-rust]

### 縦型 vs 横型まとめ

| 観点 | 縦型（file-major） | 横型（rank-major） |
|------|-------------------|-------------------|
| 筋マスク | `0x1FF << (file * 9)` で単純 | 9 ビット間隔で散在 |
| 香車の利き | 連続ビット区間 | 飛び飛びにチェック |
| 歩の一括移動 | `>> 1` で完結 | `>> 9` + 境界処理 |
| 段マスク | 9 ビット間隔で散在 | `0x1FF << (rank * 9)` で単純 |
| [SIMD](../simd/index.md) 親和性 | SSE2 `__m128i` に直接マッピング | 同等 |

rsshogi は [SIMD](../simd/index.md#理由-1-81-マス問題) との親和性と香車/歩の処理効率を優先し、縦型を採用しています。
内部表現は `[u64; 2]` で、x86_64 では SSE2 の `__m128i` を介して AND/OR/XOR が 1 命令で完結します。
さらに AVX2 環境では `Bitboard256`（`__m256i`）により角の 4 方向利きを並列処理します（[利きの計算](./attacks.md#角bishop_attacks-bitboard256-並列処理)参照）。

## `[u64; 2]` の lo/hi 分割

128 ビットを `[u64; 2]` で表現する場合、分割境界に注意が必要です。

```text
lo (u64): bit 0-62   → 1筋〜7筋の全9マス（計63マス）
hi (u64): bit 63-80  → 8筋〜9筋の全9マス（計18マス）
          bit 81-127 → 未使用（常に0）
```

実装上、`BOARD_MASK_LOW = 0x7FFF_FFFF_FFFF_FFFF`（63 ビット）、`BOARD_MASK_HIGH = 0x0000_0000_0003_FFFF`（18 ビット）です。
`high = packed >> 63` により、論理 bit 63（8一）が hi の bit 0 に格納されます。

### lo/hi 境界の可視化

黄色ハイライトが lo 領域（bit 0-62: 1筋〜7筋）です。
赤い丸が hi 領域の開始点（bit 63: 8一）です。
分割境界は **7筋と8筋の間**にあり、8筋はすべて hi 側に属します。

<div id="layout-lohi" style="width:600px;height:660px;margin:1rem auto;border:1px solid #ddd;"></div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('layout-lohi');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: false });
  board.setPositionFromSFEN('9/9/9/9/9/9/9/9/9 b - 1');
  // lo 領域: bit 0-62 (1筋〜7筋の全マス)
  const loIndices = [];
  for (let f = 0; f < 7; f++) for (let r = 0; r < 9; r++) loIndices.push(f * 9 + r);
  board.highlightSquares(loIndices);
  // lo/hi 境界を矢印で示す（7九から8一への境界）
  board.setArrows([
    { from: '7i', to: '8a', color: 'rgba(220,50,50,0.8)', width: 2.0 },
  ]);
  // lo の末尾（7九 = bit 62）と hi の開始（8一 = bit 63）に丸
  board.setCircles([
    { square: '7i', color: 'rgba(0,120,215,0.8)', width: 2.0 },
    { square: '8a', color: 'rgba(220,50,50,0.8)', width: 2.0 },
  ]);
  board.goTo(0);
  window.layoutLohi = board;
}
</script>

- 黄色マス: **lo** 領域（`p[0]`, bit 0-62）
- 🔵 **青の丸** (7九): lo の末尾ビット（bit 62）
- 🔴 **赤の丸** (8一): hi の開始ビット（bit 63）
- 🔴 **赤の矢印**: lo → hi の境界をまたぐ遷移

7筋と8筋をまたぐ演算では lo/hi の境界を跨ぐことがあり、キャリー伝播やマスク処理が必要になります。
rsshogi ではこの境界処理を `Bitboard` の内部メソッドに隠蔽し、利用側が意識する必要がない設計にしています。

## 座標系の直感

座標の向きと数字の付け方を盤面で確認できます。
上が一段、左が 9 筋です。

<div id="layout-live-1" style="width:600px;height:660px;margin:1rem auto;border:1px solid #ddd;"></div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('layout-live-1');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: false });
  // 9一に後手香、1九に先手香。
  board.setPositionFromSFEN('l8/9/9/9/9/9/9/9/8L b - 1');
  board.goTo(0);
  window.layoutLive1 = board;
}
</script>

## 180 度回転

後手側から見た表示にすると、盤は 180 度回転します。
この表示では筋番号も段番号も画面上では逆順になりますが、内部座標の意味は変わりません。

<div id="layout-live-2" style="width:600px;height:660px;margin:1rem auto;border:1px solid #ddd;"></div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('layout-live-2');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: false });
  board.setPositionFromSFEN('l8/9/9/9/9/9/9/9/8L b - 1');
  board.flip(true);
  board.goTo(0);
  window.layoutLive2 = board;
}
</script>

## 落とし穴

### lo/hi 境界を跨ぐビット演算

lo/hi の分割境界は 7筋と8筋の間（bit 62 と bit 63 の間）にあります。
7筋から8筋へ跨ぐ `Bitboard::shift()` やマスク演算でバグが生じやすいです。
常に `Bitboard` の提供する API を通じて操作し、`p[0]` / `p[1]` への直接アクセスは避けてください。

### 未使用ビット（bit 81-127）のゴミ

上位 47 ビットは常に 0 でなければなりません。
ビットシフトやマスク演算の結果、これらのビットにゴミが入ると `popcount()` や `any()` の結果が狂います。
rsshogi では内部的にマスクで保証していますが、低レベル操作を行う場合は `& BOARD_MASK` で上位ビットをクリアする必要があります。

## まとめ

- 将棋の 81 マスには 128 ビット表現が必要（64 ビットでは 17 マス不足）
- 縦型レイアウトにより、香車・歩・筋マスクの演算が大幅に簡潔化
- `[u64; 2]` の lo/hi 分割境界（bit 62-63、7筋/8筋の間）は API 内部に隠蔽
- 未使用ビット（81-127）のクリアに注意

## 次に読む

→ **[基本操作](./operations.md)**: マスク、シフト、`pop_lsb()` などの Bitboard 操作の詳細に進みます。

## 参考文献

[^healeycodes-viz]: healeycodes, ["Visualizing Chess Bitboards"](https://healeycodes.com/visualizing-chess-bitboards) — ビットボードの視覚化手法
[^nereuxofficial-rust]: Nereuxofficial, ["Writing a BitBoard in Rust"](https://nereuxofficial.github.io/posts/bitboard-rust/) — Rust でのビットボード実装
[^psilord-rep]: psilord, ["Representation of a Chess Board with a Bitboard"](https://pages.cs.wisc.edu/~psilord/blog/data/chess-pages/rep.html) — ビットボードの基本構造
