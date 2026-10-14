# メモリレイアウト（63+18 分割）

チェスの盤面（8 × 8 = 64 マス）が 1 個の `u64` にちょうど収まるのに対し、将棋の盤面は 9 × 9 = 81 マスあり、64 ビット汎用レジスタ 1 個には収まらない。
`rsshogi` の `Bitboard` は、16 バイト境界にアラインされた 2 個の 64 ビット整数 `p: [u64; 2]` に **63 ビット + 18 ビット** の筋単位分割で 81 マスを格納する。

---

## `Bitboard` の構造体定義とアラインメント

`crates/rsshogi/src/types/bitboard.rs` における `Bitboard` の定義は以下の通りである。

```rust
#[repr(C, align(16))]
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct Bitboard {
    p: [u64; 2],
}
```

- **`#[repr(C, align(16))]`**：構造体全体を 16 バイト（128 ビット）境界に整列させる。これにより x86_64 の SSE2 / SSE4.1 や AArch64 の NEON において、128 ビット SIMD レジスタ（`__m128i`）へのロードとストアがキャッシュラインを跨がず単一命令で実行される。
- **正規表現としての `[u64; 2]`**：`union` による型パニング（type-punning）を使わず、`[u64; 2]` を唯一のストレージ表現としている。これにより Miri による未定義動作検査や `const fn` 内でのコンパイル時評価と完全に両立する。

---

## 縦型（File-major）ビット順序の全体像

ビットインデックスは `raw = file * 9 + rank` の縦型レイアウトで `0..=80` にマッピングされる。
以下の盤面は、1 一（`bit 0`、緑の丸）から 9 九（`bit 80`、赤の丸）へのインデックス増加方向を示したものである。
まず同一筋の中を一段目から九段目へ向かって下方向（青い矢印）に進み、九段目に達すると左隣の筋の一段目へジャンプ（オレンジの矢印）する。

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
  board.setArrows([
    { from: '1a', to: '1i', color: 'rgba(0,120,215,0.6)', width: 1.2 },
    { from: '1i', to: '2a', color: 'rgba(255,140,0,0.5)', width: 0.8 },
    { from: '2a', to: '2i', color: 'rgba(0,120,215,0.6)', width: 1.2 },
  ]);
  board.setCircles([
    { square: '1a', color: 'rgba(0,180,0,0.7)', width: 2.0 },
    { square: '9i', color: 'rgba(220,50,50,0.7)', width: 2.0 },
  ]);
  board.goTo(0);
  window.layoutGrid0 = board;
}
</script>

- **緑の丸（1 一）**：`bit 0`（LSB）
- **赤の丸（9 九）**：`bit 80`
- **青の矢印**：同一筋内のインデックス増加方向（段方向）
- **オレンジの矢印**：筋を跨ぐ遷移（1 九 → 2 一）

### 1. 縦方向スライダ（香車・飛車の縦）の利きが連続ビット区間になる

縦型レイアウトでは、同一筋の 9 マスが 9 ビットの連続領域に並ぶ。
そのため、香車や飛車の縦方向の利き（たとえば以下の盤面における 5 九の先手香から 5 五までの 4 マス）は **`bit 40, 41, 42, 43` の連続ビット区間** となり、対象筋の 9 ビット占有状態をシフトとマスク（`(p[part] >> shift) & 0x1ff`）で切り出すだけで `FILE_ATTACKS` テーブルを直接引ける。

<div id="layout-lance-demo" style="width:600px;height:660px;margin:1rem auto;border:1px solid #ddd;"></div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('layout-lance-demo');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: false });
  board.setPositionFromSFEN('9/9/9/4P4/9/9/9/9/4L4 b - 1');
  board.goTo(0);
  board.setArrows([
    { from: '5i', to: '5e', color: 'rgba(0,120,215,0.7)', width: 1.5 },
  ]);
  const lanceRange = [];
  for (let r = 4; r < 8; r++) lanceRange.push(4 * 9 + r);
  board.highlightSquares(lanceRange);
  board.setCircles([
    { square: '5d', color: 'rgba(220,50,50,0.7)', width: 1.5 },
  ]);
  window.layoutLanceDemo = board;
}
</script>

