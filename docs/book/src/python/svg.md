# SVG 描画（`rsshogi.svg`）

`rsshogi.svg` モジュールの `Svg` クラスと `Board.to_svg()` メソッドは、局面を SVG 画像として描画する機能を提供する。
Rust コアの `Position::to_svg_with_options()` と共通の描画エンジンを使用するため、Rust と Python で同一の SVG 出力が得られる。

```python
from rsshogi.core import Board
from rsshogi.svg import Svg
```

## `Board.to_svg()`

```python
board.to_svg(
    last_move: Move | Move32 | None = None,
    scale: float = 1.0,
    *,
    highlight_squares: bool = True,
    bold_destination: bool = False,
    board_background: str | None = None,
) -> Svg
```

現在局面の盤面と持ち駒を SVG 画像として描画し、`Svg` オブジェクトを返す。

### 引数

| 引数 | 型 | デフォルト | 説明 |
|---|---|---|---|
| `last_move` | `Move \| Move32 \| None` | `None` | 強調表示する直前の指し手を指定する。`Move` または `Move32` のみを受け付け、文字列や整数を渡すと `TypeError` を送出する。 |
| `scale` | `float` | `1.0` | SVG 全体の出力サイズ（`width` と `height`）に掛ける拡大率を指定する。 |
| `highlight_squares` | `bool` | `True` | キーワード専用引数である。`last_move` 指定時に移動先を `#f6b94d`、盤上移動の移動元を `#fdf0e3` で塗りつぶす。 |
| `bold_destination` | `bool` | `False` | キーワード専用引数である。`last_move` 指定時に移動先マスにある駒 1 枚だけを `font-weight="bold"` で描画する。 |
| `board_background` | `str \| None` | `None` | キーワード専用引数である。盤内矩形の背景色（例：`"#efefef"`）を指定する。`None` の場合は透明になる。 |

### `last_move` に関する注意点

- `last_move=None` の場合は、`Board` に指し手履歴が残っていても自動的にはハイライトしない。直前の手を強調したい場合は明示的に `last_move=board.last_move()` を渡す。
- `Move.MOVE_RESIGN` などの特殊手定数を渡した場合は、マス目の着色も太字化も行わない。
- 16 ビット型の `Move` は駒情報を持たないため、現在の盤面と照合して `Move32` への変換を試みる。しかし指し手を適用した後の盤面では移動元マスがすでに空になっており、盤上移動の `Move` を解決できずに `ValueError` となる。着手後の盤面で直前の手を強調する場合は、駒情報を持つ `Move32`（`board.last_move()` や `board.push_move()` の戻り値）を渡す。

### レイアウト仕様

SVG の `viewBox` は `0 0 230 192` で固定されており、`scale` を変更しても内部座標系は変わらない。
盤内の矩形は `x=20.5, y=10.5, width=180, height=180`（1 マス `20x20`）、外枠は `x=20, y=10, width=181, height=181`（線幅 `1.5`）である。
描画要素は盤内背景、マス目のハイライト、罫線、筋と段の座標文字、盤上の駒、持ち駒の順に重ねられる。
玉は先後ともに「王」、成香と成桂と成銀は 2 文字縦並び、馬と龍は 1 文字で描画される。

## `Svg`

`Svg` は生成された SVG XML 文字列を保持するラッパクラスである。
直接コンストラクタは提供せず、`Board.to_svg()` から取得する。

| メソッド | シグネチャ | 説明 |
|---|---|---|
| `__str__` | `str(svg) -> str` | SVG XML 文字列を返す。 |
| `__repr__` | `repr(svg) -> str` | SVG XML 文字列を返す。 |
| `_repr_svg_` | `svg._repr_svg_() -> str` | IPython / Jupyter Notebook がインライン画像表示に利用する SVG 文字列を返す。 |

## 使用例

### 直前の指し手をハイライトしてファイルに保存する

```python
from rsshogi.core import Board

board = Board()
board.apply_usi("7g7f")
board.apply_usi("3c3d")

# 直前の指し手（Move32）を渡して SVG を生成
svg = board.to_svg(last_move=board.last_move(), scale=1.5)

with open("position.svg", "w", encoding="utf-8") as f:
    f.write(str(svg))
```

### SFEN と Move32 の整数値を保存して後から図面を復元する

SFEN 文字列単体には直前の指し手が含まれないため、図面を後から再描画する場合は SFEN と `Move32.value` を組にして保存する。

```python
from rsshogi.core import Board, Move32

board = Board()
board.apply_usi("7g7f")
board.apply_usi("3c3d")

sfen = board.to_sfen()
last_move_in_u32 = board.last_move().value

# 保存した SFEN と Move32 の値から局面と最終手表示を復元する
diagram = Board(sfen)
svg = diagram.to_svg(
    last_move=Move32(last_move_in_u32),
    highlight_squares=False,
    bold_destination=True,
    board_background="#efefef",
)
```

### Jupyter Notebook で複数の局面を横並びに表示する

```python
from IPython.display import HTML, display
from rsshogi.core import Board

board = Board()
svgs = []
for usi in ["7g7f", "3c3d", "2g2f", "4c4d"]:
    mv32 = board.push_usi(usi)
    svgs.append(str(board.to_svg(last_move=mv32, scale=0.6)))

html = "<div style='display: flex; gap: 12px;'>" + "".join(svgs) + "</div>"
display(HTML(html))
```

## 関連項目

- [`Board`](board.md)
- [指し手クラス（`Move` / `Move32`）](move.md)
