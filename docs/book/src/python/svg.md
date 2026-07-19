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
`Board.to_svg()` は Rust core の `Position::to_svg()` を利用するため、
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
board.to_svg(last_move=None, scale=1.0) -> Svg
```

現在の局面を SVG 画像として生成します。

### 引数

| 引数 | 型 | デフォルト | 説明 |
|------|-----|------------|------|
| `last_move` | `Move \| Move32 \| None` | `None` | 最後の指し手（ハイライト表示）。`Move` または `Move32` のみ受け付けます（USI 文字列・int は不可）。 |
| `scale` | `float` | `1.0` | 拡大率 |

### 戻り値

- `Svg`: SVG 画像オブジェクト

### 使用例

```python
from rsshogi.core import Board, Move

board = Board()
move = Move.from_usi("7g7f")
board.apply_move(move)

# 最後の指し手をハイライト
svg = board.to_svg(last_move=move)

# 2倍に拡大
svg = board.to_svg(scale=2.0)
```

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
