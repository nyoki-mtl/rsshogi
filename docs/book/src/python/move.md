# 指し手クラス（Move / Move32 / AperyMove / AperyMove32）

`rsshogi.core` は用途と互換性に応じて 4 種類の指し手クラスと変換関数 `to_move()` を提供する。
通常の盤面操作や棋譜管理では 16 ビットの `Move` または 32 ビットの `Move32` を使い、`cshogi` や Apery 系の学習データ（HCPE など）と連携する境界では `AperyMove` と `AperyMove32` を使う。

```python
from rsshogi.core import AperyMove, AperyMove32, Board, Move, Move32, to_move
```

## 4 つの指し手型の比較

`rsshogi` では 16 ビット指し手を基本型として `Move` と呼び、32 ビット指し手を `Move32` と呼ぶ。
やねうら王と `cshogi`（Apery 系）では 16 ビット型を `Move16`、32 ビット型を `Move` と呼ぶため、名称とビットレイアウトの対応に注意が必要である。

| 型 | ビット幅 | ビットレイアウト互換 | 保持する駒情報 | 主な用途 |
|---|---|---|---|---|
| `Move` | 16 ビット | やねうら王 `Move16` 互換 | なし（駒打ちの打つ駒種のみ） | `Board.legal_moves()`、`Record`、定跡、探索木の保存など一般的な用途全般に使う。 |
| `Move32` | 32 ビット | `rsshogi` 拡張（下位 16 ビットは `Move` と同一） | 移動後の色付き駒（`Piece`） | `Board.legal_moves_move32()`、`Board.last_move()`、CSA 文字列出力（`to_csa()`）に使う。 |
| `AperyMove` | 16 ビット | `cshogi` / Apery `Move16` 互換 | なし（駒打ちの打つ駒種のみ） | HCPE の `bestMove16` や `cshogi.move16()` との相互変換に使う。 |
| `AperyMove32` | 32 ビット | `cshogi` / Apery `Move` 互換 | 移動前の駒種（`PieceType`）と取得駒種（`PieceType`） | `cshogi` の 32 ビット指し手との相互変換や取得駒種の参照に使う。 |

`Move` と `AperyMove` は同じ 16 ビット幅だが、成りフラグの位置と駒打ちの表現方法が異なるため、整数値を直接比較してはならない。
相互変換には必ず `Move.to_apery()` と `AperyMove.to_move()` を使用する。

### ビット構造の違い

`Move` と `Move32` はやねうら王の `Move16` レイアウトを基礎とし、`Move32` は上位ビットに移動後の `Piece`（手番と成り状態を含む）を格納する。

```text
Move (16 ビット):
  bit 15    : 成りフラグ (1 = 成り)
  bit 14    : 駒打ちフラグ (1 = 駒打ち)
  bit 7..13 : 移動元マス (0..80) または打つ駒種
  bit 0..6  : 移動先マス (0..80)

Move32 (32 ビット):
  bit 21..31: 未使用 (0)
  bit 16..20: 移動後の色付き駒 (Piece)
  bit 0..15 : Move と同一
```

一方、`AperyMove` と `AperyMove32` は Apery および `cshogi` のビットレイアウトに従う。

```text
AperyMove (16 ビット):
  bit 15    : 未使用 (0)
  bit 14    : 成りフラグ (1 = 成り)
  bit 7..13 : 移動元マス (0..80) または駒打ちコード (81 + HandPiece)
  bit 0..6  : 移動先マス (0..80)

AperyMove32 (32 ビット):
  bit 20..27: 取得した駒種 (PieceType)
  bit 16..19: 移動前の駒種 (PieceType、駒打ち時は 0)
  bit 0..15 : AperyMove と同一
```

## 相互変換マトリクス

4 つの指し手型と USI 文字列の間の変換方法を下表に示す。
32 ビット型への昇格や取得駒情報の補完には現在局面の `board`（`Board` インスタンス）が必要になる。