### 2. 筋マスクが連続 9 ビットのブロックになる

「`n` 筋のマス集合（`Bitboard::file_mask(file)`）」は連続 9 ビットのブロック（`0x1FF << shift`）で表現される。
以下の盤面は 5 筋のマスク（`bit 36..=44`）をハイライトしたものであり、二歩判定はこの筋マスクと自歩ビットボードの論理積（`intersects`）だけで完結する。

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
  const file5 = [];
  for (let r = 0; r < 9; r++) file5.push(4 * 9 + r);
  board.highlightSquares(file5);
  board.setCircles([
    { square: '5a', color: 'rgba(0,120,215,0.5)' },
    { square: '5i', color: 'rgba(0,120,215,0.5)' },
  ]);
  board.goTo(0);
  window.layoutFileMask = board;
}
</script>

---

## なぜ「64 + 17」ではなく「63 + 18」分割なのか

81 マスを 2 個の `u64` に分割する際、単純に先頭から詰める「64 ビット + 17 ビット」分割を採用すると、**8 筋の一段目（`sq = 63`）だけが `p[0]` の最上位ビット（Bit 63）に入り、8 筋の二段目以降（`sq = 64..=71`）が `p[1]` に分断される**。
8 筋が `p[0]` と `p[1]` に跨ってしまうと、8 筋の香車や飛車の縦利き計算、および 8 筋の二歩判定のたびに 2 つの 64 ビット整数をシフトして合成する特別処理が必要になる。

これを防ぐため、`rsshogi` では `p[0]` の Bit 63 をあえて未使用（常に `0`）とし、**筋の境界（9 マス単位）に合わせて 63 ビット（1〜7 筋）と 18 ビット（8〜9 筋）に分割** している。

```text
p[0] (64 bits): 1 筋〜7 筋の 63 マスを格納 (Bit 63 は常に 0)
Bit:   63   62..54   53..45   44..36   35..27   26..18   17..9     8..0
     +----+--------+--------+--------+--------+--------+--------+--------+
     |  0 |  7 筋  |  6 筋  |  5 筋  |  4 筋  |  3 筋  |  2 筋  |  1 筋  |
     +----+--------+--------+--------+--------+--------+--------+--------+
           (sq54..) (sq45..) (sq36..) (sq27..) (sq18..) (sq 9..) (sq 0..)

p[1] (64 bits): 8 筋〜9 筋の 18 マスを格納 (Bit 18..63 は常に 0)
Bit:  63................18   17..9     8..0
     +--------------------+--------+--------+
     |      0 (46 bits)   |  9 筋  |  8 筋  |
     +--------------------+--------+--------+
                           (sq72..) (sq63..)
```

各要素の有効ビットマスクは以下の定数として定義されており、ビット反転やシフト演算の際には盤外ビットが常に `0` に保たれる。

```rust
const BOARD_MASK_LOW: u64 = 0x7FFF_FFFF_FFFF_FFFF;  // 下位 63 ビット (1〜7 筋)
const BOARD_MASK_HIGH: u64 = 0x0000_0000_0003_FFFF; // 下位 18 ビット (8〜9 筋)
```

任意のマス `sq`（`0..=80`）と `(part, bit)` の対応は以下のようになる。

- `sq < 63`（1〜7 筋）：`p[0]` のビット `sq`（`1u64 << sq`）
- `sq >= 63`（8〜9 筋）：`p[1]` のビット `sq - 63`（`1u64 << (sq - 63)`）

この 63+18 分割により、**すべての筋（1〜9 筋）が `p[0]` または `p[1]` のどちらか一方の内部で必ず連続した 9 ビットを占める** という強い幾何学的不変条件が成立する。

### `p[0]` と `p[1]` の境界の可視化

