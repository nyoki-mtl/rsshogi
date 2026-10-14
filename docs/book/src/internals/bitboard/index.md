# ビットボードの設計と実装

ビットボード（`Bitboard`）は、将棋盤の 81 マスにおける駒の配置や利き（移動可能範囲）をビット列として表現するデータ構造である。
`rsshogi` では、81 マスを 16 バイト境界にアラインされた 2 個の 64 ビット整数 `Bitboard([u64; 2])` に格納し、局面全体のビットボード集合を管理する `BitboardSet` と組み合わせることで、高速な合法手生成と王手やピンの検出を実現している。

---

## 盤面の集合演算としてのビットボード

配列ベースの盤面表現（`board: [Piece; 81]`）は「あるマス `sq` にどの駒があるか」を O(1) で引ける一方、「先手の歩がどこにあるか」「空いているマスはどこか」を探すには 81 マスの線形走査を要する。
`Bitboard` は 81 マスの真偽値集合を単一の 128 ビットレジスタ（または 2 個の 64 ビット汎用レジスタ）に載せることで、盤面全体に対する集合演算を 1〜数クロックのビット論理命令へと還元する。

| 集合演算 | ビット演算 | 用途例 |
| :--- | :--- | :--- |
| 和集合（A ∪ B） | `a \| b` | 自駒と敵駒の統合、複数方向の利きの合成。 |
| 積集合（A ∩ B） | `a & b` | 駒の利き集合と敵駒の交差（駒を取る手の抽出）。 |
| 差集合（A \ B） | `a.andnot(b)` / `a & !b` | 駒の利きから味方駒がいるマスを除外。 |
| 対称差（A △ B） | `a ^ b` | 駒の移動に伴う占有ビットの差分更新。 |
| 補集合（Ā） | `!a` | 空きマスの抽出（`!occupied`）。 |

### 駒の配置と利きの可視化

まず、5 筋に 3 枚の駒（5 九の先手香、5 三の先手歩、5 一の後手香）を置いた盤面で、駒の向きと利きの基本を確認する。
丸はこの例で注目する駒を示し、青い矢印は先手の歩が 5 三から 5 二へ 1 マス利いていることを示す。

<div id="basics-live-1" style="width:600px;height:660px;margin:1rem auto;border:1px solid #ddd;"></div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('basics-live-1');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: false });
  board.setPositionFromSFEN('4l4/9/4P4/9/9/9/9/9/4L4 b - 1');
  board.goTo(0);
  board.setArrows([
    { from: '5c', to: '5b', color: 'rgba(0,120,215,0.7)', width: 1.2 },
  ]);
  board.setCircles([
    { square: '5i', color: 'rgba(255,140,0,0.7)', width: 1.5 },
    { square: '5c', color: 'rgba(0,120,215,0.7)', width: 1.5 },
    { square: '5a', color: 'rgba(220,50,50,0.7)', width: 1.5 },
  ]);
  window.basicsLive1 = board;
}
</script>

### 集合演算 1：AND によるフィルタリング

ビットボードの基本操作の 1 つは、複数の条件をビット論理積（`&`）で絞り込むことである。
たとえば「先手の歩」は、`by_piece[PAWN]`（先後すべての歩があるマス集合）と `by_color[BLACK]`（先手の全駒があるマス集合）の共通部分として 1 命令で抽出できる。
以下の 3 枚の将棋盤は、左から順に `by_piece[PAWN]`、`by_color[BLACK]`、および両者の AND 結果（先手の歩のみ）を黄色ハイライトで示している。

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

```rust
let us = position.side_to_move();
let them = us.flip();

let our_pieces = position.bitboards().color_pieces(us);
let their_pieces = position.bitboards().color_pieces(them);

// 先手の歩だけを抽出（by_piece[PAWN] & by_color[BLACK]）
let our_pawns = position.bitboards().pieces(PieceType::PAWN) & our_pieces;

// 味方の駒がないマスへの移動候補（attacks & !our_pieces）
let targets = attacks.andnot(our_pieces);

// 敵の駒を取る移動先（targets & their_pieces）
let capture_targets = targets & their_pieces;

for to_sq in capture_targets {
    // 1 が立っているマスだけを tzcnt / blsr で順番に取り出す
}
```

### 集合演算 2：OR による利きの合成と王手判定

複数駒種の利きをビット論理和（`|`）で合成し、敵玉や対象駒集合と `AND` をとることで、王手判定や特定マスへの攻撃駒列挙（`attackers_to`）をループなしで行える。
以下の盤面では、5 五の後手玉（青い丸）に対して、5 一の先手飛車の利き（赤い矢印）と 9 九の先手角の利き（オレンジの矢印）が同時に届いている様子を示す。