| 変換元 \ 変換先 | `Move` | `Move32` | `AperyMove` | `AperyMove32` | USI 文字列 (`str`) |
|---|---|---|---|---|---|
| **`Move` (`mv`)** | - | `board.move32_from_move(mv)` | `mv.to_apery()` | `board.apery_move32_from_move(mv)` | `mv.to_usi()` |
| **`Move32` (`mv32`)** | `mv32.to_move()` | - | `mv32.to_move().to_apery()` | `mv32.to_apery(board)` または `board.apery_move32_from_move32(mv32)` | `mv32.to_usi()` |
| **`AperyMove` (`amv`)** | `amv.to_move()` | `board.move32_from_move(amv.to_move())` | - | `board.apery_move32_from_move(amv.to_move())` | `amv.to_usi()` |
| **`AperyMove32` (`amv32`)** | `amv32.to_move()` | `amv32.to_move32(board)` | `amv32.to_apery_move()` | - | `amv32.to_usi()` |
| **USI 文字列 (`usi`)** | `Move.from_usi(usi)` | `Move32.from_usi(usi)`（駒情報なし） | `AperyMove.from_usi(usi)` | `board.apery_move32_from_move(Move.from_usi(usi))` | - |

```python
from rsshogi.core import Board, Move

board = Board()
mv = Move.from_usi("7g7f")

# Move から各型への変換
mv32 = board.move32_from_move(mv)
amv = mv.to_apery()
amv32 = board.apery_move32_from_move(mv)

# 逆方向の変換
assert mv32.to_move() == mv
assert amv.to_move() == mv
assert amv32.to_move() == mv
assert amv32.to_move32(board) == mv32
assert amv32.to_apery_move() == amv
```

## 4 クラス共通の API

`Move`、`Move32`、`AperyMove`、`AperyMove32` は、座標や手種別を参照する共通のプロパティとメソッドを備えている。

### 共通コンストラクタと USI 変換

| メソッド / 操作 | 対象クラス | 戻り値 | 説明 |
|---|---|---|---|
| `Cls(value: int)` | 全 4 クラス | 各インスタンス | 内部表現の符号なし整数（16 ビット型は `0..=65535`、32 ビット型は `0..=4294967295`）から生成する。範囲外は `OverflowError` を送出する。 |
| `Cls.from_usi(usi: str)` | `Move`, `Move32`, `AperyMove` | 各インスタンス | USI 文字列から生成する。不正な文字列では `ValueError` を送出する。 |
| `mv.to_usi()` | 全 4 クラス | `str` | USI 形式の文字列（`"7g7f"`、`"P*5e"`、`"2b2a+"` など）を返す。 |