以下の盤面では、黄色ハイライトが `p[0]` の領域（`bit 0..=62`：1 筋〜7 筋の全 63 マス）を表す。
青い丸が `p[0]` の末尾マス（7 九 = `bit 62`）、赤い丸が `p[1]` の開始マス（8 一 = `bit 63`）であり、分割境界は **7 筋と 8 筋の間** に位置する。

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
  const loIndices = [];
  for (let f = 0; f < 7; f++) for (let r = 0; r < 9; r++) loIndices.push(f * 9 + r);
  board.highlightSquares(loIndices);
  board.setArrows([
    { from: '7i', to: '8a', color: 'rgba(220,50,50,0.8)', width: 2.0 },
  ]);
  board.setCircles([
    { square: '7i', color: 'rgba(0,120,215,0.8)', width: 2.0 },
    { square: '8a', color: 'rgba(220,50,50,0.8)', width: 2.0 },
  ]);
  board.goTo(0);
  window.layoutLohi = board;
}
</script>

---

## 2 種類の `u128` 表現：`packed_bits()` と `raw_bits()`

`Bitboard` は、アルゴリズムの性質に応じて `u128` との間で 2 通りの相互変換メソッドを使い分ける。

| メソッド対 | ビット配置の定義 | 主な用途 |
| :--- | :--- | :--- |
| `packed_bits()` / `from_u128(bits)` | `(p[0] & BOARD_MASK_LOW) \| ((p[1] & BOARD_MASK_HIGH) << 63)` | 81 マスがビット `0..=80` に隙間なく連続する表現。`build.rs` のテーブル定数、幾何テーブル（`BETWEEN` / `LINE`）、筋単位のシフト抽出（`>> (file * 9)`）で使用する。 |
| `raw_bits()` / `from_raw_bits(bits)` | `(p[0] as u128) \| ((p[1] as u128) << 64)` | `p[0]` が下位 64 ビット、`p[1]` がビット `64..=81` にそのまま並ぶ表現（Bit 63 は `0` の隙間）。128 ビット / 256 ビット SIMD レジスタ上のメモリ配置と完全に一致するため、飛車と角の SIMD レイ計算（`attack_tables.rs`）で使用する。 |

`packed_bits()` では `Square` のインデックス `0..=80` がそのまま `u128` のビット位置 `0..=80` に一致する。
一方、`raw_bits()` はメモリ上の `[u64; 2]` とビット位置が 1 対 1 に対応するため、シフト補正（`<< 63` や `>> 63`）を一切介さずに `__m128i` / `__m256i` レジスタへ直接ロードして Qugiy（減算と XOR によるブロッカー検出）およびビット反転（`reverse_bits`）を実行できる。

---

## 表示の向きと 180 度回転

通常の盤面表示（先手視点）では、上が一段、左が 9 筋、右が 1 筋となる。
以下の 2 枚の盤面は、9 一に後手香、1 九に先手香を置いた同一局面を先手視点と後手視点（180 度回転）で並べたものである。
表示上の向きを反転しても、内部の `Square` インデックス（1 九 = `8`、9 一 = `72`）および `Bitboard` のビット位置は変化しない。

<div style="display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:1rem;margin:1rem auto;max-width:960px;">
  <div>
    <p style="text-align:center;margin:0 0 0.35rem;">先手視点（通常表示）</p>
    <div id="layout-live-1" style="width:100%;height:460px;border:1px solid #ddd;"></div>
  </div>
  <div>
    <p style="text-align:center;margin:0 0 0.35rem;">後手視点（180 度回転表示）</p>
    <div id="layout-live-2" style="width:100%;height:460px;border:1px solid #ddd;"></div>
  </div>
</div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root1 = document.getElementById('layout-live-1');
  const board1 = new ShogiBoardAdapter();
  board1.mount(root1);
  board1.setOptions({ showHands: false });
  board1.setPositionFromSFEN('l8/9/9/9/9/9/9/9/8L b - 1');
  board1.goTo(0);
  window.layoutLive1 = board1;
  const root2 = document.getElementById('layout-live-2');
  const board2 = new ShogiBoardAdapter();
  board2.mount(root2);
  board2.setOptions({ showHands: false });
  board2.setPositionFromSFEN('l8/9/9/9/9/9/9/9/8L b - 1');
  board2.flip(true);
  board2.goTo(0);
  window.layoutLive2 = board2;
}
</script>
