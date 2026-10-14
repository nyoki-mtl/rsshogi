# ビットボードの設計と実装

ビットボード（`Bitboard`）は、将棋盤の 81 マスにおける駒の配置や利き（移動可能範囲）をビット列として表現するデータ構造である。
`rsshogi` では、81 マスを 16 バイト境界にアラインされた 2 個の 64 ビット整数 `Bitboard([u64; 2])` に格納し、局面全体のビットボード集合を管理する `BitboardSet` と組み合わせることで、高速な合法手生成と王手やピンの検出を実現している。

---

## 盤面の集合演算としてのビットボード

配列ベースの盤面表現（`board: [Piece; 81]`）は「あるマス `sq` にどの駒があるか」を O(1) で引ける一方、「先手の歩がどこにあるか」「空いているマスはどこか」を探すには 81 マスの線形走査を要する。
`Bitboard` は 81 マスの真偽値集合を単一の 128 ビットレジスタ（または 2 個の 64 ビット汎用レジスタ）に載せることで、盤面全体に対する集合演算を 1〜数クロックのビット論理命令へと還元する。

たとえば、ある駒の利き集合 `attacks` から味方駒のいるマスを除外して移動先候補を求め、さらに敵駒との共通部分から「駒を取る手」だけを抽出する処理は、以下のように分岐なしで記述できる。

```rust
let us = position.side_to_move();
let them = us.flip();

let our_pieces = position.bitboards().color_pieces(us);
let their_pieces = position.bitboards().color_pieces(them);

// 味方の駒がないマスへの移動候補（attacks & !our_pieces）
let targets = attacks.andnot(our_pieces);

// 敵の駒を取る移動先（targets & their_pieces）
let capture_targets = targets & their_pieces;

for to_sq in capture_targets {
    // 1 が立っているマスだけを tzcnt / blsr で順番に取り出す
}
```

---

## `BitboardSet` と 6 種類の複合ビットボードキャッシュ

`Position` の内部で全ビットボードを保持する `BitboardSet`（`crates/rsshogi/src/board/bitboard_set.rs`）は、基本となる駒種別および陣営別のビットボードに加えて、探索ホットパスで頻繁に合成される **6 種類の複合キャッシュ（Derived Composite Caches）** を常駐させている。

### `BitboardSet` のフィールド構成

| フィールド名 | 型 | 保持する集合の内容 |
| :--- | :--- | :--- |
| `by_piece` | `[Bitboard; PieceType::COUNT]` | 各 `PieceType`（`1..=14`）の駒が存在するマス（先後合算）。 |
| `by_color` | `[Bitboard; Color::COUNT]` | 先手（`BLACK = 0`）および後手（`WHITE = 1`）の全駒が存在するマス。 |
| `occupied` | `Bitboard` | 先後合わせた全占有マス（`by_color[0] | by_color[1]`）。 |
| `golds_cache` | `Bitboard` | 金と同じ動きをする全 5 駒種（`GOLD | PRO_PAWN | PRO_LANCE | PRO_KNIGHT | PRO_SILVER`）。 |
| `hdk_cache` | `Bitboard` | 玉および成大駒の合成（`HORSE | DRAGON | KING`）。 |
| `bishop_horse_cache` | `Bitboard` | 斜め方向の飛び駒の合成（`BISHOP | HORSE`）。 |
| `rook_dragon_cache` | `Bitboard` | 縦横方向の飛び駒の合成（`ROOK | DRAGON`）。 |
| `silver_hdk_cache` | `Bitboard` | 斜め前方に 1 マス進める駒の合成（`SILVER | HORSE | DRAGON | KING`）。 |
| `golds_hdk_cache` | `Bitboard` | 前方および横方向に 1 マス進める駒の合成（`golds_cache | HORSE | DRAGON | KING`）。 |

### なぜ 6 種類の複合キャッシュを持つのか

特定のマス `sq` に利いている駒をすべて列挙する `attackers_to(sq, occupied)` や、王手可能マスを計算する `check_squares`、および指し手生成では、「角または馬」「飛または龍」「金および成金 4 種」「銀と馬と龍と玉」といった複数駒種の和集合が毎ノード何度も参照される。
もし基本配列 `by_piece` しか持たなければ、`attackers_to` を 1 回呼ぶたびに 4 個の成金ビットボードや馬、龍、玉のビットボードを毎回 OR 演算で合成しなければならない。

`BitboardSet` は、盤面上の駒が追加、削除、移動されるたびに `refresh_derived()` で上記 6 個のキャッシュを更新する。
特に `silver_hdk_cache`（銀の利きテーブルと交差させることで斜め 1 マス利きを一括抽出できる集合）と `golds_hdk_cache`（金の利きテーブルと交差させることで縦横や斜め前 1 マス利きを一括抽出できる集合）をあらかじめ保持しておくことで、`attackers_to` における近接駒の利き判定は最小限のビット論理積だけで完了する。

---

## 本セクションの構成

- **[メモリレイアウト（63+18 分割）](layout.md)**：`#[repr(C, align(16))] Bitboard([u64; 2])` の 63+18 ビット分割構造と、`packed_bits()` および `raw_bits()` の 2 種類の `u128` 表現の使い分けを解説する。
- **[基本演算、幾何テーブル、SIMD 命令](operations.md)**：SSE2 / SSE4.1 / BMI1 を用いたビット論理演算とビット走査、および `board/lookup.rs` の幾何テーブル（`BETWEEN`、`LINE`、`is_aligned`）を解説する。
- **[利き計算とテーブル生成](attacks.md)**：`build.rs` によるコンパイル時テーブル生成、9 ビット占有状態による縦方向ルックアップ（`FILE_ATTACKS`）、および AVX2 双方向レイ計算を用いた飛車と角の利きアルゴリズムを解説する。