<div id="basics-live-check" style="width:600px;height:660px;margin:1rem auto;border:1px solid #ddd;"></div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('basics-live-check');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: false });
  board.setPositionFromSFEN('4R4/9/9/9/4k4/9/9/9/B8 b - 1');
  board.goTo(0);
  board.setArrows([
    { from: '5a', to: '5e', color: 'rgba(220,50,50,0.7)', width: 1.5 },
    { from: '9i', to: '5e', color: 'rgba(255,140,0,0.7)', width: 1.5 },
  ]);
  board.setCircles([
    { square: '5e', color: 'rgba(0,120,215,0.8)', width: 2.0 },
  ]);
  window.basicsLiveCheck = board;
}
</script>

```rust
let attackers =
    (rook_attacks(king_sq, occ) & enemy_rooks)
    | (bishop_attacks(king_sq, occ) & enemy_bishops);
let in_check = attackers.is_not_empty();
```

---

## `BitboardSet` と 6 種類の複合ビットボードキャッシュ

`Position` の内部で全ビットボードを保持する `BitboardSet`（`crates/rsshogi/src/board/bitboard_set.rs`）は、基本となる駒種別および陣営別のビットボードに加えて、探索ホットパスで頻繁に合成される **6 種類の複合キャッシュ（Derived Composite Caches）** を常駐させている。

### `BitboardSet` のフィールド構成

| フィールド名 | 型 | 保持する集合の内容 |
| :--- | :--- | :--- |
| `by_piece` | `[Bitboard; PieceType::COUNT]` | 各 `PieceType`（`1..=14`）の駒が存在するマス（先後合算）。 |
| `by_color` | `[Bitboard; Color::COUNT]` | 先手（`BLACK = 0`）および後手（`WHITE = 1`）の全駒が存在するマス。 |
| `occupied` | `Bitboard` | 先後合わせた全占有マス（`by_color[0] \| by_color[1]`）。 |
| `golds_cache` | `Bitboard` | 金と同じ動きをする全 5 駒種（`GOLD \| PRO_PAWN \| PRO_LANCE \| PRO_KNIGHT \| PRO_SILVER`）。 |
| `hdk_cache` | `Bitboard` | 玉および成大駒の合成（`HORSE \| DRAGON \| KING`）。 |
| `bishop_horse_cache` | `Bitboard` | 斜め方向の飛び駒の合成（`BISHOP \| HORSE`）。 |
| `rook_dragon_cache` | `Bitboard` | 縦横方向の飛び駒の合成（`ROOK \| DRAGON`）。 |
| `silver_hdk_cache` | `Bitboard` | 斜め前方に 1 マス進める駒の合成（`SILVER \| HORSE \| DRAGON \| KING`）。 |
| `golds_hdk_cache` | `Bitboard` | 前方および横方向に 1 マス進める駒の合成（`golds_cache \| HORSE \| DRAGON \| KING`）。 |

### なぜ 6 種類の複合キャッシュを持つのか

特定のマス `sq` に利いている駒をすべて列挙する `attackers_to(sq, occupied)` や、王手可能マスを計算する `check_squares`、および指し手生成では、「角または馬」「飛または龍」「金および成金 4 種」「銀と馬と龍と玉」といった複数駒種の和集合が毎ノード何度も参照される。
もし基本配列 `by_piece` しか持たなければ、`attackers_to` を 1 回呼ぶたびに 4 個の成金ビットボードや馬、龍、玉のビットボードを毎回 OR 演算で合成しなければならない。

`BitboardSet` は、盤面上の駒が追加、削除、移動されるたびに `refresh_derived()` で上記 6 個のキャッシュを更新する。
特に `silver_hdk_cache`（銀の利きテーブルと交差させることで斜め 1 マス利きを一括抽出できる集合）と `golds_hdk_cache`（金の利きテーブルと交差させることで縦横や斜め前 1 マス利きを一括抽出できる集合）をあらかじめ保持しておくことで、`attackers_to` における近接駒の利き判定は最小限のビット論理積だけで完了する。

---

## 本セクションの構成

- **[メモリレイアウト（63+18 分割）](layout.md)**：`#[repr(C, align(16))] Bitboard([u64; 2])` の 63+18 ビット分割構造と、`packed_bits()` および `raw_bits()` の 2 種類の `u128` 表現の使い分けを将棋盤上で解説する。
- **[基本演算、幾何テーブル、SIMD 命令](operations.md)**：SSE2 / SSE4.1 / BMI1 を用いたビット論理演算とビット走査、および `board/lookup.rs` の幾何テーブル（`BETWEEN`、`LINE`、`is_aligned`）を解説する。
- **[利き計算とテーブル生成](attacks.md)**：`build.rs` によるコンパイル時テーブル生成、9 ビット占有状態による縦方向ルックアップ（`FILE_ATTACKS`）、および AVX2 双方向レイ計算を用いた飛車と角の利きアルゴリズムを解説する。
