# 座標系の定義

> **前提知識**: [基本型の概要](./index.md)

## このページの要点

- 81 マスを 0〜80 の整数（`Square`）で表現し、`file * 9 + rank` で計算する
- **縦型レイアウト**: 同じ筋のマスが連続するインデックスになるよう配置する。これは香車や歩の利き計算を劇的に高速化する
- 方角定数（`SQ_U`, `SQ_D` など）により、駒の移動方向をテーブルなしで表現できる
- USI 表記 `7f` と棋譜表記「７六」の対応を理解することで、デバッグ時に値を素早く読めるようになる

## なぜ座標設計が重要か

座標系はライブラリ全体の基盤です。ここでの設計判断が、後続の Bitboard レイアウト、利き計算、指し手生成のすべてに波及します。

たとえば、**横型**（段優先）のレイアウトを選んだ場合、「先手の歩を全部1マス前進させる」操作は 9 箇所のビットを個別に動かす必要があります。
一方、**縦型**（筋優先）のレイアウトなら、歩の前進は Bitboard 全体の `>> 1` だけで完了します。
このような差が探索全体で何百万回も蓄積するため、座標系の設計は性能に直結します。

## Square（マス）の定義

将棋の盤面は 9×9 = 81 マスです。rsshogi は各マスに 0〜80 のインデックスを割り当てます。[^yaneuraou-square]

### 座標系の規約

```text
筋（File）: 1〜9（右から左）
段（Rank）: 1〜9（上から下）
```

将棋の棋譜表記では「７六歩」のように **筋→段** の順で記述しますが、内部インデックスは以下の式で計算します：

```rust,ignore
square_index = file * 9 + rank
```

ここで `file` と `rank` は 0-indexed（0〜8）です。
`file` は１筋を 0、９筋を 8 とする右から左へのインデックスです。
`rank` は一段を 0、九段を 8 とする上から下へのインデックスです。
インデックスは１筋を上から下へ走査したのちに次の筋へ移るため、図のように１一が 0、９九が 80 になります。

### 実装: Square 構造体

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/types/square.rs:square_struct}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/types/square.rs#L48-L83)</small>

### 視覚的なマッピング

以下の図は、将棋盤の各マスがどのビットインデックスに対応するかを示しています：

<div id="coords-map" style="width:600px;height:660px;margin:1rem auto;border:1px solid #ddd;"></div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('coords-map');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: false });
  board.setPositionFromSFEN('9/9/9/9/9/9/9/9/9 b - 1');
  // 1一(0), 5五(40), 9九(80) をハイライト
  board.highlightSquares([0, 40, 80]);
  board.goTo(0);
  window.coordsMap = board;
  // 上の数字が筋（9..1）、右側の漢数字が段（一..九）です。
}
</script>

<small>**図**: 黄色でハイライトされたマス（1一=0, 5五=40, 9九=80）は、座標計算の基準となる重要な位置です。</small>

### 具体例

| 棋譜表記 | USI表記 | file | rank | インデックス | 定数名 |
|---------|---------|------|------|-------------|--------|
| １一 | `1a` | 0 | 0 | 0 | `SQ_11` |
| ５五 | `5e` | 4 | 4 | 40 | `SQ_55` |
| ９九 | `9i` | 8 | 8 | 80 | `SQ_99` |
| ７七 | `7g` | 6 | 6 | 60 | `SQ_77` |

### 実装: 筋と段からマスを生成

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/types/square.rs:square_from_file_rank}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/types/square.rs#L278-L289)</small>

### 方角定数

駒の移動方向を表す定数も定義されています：

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/types/square.rs:square_direction_constants}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/types/square.rs#L85-L95)</small>

これにより、例えば「金の移動」は以下のように簡潔に表現できます：

```rust,ignore
// 先手の金の移動方向（後退以外の6方向）
const GOLD_BLACK_DIRS: [i8; 6] = [SQ_U, SQ_D, SQ_L, SQ_R, SQ_RU, SQ_LU];
```

