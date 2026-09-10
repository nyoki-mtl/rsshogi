# Svg クラス

```python
rsshogi.svg.Svg
```

盤面を SVG 画像として出力するためのクラスです。
Jupyter Notebook では自動的に画像として表示されます。

```python
from rsshogi.svg import Svg
```

## 概要

`Svg` クラスは `Board.to_svg()` メソッドによって生成されます。
直接コンストラクタを呼び出すのではなく、`Board` から生成してください。
`Board.to_svg()` は Rust core の `Position::to_svg_with_options()` を利用するため、
Rust/Python で同じ SVG レイアウトとハイライト仕様になります。

## 基本的な使い方

```python
from rsshogi.core import Board

board = Board()
board.apply_usi("7g7f")

# SVG を生成
svg = board.to_svg()

# Jupyter Notebook では自動的に表示される
svg
```

### ファイルに保存

```python
from rsshogi.core import Board

board = Board()
svg = board.to_svg()

# SVG をファイルに保存
with open("position.svg", "w", encoding="utf-8") as f:
    f.write(str(svg))
```

## Board.to_svg() メソッド

```python
board.to_svg(last_move=None, scale=1.0, *, highlight_squares=True,
             bold_destination=False, board_background=None) -> Svg
```

現在の局面を SVG 画像として生成します。

### 引数

| 引数 | 型 | デフォルト | 説明 |
|------|-----|------------|------|
| `last_move` | `Move \| Move32 \| None` | `None` | 最後の指し手（ハイライト表示）。`Move` または `Move32` のみ受け付けます（USI 文字列・int は不可）。 |
| `scale` | `float` | `1.0` | 拡大率 |
| `highlight_squares` | `bool` | `True` | 移動先を `#f6b94d`、通常移動の移動元を `#fdf0e3` で着色する。 |
| `bold_destination` | `bool` | `False` | 移動先にある一枚の駒だけを `font-weight="bold"` で描画する。 |
| `board_background` | `str \| None` | `None` | 盤内の SVG `fill` 値（例：`"#efefef"`）。`None` は透明。文字列は XML 属性としてエスケープする。 |

追加の三引数はキーワード専用で、着色、太字、背景色を独立して指定できる。
背景色にはレンダラーが解釈できる SVG の色を渡す。
`last_move=None` では履歴を自動選択しない。
投了などの特殊手を渡した場合も升と駒を強調しない。
移動先が空なら太字化せず、駒打ちや成りでは着手後の駒を太字にする。
局面と最終着手の対応は呼び出し側で管理する。

`Move` は 16bit で駒情報を持たないため、現在の局面と照合して駒を復元します。
指した後の局面では移動元が空になっており、その `Move` はもう解決できません。
指した手をハイライトするときは、駒情報を持つ `board.last_move()`（`Move32`）を渡してください。

### 戻り値

- `Svg`: SVG 画像オブジェクト

### 使用例

```python
from rsshogi.core import Board

board = Board()
board.apply_usi("7g7f")

# 直前の指し手をハイライト
svg = board.to_svg(last_move=board.last_move())

# 2倍に拡大
svg = board.to_svg(scale=2.0)
```

### 太字と盤内背景を指定する

```python
from rsshogi.core import Board, Move32

board = Board()
board.apply_usi("7g7f")
board.apply_usi("3c3d")

sfen = board.to_sfen()
last_move_value = board.last_move().value

# Store the SFEN and complete Move32 value together, then restore them.
diagram = Board(sfen)
svg = diagram.to_svg(
    last_move=Move32(last_move_value),
    highlight_squares=False,
    bold_destination=True,
    board_background="#efefef",
)
```

SFEN 単体には最終着手が含まれないため、この例では完全な `Move32` の値を別途保存する。
図題や背景色の選択は呼び出し側で行う。
通常の駒は SVG の既定ウェイトで、太字は移動先の `<use>` にだけ指定する。
同種の他の駒、座標、持ち駒のウェイトは変わらない。

Rust では `svg` feature を有効にし、同じ三項目を
`rsshogi::board::position::SvgOptions` に渡す。
`position.to_svg_with_options(last_move, scale, &options)` が描画を行う。
`Position::to_svg(last_move, scale)` は既定のオプションで描画する。

### 配置と描画順

SVG 座標系の `viewBox` は `0 0 230 192`、盤内の矩形は
`x=20.5, y=10.5, width=180, height=180`、升の一辺は `20`。
外罫の矩形は `x=20, y=10, width=181, height=181` で、線幅は `1.5`。
背景、升の着色、罫線、座標、盤上の駒、持ち駒の順に重ねる。
背景は持ち駒欄や SVG 全体の余白を塗らない。

太字の有無や持ち駒の量によって、盤の原点、寸法、駒の配置中心は変わらない。
`scale` は SVG 全体の出力寸法に掛かり、`viewBox` は変わらない。
持ち駒が多い場合は持ち駒の文字だけを縮小する。
駒の書体は `serif` なので、実際の字形と太字の外観は表示環境のフォントに依存する。
玉将は先後とも「王」、成香、成桂、成銀は二文字、馬と龍は一文字で描画する。

## Svg クラスのメソッド

### `__str__()`

```python
str(svg) -> str
```

SVG 文字列を返します。

### `_repr_svg_()`

```python
svg._repr_svg_() -> str
```

Jupyter Notebook での表示用メソッドです。SVG 文字列を返します。

## 使用例

### Jupyter Notebook での表示

```python
from rsshogi.core import Board

board = Board()
board.apply_usi("7g7f")
board.apply_usi("3c3d")

# セルの最後に svg を置くと画像として表示される
board.to_svg()
```

### 複数の局面を並べて表示

```python
from rsshogi.core import Board
from IPython.display import display, HTML

board = Board()
moves = ["7g7f", "3c3d", "2g2f", "4c4d"]

svgs = []
for usi in moves:
    board.apply_usi(usi)
    svgs.append(str(board.to_svg(scale=0.5)))

# 横に並べて表示
html = "<div style='display: flex; gap: 10px;'>" + "".join(svgs) + "</div>"
display(HTML(html))
```

### Web ページへの埋め込み

```python
from rsshogi.core import Board

board = Board()
svg = board.to_svg()

# HTML に埋め込み
html = f"""
<!DOCTYPE html>
<html>
<body>
    <h1>現在の局面</h1>
    {svg}
</body>
</html>
"""

with open("position.html", "w", encoding="utf-8") as f:
    f.write(html)
```

## 関連項目

- [`Board`](board.md) - 盤面管理クラス
- [クイックスタート](../getting-started/quickstart.md)