`Move.from_usi(usi)` と `Move32.from_usi(usi)` は通常の指し手に加えてパス手を表す `"0000"` を受け付け、`MOVE_NULL` を返す。
`"resign"`、`"win"`、`"none"` は `Move.from_usi()` では `ValueError` となるため、特殊手トークンも含めて変換したい場合は [`rsshogi.usi.move_from_usi()`](usi.md#定数と指し手変換rsshogiusi) を使用する。
また `AperyMove.from_usi(usi)` は通常の指し手のみを受け付け、特殊手文字列に対しては `ValueError` を送出する。

### 共通プロパティと判定メソッド

| プロパティ / メソッド | 型 / 戻り値 | 説明 |
|---|---|---|
| `from_sq` | `Square` | 移動元のマスを返す。駒打ちや特殊手の場合は移動先や内部コードに依存するため、通常移動時（`not is_drop()` かつ `is_normal()`）に参照する。 |
| `to_sq` | `Square` | 移動先のマスを返す。 |
| `dropped_piece_type` | `PieceType \| None` | 駒打ちの場合に打つ駒種を返し、駒打ちでない場合は `None` を返す。 |
| `move_type` | `MoveType` | `MoveType.NORMAL`、`MoveType.PROMOTION`、`MoveType.DROP` のいずれかを返す。 |
| `value` | `int` | 内部エンコーディングの整数値を返す。 |
| `is_drop()` | `bool` | 駒打ちであれば `True` を返す。 |
| `is_promotion()` | `bool` | 成る手であれば `True` を返す。 |
| `is_normal()` | `bool` | 特殊手定数（`MOVE_NONE` や `MOVE_NULL` など）ではない通常の指し手であれば `True` を返す。 |

### 特殊手クラス定数（`Move` と `Move32`）

`Move` と `Move32` は特殊手を表す 5 つのクラス定数を持つ。
いずれも `is_normal()` は `False` を返す。

| 定数 | `to_usi()` の値 | 説明 |
|---|---|---|
| `MOVE_NONE` | `"none"` | 無効手または未指定を表す定数である。 |
| `MOVE_NULL` | `"null"` | パス手（null move）を表す定数である。 |
| `MOVE_RESIGN` | `"resign"` | 投了を表す定数である。 |
| `MOVE_WIN` | `"win"` | 入玉宣言勝ちを表す定数である。 |
| `MOVE_END` | `"end"` | 指し手終端を表す番兵定数である。 |

### 対称変換（`Move` と `Move32`）

`Move` と `Move32` は、学習データの対称性拡張向けに盤面の反転に対応した指し手を返すメソッドを持つ。
いずれも自身を書き換えず新しいインスタンスを返し、特殊手定数（`is_normal()` が `False` の値）はそのまま返す。

| メソッド | シグネチャ | 説明 |
|---|---|---|
| `flip_side` | `mv.flip_side() -> Move \| Move32` | 盤面を 180 度回転（先後反転）した指し手を返す。駒打ちは駒種を保ち打ち先だけを反転する。`Move32` では駒情報（`piece_after_move`）の手番も反転する。 |
| `mirror_file` | `mv.mirror_file() -> Move \| Move32` | 筋を左右反転した指し手を返す。段は変わらないため成れる範囲も手番も変わらず、`Move32` の駒情報はそのまま保つ。 |

どちらも 2 回適用すると元の指し手に戻る。
盤面側の反転は [`Board.to_sfen_flipped()`](board.md#局面の初期化と入出力) と [`Board.to_packed_sfen_flipped()`](board.md#バイナリシリアライズと-apery-互換変換) を使用する。

```python
from rsshogi.core import Board, Move

board = Board()
mv = Move.from_usi("7g7f")

assert mv.flip_side().to_usi() == "3c3d"
assert mv.mirror_file().to_usi() == "3g3f"
assert mv.flip_side().flip_side() == mv

mv32 = board.move32_from_move(mv)
assert mv32.flip_side().piece_after_move.color == mv32.piece_after_move.color.flip()
assert mv32.mirror_file().piece_after_move == mv32.piece_after_move
```

### 共通の特殊メソッド

- `int(mv)`：`mv.value` と同じ内部整数値を返す。
- `str(mv)`：`mv.to_usi()` と同じ USI 文字列を返す。
- `repr(mv)`：`Move(7g7f)` のようなデバッグ用文字列を返す。
- `hash(mv)`：内部整数値に基づくハッシュ値を返し、`dict` のキーや `set` の要素として使用できる。
- `mv == other` / `mv != other`：同じクラスのインスタンス、またはビット幅に収まる非負 `int` と等価比較できる。異なる指し手クラス同士の比較や `<` などの順序比較は `TypeError` となる。

## Move

```python
rsshogi.core.Move(value: int)
```

`rsshogi` 標準の 16 ビット指し手クラスである。
`Board.legal_moves()` の戻り値や `MoveEntry.move`、`BookMove.mv` など、ライブラリ全体で標準の指し手表現として使われる。

### Move 固有のメソッド

| メソッド | シグネチャ | 説明 |
|---|---|---|
| `to_ki2` | `mv.to_ki2(board: Board) -> str \| None` | 現在局面 `board` を参照して KI2 形式の文字列（例：`"▲７六歩"`）に変換する。 |
| `to_apery` | `mv.to_apery() -> AperyMove` | `cshogi` / Apery 互換の 16 ビット指し手 `AperyMove` に変換する。 |

`Move.to_ki2(board)` は内部で `board.move32_from_move(mv)` と同様に現在局面から移動駒を特定する。
`mv` が特殊手である場合や、現在局面の移動元に自分の駒がないなど正常な指し手へ解決できない場合は `ValueError` を送出する。

```python
from rsshogi.core import Board, Move

board = Board()
mv = Move.from_usi("7g7f")
print(mv.to_ki2(board))  # => "▲７六歩"

amv = mv.to_apery()
print(amv.to_usi())      # => "7g7f"
```

## Move32

```python
rsshogi.core.Move32(value: int)
```

16 ビットの指し手情報に加えて、移動後の色付き駒（`Piece`）を上位ビットに保持する 32 ビット指し手クラスである。
`Board.legal_moves_move32()`、`Board.push_move()`、`Board.last_move()`、`Board.move32_from_move()` などから取得できる。

### Move32 固有のプロパティとメソッド

| プロパティ / メソッド | シグネチャ | 説明 |
|---|---|---|
| `piece_after_move` | `mv32.piece_after_move -> Piece` | 着手後に移動先マスに置かれる色付き駒（成り手なら成り駒、駒打ちなら打った駒）を返す。 |
| `to_move` | `mv32.to_move() -> Move` | 上位ビットの駒情報を落として 16 ビットの `Move` に変換する。 |
| `to_csa` | `mv32.to_csa() -> str \| None` | 手番符号付きの CSA 指し手文字列（例：`"+7776FU"`）を返す。特殊手や駒情報を持たない場合は `None` を返す。 |
| `to_ki2` | `mv32.to_ki2(board: Board) -> str \| None` | 現在局面 `board` を参照して KI2 形式の文字列を返す。変換できない場合は例外を送出せず `None` を返す。 |
| `to_apery` | `mv32.to_apery(board: Board) -> AperyMove32` | 現在局面 `board` から取得駒種を補完して `AperyMove32` に変換する。 |

`Move32.from_usi("7g7f")` で生成したインスタンスは局面情報を経由していないため、上位ビットの `piece_after_move` が空（`Piece(0)`）の「部分 `Move32`」となる。
部分 `Move32` に対して `to_csa()` や `to_ki2(board)` を呼ぶと `None` が返るため、CSA や KI2 への変換や `piece_after_move` の参照には `board.legal_moves_move32()` または `board.move32_from_move(mv)` で取得した完全な `Move32` を使用する。

```python
from rsshogi.core import Board, Move, Move32

board = Board()
mv32 = board.move32_from_move(Move.from_usi("7g7f"))

print(mv32.piece_after_move)  # => P
print(mv32.to_csa())          # => "+7776FU"
print(mv32.to_ki2(board))     # => "▲７六歩"

# Move32.from_usi() は駒情報を持たないため to_csa() は None を返す
partial = Move32.from_usi("7g7f")
assert partial.to_csa() is None
```

## AperyMove

```python
rsshogi.core.AperyMove(value: int)
```

`cshogi` の `move16` や HCPE の `bestMove16` フィールドと同一のビットレイアウトを持つ 16 ビット指し手クラスである。

### AperyMove のコンストラクタとメソッド一覧

| メソッド / プロパティ | シグネチャ | 説明 |
|---|---|---|
| コンストラクタ | `AperyMove(value: int) -> AperyMove` | Apery 16 ビット整数値（`0..=65535`）から生成する。 |
| `from_usi` | `AperyMove.from_usi(usi: str) -> AperyMove` | USI 文字列から `AperyMove` を生成する。 |
| `to_usi` | `amv.to_usi() -> str` | USI 文字列に変換する。 |
| `to_move` | `amv.to_move() -> Move` | `rsshogi` 標準の `Move` に変換する。 |
| `is_drop` | `amv.is_drop() -> bool` | 駒打ちであれば `True` を返す。 |
| `is_promotion` | `amv.is_promotion() -> bool` | 成る手であれば `True` を返す。 |
| `is_normal` | `amv.is_normal() -> bool` | 通常の指し手であれば `True` を返す。 |
| `from_sq` / `to_sq` | `Square` | 移動元と移動先のマスを返す。 |
| `dropped_piece_type` | `PieceType \| None` | 駒打ちの場合に打つ駒種を返す。 |
| `move_type` | `MoveType` | 指し手の種類を返す。 |
| `value` | `int` | 16 ビット整数値を返す。 |

## AperyMove32

```python
rsshogi.core.AperyMove32(value: int)
```

`cshogi` の 32 ビット指し手と互換のクラスであり、下位 16 ビットの `AperyMove` に加えて移動前の駒種と取得駒種を保持する。
現在局面からの生成には `board.apery_move32_from_move(mv)`、`board.apery_move32_from_move32(mv32)`、または `mv32.to_apery(board)` を使用する。

### AperyMove32 のコンストラクタとメソッド一覧

| メソッド / プロパティ | シグネチャ | 説明 |
|---|---|---|
| コンストラクタ | `AperyMove32(value: int) -> AperyMove32` | Apery 32 ビット整数値（`0..=4294967295`）から生成する。 |
| `to_usi` | `amv32.to_usi() -> str` | USI 文字列に変換する。 |
| `to_move` | `amv32.to_move() -> Move` | `rsshogi` 標準の 16 ビット `Move` に変換する。 |
| `to_move32` | `amv32.to_move32(board: Board) -> Move32` | 現在局面 `board` の手番情報を使って `Move32` に変換する。 |
| `to_apery_move` | `amv32.to_apery_move() -> AperyMove` | 下位 16 ビットを取り出して `AperyMove` に変換する。 |
| `is_drop` | `amv32.is_drop() -> bool` | 駒打ちであれば `True` を返す。 |
| `is_promotion` | `amv32.is_promotion() -> bool` | 成る手であれば `True` を返す。 |
| `is_capture` | `amv32.is_capture() -> bool` | 相手の駒を取る手であれば `True` を返す。 |
| `is_normal` | `amv32.is_normal() -> bool` | 通常の指し手であれば `True` を返す。 |
| `piece_type_before` | `PieceType` | 移動前の駒種を返す（駒打ちの場合は `PieceType.NO_PIECE_TYPE`）。 |
| `piece_type_after` | `PieceType` | 着手後に移動先にある駒種を返す。 |
| `captured_piece_type` | `PieceType` | 取得した駒の駒種を返す（駒を取らない手では `PieceType.NO_PIECE_TYPE`）。 |
| `from_sq` / `to_sq` | `Square` | 移動元と移動先のマスを返す。 |
| `dropped_piece_type` | `PieceType \| None` | 駒打ちの場合に打つ駒種を返す。 |
| `move_type` | `MoveType` | 指し手の種類を返す。 |
| `value` | `int` | 32 ビット整数値を返す。 |

`AperyMove32` が保持する駒情報は手番を持たない `PieceType` であり、`Move32` が必要とする色付き駒 `Piece` ではない。
そのため `amv32.to_move32(board)` で `Move32` へ変換する際には現在局面の `board` を渡す必要がある。

```python
from rsshogi.core import Board, Move

board = Board()
board.apply_usi("7g7f")
board.apply_usi("3c3d")

# 2 二角成（駒取り＋成り）を AperyMove32 に変換
mv = Move.from_usi("8h2b+")
amv32 = board.apery_move32_from_move(mv)

print(amv32.is_capture())          # => True
print(amv32.piece_type_before)     # => B (BISHOP)
print(amv32.piece_type_after)      # => +B (HORSE)
print(amv32.captured_piece_type)   # => B (BISHOP)
```

## `to_move`

```python
rsshogi.core.to_move(move: Move | Move32 | int) -> Move
```

`Move`、`Move32`、または 16 ビット符号なし整数を `Move` に正規化するヘルパー関数である。

- `Move` を渡した場合は同じ値を持つ `Move` を返す。
- `Move32` を渡した場合は上位ビットの駒情報を落とした `Move`（`mv32.to_move()` と同等）を返す。
- `int` を渡した場合は `0..=65535`（`0..=0xFFFF`）の範囲内であれば `Move(move)` として変換する。下位 16 ビットのマスク切り詰めは行わず、負数や `65536` 以上の整数を渡した場合は `ValueError` を送出する。
- 上記以外の型（`str` など）を渡した場合は `TypeError` を送出する。

```python
from rsshogi.core import Board, Move, to_move

board = Board()
mv32 = board.legal_moves_move32()[0]

m1 = to_move(mv32)
m2 = to_move(int(m1))
assert m1 == m2
assert isinstance(m1, Move)
```

## 関連項目

- [`Board`](board.md)
- [基本型（`Square` / `PieceType` / `Piece` / `MoveType`）](types.md)
- [NumPy 連携（HCP / HCPE）](numpy.md)
- [USI プロトコル（`move_from_usi`）](usi.md)