### チェスとの比較

チェスでは Little-Endian Rank-File (LERF) マッピングが標準的です：[^cpw-board-rep]

```text
a1 = bit 0,  h1 = bit 7
a2 = bit 8,  h2 = bit 15
...
a8 = bit 56, h8 = bit 63
```

将棋の縦型レイアウトと異なり、チェスは **横型**（rank 優先）です。

| 項目 | チェス（横型 LERF） | 将棋（縦型 File-Major） |
|------|-------------------|----------------------|
| 走査順 | 段→筋（a1, b1, …, h1, a2, …） | 筋→段（1一, 1二, …, 1九, 2一, …） |
| 遠方駒の得意方向 | ルーク横、ビショップ斜め | 香車が**縦方向**に一直線 |
| ビットシフトで表せる操作 | `<< 8` で1段前進 | `>> 1` で1段前進 |
| マスク生成の簡潔さ | rank マスクが連続ビット | **file マスクが連続ビット** |
| 盤サイズ | 64 マス（1 × u64） | 81 マス（128bit 必要） |

チェスでは横方向のルーク移動が多いため横型が自然ですが、将棋では香車の縦方向移動と歩の前進が頻繁なため、筋方向を連続させる縦型レイアウトが有利です。

## 落とし穴

### file/rank の 0-indexed と 1-indexed の混同

棋譜表記やUSI表記は **1-indexed**（「７六歩」の7と6）ですが、内部インデックスは **0-indexed** です。
変換を忘れると、1マスずれたバグが発生します。

`Square::from_file_rank` は生の整数ではなく `File` / `Rank`（どちらも 0-indexed の newtype）を取ります。
`File::FILE_7` のような定数は「7筋」を表しつつ内部値は 0-indexed（`FILE_7 = 6`）なので、定数を使えばズレは生じません。
注意が必要なのは、棋譜の 1-indexed の数字から生インデックスを**自分で計算する**ときです。

```rust,ignore
// 棋譜「7六」(= 7筋6段) を表すマス。定数なら 0/1-indexed のズレを気にしなくてよい
let sq = Square::from_file_rank(File::FILE_7, Rank::RANK_6); // = Square(59)

// ❌ 間違い: 棋譜の数字をそのまま 0-indexed として計算してしまう
let index = 7 * 9 + 6;        // 意図: 7筋6段 → 実際は 8筋7段相当でズレる

// ✅ 正しい: 1-indexed から 0-indexed に変換してから計算
let index = (7 - 1) * 9 + (6 - 1); // 7筋6段 = file=6, rank=5 → 59
```

### USI 表記 `7f` と棋譜表記「７六」の段表記の違い

USI 表記では段を `a`〜`i` のアルファベットで表します。`a` = 一段 = rank 0 です。
棋譜表記の「六」= rank 5 = USI `f` です（`g` ではありません）。

```text
棋譜「７六」 = USI "7f" = file=6, rank=5 = Square(59)
```

## まとめ

- `Square` は `file * 9 + rank`（0-indexed）で 81 マスを一次元化する
- 筋方向を連続させる**縦型レイアウト**は、香車・歩の利き計算で横型より有利
- 方角定数を使えば、テーブル参照なしに隣接マスを `+1`, `-1`, `+9` 等で求められる
- 外部表記（棋譜・USI）との変換では 0/1-indexed の違いに注意

## 次に読む

→ **[駒の定義](./pieces.md)**: 駒種と先後をどのように数値化するか、ビットフラグによる成り判定の仕組みを解説します。

---

[^yaneuraou-square]: YaneuraOu `types.h:114-155`（[Commit eb2856f](https://github.com/yaneurao/YaneuraOu/blob/eb2856f91e6088e5b8c1216e3d84d0563f7cd85f/source/types.h#L114-L155)）
[^cpw-board-rep]: ChessProgramming Wiki, ["Board Representation"](https://www.chessprogramming.org/Board_Representation)
