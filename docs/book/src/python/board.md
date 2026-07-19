# Board クラス

```python
rsshogi.core.Board(sfen=None)
```

将棋の盤面を管理するメインクラスです。局面の状態、指し手の適用、合法手生成、
ゲーム状態の判定などの機能を提供します。

Board クラスは以下の機能を持ちます：

- 局面の初期化と SFEN 形式での読み書き
- `PositionState` を介した raw-state の export / import
- 指し手の適用と取り消し（`apply_move` / `undo_move`）
- 合法手と擬似合法手の生成
- ゲーム状態の判定（王手、詰み、千日手など）
- 手番や手数などのメタ情報の取得

## 引数

- **sfen**: `str | None` (デフォルト: `None`)
  初期局面を SFEN 形式で指定します。`None` の場合は平手初期局面になります。

## メソッド一覧

| カテゴリ | 主なメソッド・プロパティ |
|----------|-------------------------|
| [局面の設定と取得](#局面の設定と取得) | `to_sfen` / `set_sfen` / `copy` / `reset` / `to_position_state` / `set_position_state` / `set_usi_position` / `to_csa` / `to_bod` / `validate` / `is_valid` / `validate_all` |
| [局面の設定と取得（シリアライズ）](#局面の設定と取得) | `to_packed_sfen` / `set_packed_sfen` / `to_hcp` / `set_hcp` / `to_psv` / `to_hcpe` / `apery_move32_from_move` / `apery_move32_from_move32` |
| [指し手の適用と取り消し](#指し手の適用と取り消し) | `apply_move` / `apply_move32` / `apply_usi` / `apply_csa` / `push_move` / `push_move32` / `push_usi` / `push_usi_with_delta` / `undo_move` / `undo_move32` / `pop` / `last_move` |
| [盤上の駒の取得](#盤上の駒の取得) | `piece_on` / `is_square_empty` / `empties` |
| [持ち駒と玉の位置](#持ち駒と玉の位置) | `hand` / `hand_counts` / `king_square` / `iter_pieces` |
| [駒のビットボード](#駒のビットボード) | `pieces` / `pieces_by_type` / `pieces_by_color` / `pieces_for` / `golds` / `bishop_horse` / `rook_dragon` |
| [攻撃判定](#攻撃判定) | `attackers_to` / `attackers_to_color` / `attackers_to_color_current` / `attacks_by` / `is_attacked_by` |
| [合法手の生成と判定](#合法手の生成と判定) | `legal_moves` / `legal_moves_move32` / `pseudo_legal_moves` / `pseudo_legal_moves_move32` / `is_legal_move` / `is_legal_move32` |
| [ゲーム状態の判定](#ゲーム状態の判定) | `turn` / `game_ply` / `is_in_check` / `is_mated` / `repetition_state` / `is_repetition` / `can_declare_win` / `evaluate_declaration` / `solve_mate_in_one` / `zobrist_hash` / `move32_from_move` / `move_from_csa` / `to_svg` |

### 例外ポリシー

- **型が不正**: `TypeError`
- **値は型として正しいが内容が不正**（不正な文字列・違法手など）: `ValueError`
- **履歴が空など「未存在」**: `None` を返す（例: `last_move()`, `pop()`）

### 局面の設定と取得

#### `to_sfen()`

```python
board.to_sfen() -> str
```

現在局面を SFEN 形式の文字列で返します。

**戻り値:**
- `str`: SFEN 形式の局面文字列

**使用例:**

```python
board = rsshogi.core.Board()
print(board.to_sfen())
# => "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"
```

---

#### `copy()`

```python
board.copy() -> Board
```

現在局面のコピーを返します。

**戻り値:**
- `Board`: コピーされた盤面

**使用例:**

```python
board = rsshogi.core.Board()
board.apply_usi("7g7f")
clone = board.copy()
print(clone.to_sfen() == board.to_sfen())  # => True
```

---

#### `reset()`

```python
board.reset() -> None
```

盤面を平手初期局面に戻します。

**使用例:**

```python
board = rsshogi.core.Board()
board.apply_usi("7g7f")
board.reset()
print(board.to_sfen())
```

---

#### `set_sfen(sfen)`

```python
board.set_sfen(sfen: str) -> None
```

盤面を指定した SFEN で上書きします。

**引数:**
- **sfen**: `str` - 設定する SFEN 文字列

**例外:**
- `ValueError`: SFEN のパースに失敗した場合

**使用例:**

```python
board = rsshogi.core.Board()
board.set_sfen("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1")
```

---

#### `to_position_state()`

```python
board.to_position_state() -> PositionState
```

現在局面を SFEN 文字列ではなく、編集可能な raw-state として取得します。

**使用例:**

```python
from rsshogi.core import Board

board = Board()
state = board.to_position_state()
print(state.to_sfen())
```

---

#### `set_position_state(state)`

```python
board.set_position_state(state: PositionState) -> None
```

`PositionState` から局面を再構築します。SFEN 文字列を経由しない import 用 API です。
この import は unchecked なので、必要に応じて `validate()` / `validate_all()` を呼んでください。

**使用例:**

```python
from rsshogi.core import Board
from rsshogi.types import Color, Piece, PieceType, Square

board = Board()
state = board.to_position_state()
state.set_piece(Square.from_usi("7g"), Piece(0))
state.set_piece(Square.from_usi("7f"), Piece.from_color_type(Color.BLACK, PieceType.PAWN))
state.side_to_move = Color.WHITE
board.set_position_state(state)
```

---

#### `set_usi_position(position)`

```python
board.set_usi_position(position: str) -> None
```

USI の `position` 形式文字列から局面を設定します。

受け付ける形式:
- `startpos`
- `position startpos`
- `sfen <board> <turn> <hand> <ply>`
- `position sfen <board> <turn> <hand> <ply>`
- 上記に `moves <usi1> <usi2> ...` を付加した形式

`moves` がある場合は手順を順に適用します（不正手・非合法手は `ValueError`）。

**引数:**
- **position**: `str` - USI position 文字列

**例外:**
- `ValueError`: position 文字列が不正、または手順に不正手/非合法手が含まれる場合

**使用例:**

```python
from rsshogi.core import Board

board = Board()
board.set_usi_position("position startpos moves 7g7f 3c3d")
print(board.to_sfen())
# => "lnsgkgsnl/1r5b1/pppppp1pp/6p2/9/2P6/PP1PPPPPP/1B5R1/LNSGKGSNL b - 3"
```

---

#### `to_packed_sfen(out=None)`

```python
board.to_packed_sfen(out=None) -> bytes | None
```

現在局面を PackedSfen (psfen) 形式で取得します。

**引数:**
- **out**: `bytearray | numpy.ndarray | None`  
  出力先バッファ。`None` の場合は `bytes` を返します。  
  指定する場合は以下のいずれか:
  - 長さ 32 の writable `uint8` 配列
  - `dtype=[("sfen", uint8, 32)]` の `numpy.ndarray`（shape `(1,)`）

**戻り値:**
- `bytes | None`: `out is None` なら `bytes`、`out` を渡した場合は `None`

**使用例:**

```python
board = rsshogi.core.Board()
psfen = board.to_packed_sfen()
print(len(psfen))  # => 32

out = bytearray(32)
board.to_packed_sfen(out)
```

---

#### `to_hcp(out=None)`

```python
board.to_hcp(out=None) -> bytes | None
```

現在局面を `cshogi` / Apery 互換の `HuffmanCodedPos`（32 バイト）で取得します。

- `out`: writable `uint8[32]` または `HuffmanCodedPos` ndarray（shape `(1,)`）

```python
import numpy as np
from rsshogi.core import Board
from rsshogi.numpy import HuffmanCodedPos

board = Board()
hcp = board.to_hcp()
print(len(hcp))  # => 32

out = np.zeros(1, dtype=HuffmanCodedPos)
board.to_hcp(out)
```

---

#### `to_psv(mv=None, score=0, game_result=None, game_ply=None, out=None)`

```python
board.to_psv(mv=None, score=0, game_result=None, game_ply=None, out=None) -> bytes | None
```

現在局面から `PackedSfenValue` 1件分（40バイト）を直接生成します。
`out=None` の場合は `bytes` を返し、`out` 指定時は `None` を返します。

- `mv`: `Move | Move32 | int | str | None`（未指定時は `MOVE_NONE`）
- `score`: `int16`
- `game_result`: `GameResult | int | str | None`（未指定時は `INVALID`）
- `game_ply`: `int | None`（未指定時は `board.game_ply`）
- `out`: writable `uint8[40]` または `PackedSfenValue` ndarray（shape `(1,)`）

---

#### `to_hcpe(best_move=None, score=0, game_result=None, out=None)`

```python
board.to_hcpe(best_move=None, score=0, game_result=None, out=None) -> bytes | None
```

現在局面から `HuffmanCodedPosAndEval` 1件分（38 バイト）を生成します。

- `best_move`: `AperyMove | AperyMove32 | Move | Move32 | int | str | None`
- `score`: `int16`
- `game_result`: `GameResult | int | str`
  - `Draw` / `BlackWin` / `WhiteWin` に変換できる値のみ受理します。
- `out`: writable `uint8[38]` または `HuffmanCodedPosAndEval` ndarray（shape `(1,)`）

`best_move` は最終的に Apery 16bit (`bestMove16`) へ正規化されます。
`Move32` や USI 文字列を渡した場合も、局面に応じて `AperyMove` に変換してから書き出します。
`game_result` は必須です。未指定のまま `hcpe` を出力することはできません。

```python
import numpy as np
from rsshogi.core import Board
from rsshogi.numpy import HuffmanCodedPosAndEval

board = Board()
hcpe = board.to_hcpe(best_move="7g7f", score=84, game_result="BLACK_WIN")
print(len(hcpe))  # => 38

out = np.zeros(1, dtype=HuffmanCodedPosAndEval)
board.to_hcpe(best_move="7g7f", score=84, game_result=1, out=out)
```

---

#### `set_packed_sfen(psfen)`

```python
board.set_packed_sfen(psfen) -> None
```

PackedSfen (psfen) 形式の 32 バイト列から局面を設定します。
psfen は 互換の PackedSfen 形式（32 バイト）です。

**引数:**
- **psfen**: 次のいずれか
  - `bytes` / `bytearray`
  - 長さ 32 の `Sequence[int]`
  - 長さ 32 の `numpy.ndarray`（`uint8`）
  - `dtype=[("sfen", uint8, 32)]` の `numpy.ndarray`（shape `(1,)`）

**例外:**
- `ValueError`: 長さが 32 バイトでない場合
- `TypeError`: 受け取り型が不正な場合

**使用例:**

```python
board = rsshogi.core.Board()
psfen = board.to_packed_sfen()
board.set_packed_sfen(psfen)
```

---

#### `set_hcp(hcp, game_ply=1)`

```python
board.set_hcp(hcp, game_ply=1) -> None
```

`HuffmanCodedPos` 形式の 32 バイト列から局面を設定します。

**引数:**
- **hcp**: 次のいずれか
  - `bytes` / `bytearray`
  - 長さ 32 の `Sequence[int]`
  - 長さ 32 の `numpy.ndarray`（`uint8`）
  - `dtype=[("hcp", uint8, 32)]` の `numpy.ndarray`（shape `(1,)`）
- **game_ply**: `int` - 設定後の手数

```python
board = rsshogi.core.Board()
hcp = board.to_hcp()
board.set_hcp(hcp, game_ply=17)
```

---

#### `apery_move32_from_move(mv)`

```python
board.apery_move32_from_move(mv: Move) -> AperyMove32
```

現在局面の駒情報（取得駒種など）を補完して `Move` を `AperyMove32` に変換します。

---

#### `apery_move32_from_move32(mv)`

```python
board.apery_move32_from_move32(mv: Move32) -> AperyMove32
```

現在局面を使って `Move32` を `AperyMove32` に変換します。
取得駒種の補完が必要なため、`Board` の文脈を使います。

---

#### `validate()`

```python
board.validate() -> None
```

現在局面を検証し、最初に見つかった問題を `ValueError` として送出します。

---

#### `is_valid()`

```python
board.is_valid() -> bool
```

`validate()` と同じ条件で局面が妥当かどうかを返します。

---

#### `validate_all()`

```python
board.validate_all() -> ValidationReport
```

現在局面の問題をすべて収集して返します。raw-state 編集後の全件確認に向いています。

**使用例:**

```python
from rsshogi.core import Board

board = Board()
report = board.validate_all()
print(report.is_valid())
```

---

#### `to_csa()`

```python
board.to_csa() -> str
```

現在局面を CSA 形式の局面行（`P1..P9`, `P+`, `P-`, `+/-`）として返します。

**戻り値:**
- `str`: CSA 形式の局面行

**使用例:**

```python
board = rsshogi.core.Board()
print(board.to_csa())
```

---

#### `to_bod()`

```python
board.to_bod() -> str
```

現在局面を BOD 形式（将棋ウォーズなどで使われる形式）の文字列として返します。
直前の指し手がある場合は、局面と合わせて「手数＝N　○○　まで」の形式で出力します。

**戻り値:**
- `str`: BOD 形式の文字列

**使用例:**

```python
board = rsshogi.core.Board()
board.apply_usi("7g7f")
print(board.to_bod())
```

---

### 指し手の適用と取り消し

#### `apply_move(move)`

```python
board.apply_move(move) -> None
```

指し手を適用して局面を進めます。`move` は `Move` を指定します。
`Move32` を適用する場合は `apply_move32()` を使います。

**引数:**
- **move**: `Move`
  適用する指し手。

**例外:**
- `ValueError`: 違法手を適用しようとした場合
- `TypeError`: サポートされていない型を渡した場合

**使用例:**

```python
board = rsshogi.core.Board()

# USI 文字列で指す
board.apply_usi("7g7f")

# Move で指す
move = rsshogi.core.Move.from_usi("3c3d")
board.apply_move(move)

# 合法手リストから指す
board.apply_move(board.legal_moves()[0])
```

---

#### `apply_move32(move)`

```python
board.apply_move32(move) -> None
```

`Move32` を受け取り、指し手を適用して局面を進めます。

**引数:**
- **move**: `Move32` - 適用する指し手

**例外:**
- `ValueError`: 違法手を適用しようとした場合

---

#### `push_move(move)`

```python
board.push_move(move) -> Move32
```

`apply_move()` と同様に指し手を適用し、適用した `Move32`（32bit）を返します。

---

#### `push_move32(move)`

```python
board.push_move32(move) -> Move32
```

`apply_move32()` と同様に `Move32` を適用し、適用した `Move32` を返します。

---

#### `push_usi(usi)`

```python
board.push_usi(usi: str) -> Move32
```

USI 文字列で手を進め、適用した `Move32`（32bit）を返します。

---

#### `push_usi_with_delta(usi)`

```python
board.push_usi_with_delta(usi: str) -> dict
```

USI 文字列で手を進め、適用した `Move32` と差分情報（ハッシュ変化・取得駒種など）を辞書で返します。

**例外:**
- `ValueError`: USI 文字列のパースに失敗した場合、または違法手の場合

---

#### `apply_usi(usi)`

```python
board.apply_usi(usi: str) -> None
```

USI 文字列を受け取り、指し手を適用します。文字列入力が必要な場合に使います。

**引数:**
- **usi**: `str` - USI 形式の指し手文字列（例: `"7g7f"`, `"P*5e"`）

**例外:**
- `ValueError`: 文字列のパースに失敗した場合、または違法手の場合

**使用例:**

```python
board = rsshogi.core.Board()
board.apply_usi("7g7f")
```

---

#### `apply_csa(csa)`

```python
board.apply_csa(csa: str) -> None
```

CSA 文字列を受け取り、指し手を適用します。先頭の手番記号（`+` / `-`）があっても受け付けます。

**引数:**
- **csa**: `str` - CSA 形式の指し手文字列（例: `"7776FU"`, `"+7776FU"`）

**例外:**
- `ValueError`: 文字列のパースに失敗した場合、または違法手の場合

**使用例:**

```python
board = rsshogi.core.Board()
board.apply_csa("7776FU")
```

---

#### `undo_move(move)`

```python
board.undo_move(move) -> None
```

指定した指し手を取り消し、1手戻します。

**引数:**
- **move**: `Move` - 取り消す指し手（`Move32` の場合は `undo_move32()` を使用）

**例外:**
- `ValueError`: 指し手の取り消しに失敗した場合

**使用例:**

```python
board = rsshogi.core.Board()
move = rsshogi.core.Move.from_usi("7g7f")
board.apply_move(move)
board.undo_move(move)  # 適用したのと同じ Move を渡す
```

---

#### `undo_move32(move)`

```python
board.undo_move32(move) -> None
```

`Move32` を指定して取り消し、1手戻します。直前の指し手と一致する必要があります。

**引数:**
- **move**: `Move32` - 取り消す指し手

**例外:**
- `ValueError`: 直前の指し手と一致しない場合

---

#### `pop()`

```python
board.pop() -> Move32 | None
```

直前の指し手を取り消して返します。履歴が空なら `None`。

---

#### `last_move()`

```python
board.last_move() -> Move32 | None
```

直前の指し手を取得します（盤面は変更されません）。

**戻り値:**
- `Move32 | None`: 直前の指し手。履歴が空の場合は `None`

**使用例:**

```python
board = rsshogi.core.Board()
board.apply_usi("7g7f")
last_move = board.last_move()
if last_move:
    print(last_move.to_usi())  # => "7g7f"
```

---

### 盤上の駒の取得

#### `piece_on(square)`

```python
board.piece_on(square: Square) -> Piece
```

指定したマスにある駒を取得します。

**引数:**
- **square**: `Square` - 対象のマス

**戻り値:**
- `Piece`: マスにある駒。空きマスの場合は `piece_type` が `PieceType.NO_PIECE_TYPE` を返す

**使用例:**

```python
from rsshogi.core import Board
from rsshogi.types import Square

board = Board()
sq = Square.from_usi("7g")
piece = board.piece_on(sq)
print(piece.piece_type)  # => PieceType.PAWN
print(piece.color)       # => Color.BLACK
```

---

#### `is_square_empty(square)`

```python
board.is_square_empty(square: Square) -> bool
```

指定したマスが空かどうかを判定します。

**引数:**
- **square**: `Square` - 対象のマス

**戻り値:**
- `bool`: 空きマスの場合 `True`

**使用例:**

```python
from rsshogi.core import Board
from rsshogi.types import Square

board = Board()
sq = Square.from_usi("5e")
print(board.is_square_empty(sq))  # => True（初期局面で5五は空）
```

---

#### `empties()`

```python
board.empties() -> Bitboard
```

空きマスの集合をビットボードで取得します。

**戻り値:**
- `Bitboard`: 空きマスのビットボード

**使用例:**

```python
board = Board()
empties = board.empties()
print(empties.count())  # => 61（初期局面の空きマス数）
```

---

### 持ち駒と玉の位置

#### `hand(color)`

```python
board.hand(color: Color) -> Hand
```

指定した手番の持ち駒を取得します。

**引数:**
- **color**: `Color` - 手番（`Color.BLACK` または `Color.WHITE`）

**戻り値:**
- `Hand`: 持ち駒オブジェクト

**使用例:**

```python
from rsshogi.core import Board
from rsshogi.types import Color, PieceType

board = Board()
# 持ち駒のある局面を設定
board.set_sfen("lnsgkgsnl/1r5b1/pppppp1pp/6p2/9/2P6/PP1PPPPPP/1B5R1/LNSGKGSNL b P 1")

hand = board.hand(Color.BLACK)
print(hand.count(PieceType.PAWN))  # => 1
```

---

#### `hand_counts(color)`

```python
board.hand_counts(color: Color) -> dict[str, int]
```

指定した手番の持ち駒枚数を USI 駒文字をキーとする辞書で返します。
常に `"P"`, `"L"`, `"N"`, `"S"`, `"B"`, `"R"`, `"G"` の 7 キーを含み、持っていない駒は `0` です。

```python
from rsshogi.core import Board
from rsshogi.types import Color

board = Board()
counts = board.hand_counts(Color.BLACK)
print(counts["P"])  # => 0（初期局面）
```

---

#### `king_square(color)`

```python
board.king_square(color: Color) -> Square
```

指定した手番の玉の位置を取得します。

**引数:**
- **color**: `Color` - 手番

**戻り値:**
- `Square`: 玉のマス

**使用例:**

```python
from rsshogi.core import Board
from rsshogi.types import Color

board = Board()
sq = board.king_square(Color.BLACK)
print(sq.to_usi())  # => "5i"
```

---

#### `iter_pieces()`

```python
board.iter_pieces() -> list[tuple[Square, Piece]]
```

盤上にある駒だけを `(Square, Piece)` のリストで返します。空マスは含みません。
順序は内部 square index の昇順（`1a`, `1b`, ..., `9i`）です。

```python
from rsshogi.core import Board

board = Board()
for square, piece in board.iter_pieces():
    print(square.to_usi(), piece)
```

---

### 駒のビットボード

盤上の駒をビットボードで取得するメソッド群です。

#### `pieces()`

```python
board.pieces() -> Bitboard
```

全ての駒があるマスをビットボードで取得します。

---

#### `pieces_by_type(piece_type)`

```python
board.pieces_by_type(piece_type: PieceType) -> Bitboard
```

指定した駒種のマスをビットボードで取得します（先後両方）。

**使用例:**

```python
from rsshogi.core import Board
from rsshogi.types import PieceType

board = Board()
pawns = board.pieces_by_type(PieceType.PAWN)
print(pawns.count())  # => 18（先後合計）
```

---

#### `pieces_by_color(color)`

```python
board.pieces_by_color(color: Color) -> Bitboard
```

指定した手番の全駒をビットボードで取得します。

---

#### `pieces_for(piece_type, color)`

```python
board.pieces_for(piece_type: PieceType, color: Color) -> Bitboard
```

指定した駒種と手番の駒をビットボードで取得します。

---

#### その他のビットボード取得

```python
board.golds() -> Bitboard         # 金相当の駒（金、と、成香、成桂、成銀）
board.bishop_horse() -> Bitboard  # 角と馬
board.rook_dragon() -> Bitboard   # 飛と龍
```

---

### 攻撃判定

#### `attackers_to(square, occupied)`

```python
board.attackers_to(square: Square, occupied: Bitboard) -> Bitboard
```

指定したマスを攻撃している駒のビットボードを取得します。

**引数:**
- **square**: `Square` - 対象のマス
- **occupied**: `Bitboard` - 駒がある位置（通常は `board.pieces()`）

**戻り値:**
- `Bitboard`: 攻撃している駒の位置

---

#### `attackers_to_color(color, square, occupied)`

```python
board.attackers_to_color(color: Color, square: Square, occupied: Bitboard) -> Bitboard
```

指定した手番の駒で、マスを攻撃しているものを取得します。

---

#### `attackers_to_color_current(color, square)`

```python
board.attackers_to_color_current(color: Color, square: Square) -> Bitboard
```

現在の盤面で、指定した手番の駒がマスを攻撃しているものを取得します。

---

#### `attacks_by(color, piece_type)`

```python
board.attacks_by(color: Color, piece_type: PieceType) -> Bitboard
```

指定した手番・駒種の攻撃範囲をビットボードで取得します。

---

#### `is_attacked_by(square, by_color)`

```python
board.is_attacked_by(square: Square, by_color: Color) -> bool
```

指定したマスが、指定した手番の駒に攻撃されているかを判定します。

**使用例:**

```python
from rsshogi.core import Board
from rsshogi.types import Color, Square

board = Board()
board.apply_usi("7g7f")

sq = Square.from_usi("7e")
# 7六の歩が7五を攻撃しているか
print(board.is_attacked_by(sq, Color.BLACK))  # => True
```

---

### 合法手の生成と判定

#### `legal_moves()`

```python
board.legal_moves() -> list[Move]
```

現在局面の全合法手をリストで返します。歩・香・角・飛の不成も含めて生成します。

**戻り値:**
- `list[Move]`: 合法手のリスト

**使用例:**

```python
board = rsshogi.core.Board()
for move in board.legal_moves():
    print(move.to_usi())
```

---

#### `legal_moves_move32()`

```python
board.legal_moves_move32() -> list[Move32]
```

現在局面の全合法手を `Move32`（32bit）で返します。詳細な駒情報が必要な場合に使用します。

**戻り値:**
- `list[Move32]`: 合法手のリスト

**使用例:**

```python
board = rsshogi.core.Board()
for move in board.legal_moves_move32():
    print(move.to_usi())
```

---

#### `pseudo_legal_moves()`

```python
board.pseudo_legal_moves() -> list[Move]
```

擬似合法手（王手放置を含む可能性のある手）をリストで返します。
王手中の場合は回避手も含まれます。

**戻り値:**
- `list[Move]`: 擬似合法手のリスト

**注意:**
探索や局面生成でローカルにフィルタリングする用途向けです。
通常の用途では `legal_moves()` を使用してください。

**使用例:**

```python
board = rsshogi.core.Board()
for move in board.pseudo_legal_moves():
    if board.is_legal_move(move):
        print(move.to_usi())
```

---

#### `pseudo_legal_moves_move32()`

```python
board.pseudo_legal_moves_move32() -> list[Move32]
```

擬似合法手（王手放置を含む可能性のある手）を `Move32`（32bit）で返します。

**戻り値:**
- `list[Move32]`: 擬似合法手のリスト

**使用例:**

```python
board = rsshogi.core.Board()
for move in board.pseudo_legal_moves_move32():
    if board.is_legal_move32(move):
        print(move.to_usi())
```

---

#### `is_legal_move(move)`

```python
board.is_legal_move(move) -> bool
```

指定した指し手が合法手かどうかを判定します。`Move32` の場合は `is_legal_move32()` を使います。

**引数:**
- **move**: `Move` - 判定する指し手

**戻り値:**
- `bool`: 合法手なら `True`、そうでなければ `False`

`move` は `Move` を受け付けます。

**使用例:**

```python
board = rsshogi.core.Board()
move = rsshogi.core.Move.from_usi("7g7f")
if board.is_legal_move(move):
    board.apply_move(move)
```

---

#### `is_legal_move32(move)`

```python
board.is_legal_move32(move) -> bool
```

`Move32` が合法手かどうかを判定します。

**引数:**
- **move**: `Move32` - 判定する指し手

**戻り値:**
- `bool`: 合法手なら `True`、そうでなければ `False`

**使用例:**

```python
board = rsshogi.core.Board()
for move in board.pseudo_legal_moves_move32():
    if board.is_legal_move32(move):
        board.apply_move32(move)
        break
```

---

### ゲーム状態の判定

#### `turn`

```python
board.turn: Color
```

現在の手番を `Color` 型で取得します。`Color.BLACK`/`Color.WHITE` がそのまま返るので型安全な比較が可能です。

---

#### `zobrist_hash()`

```python
board.zobrist_hash() -> int
```

現在局面の Zobrist ハッシュ値を返します（局面キー用途）。

---

#### `can_declare_win()`

```python
board.can_declare_win() -> bool
```

現在手番で入玉宣言勝ち可能かを判定します。

---

#### `evaluate_declaration()`

```python
board.evaluate_declaration() -> dict
```

入玉宣言の詳細評価を辞書形式で返します。点数計算の内訳（大駒点数・小駒点数・総点数など）が含まれます。

---

#### `solve_mate_in_one()`

```python
board.solve_mate_in_one() -> Move32 | None
```

現在局面で1手詰めが存在すれば、詰め手を `Move32` で返します。存在しない場合は `None`。

```python
board = rsshogi.core.Board()
# ... 1手詰め局面を設定 ...
mate = board.solve_mate_in_one()
if mate:
    print(mate.to_usi())
```

---

#### `move32_from_move(mv)`

```python
board.move32_from_move(mv: Move) -> Move32
```

`Move`（16bit）を現在の局面を参照して完全な `Move32`（32bit、駒情報付き）に変換します。
返される Move32 は `piece_after_move` や CSA/KI2 変換に使用できます。

`Move32.from_usi()` で生成した Move32 には駒情報が含まれない（部分 Move32）ため、
駒情報が必要な場合はこのメソッドで完全化してください。

---

#### `move_from_csa(csa)`

```python
board.move_from_csa(csa: str) -> Move32
```

CSA 文字列から局面依存の `Move32`（32bit）を復元します（線形探索不要）。

---

#### `game_ply`

```python
board.game_ply: int
```

初期局面からの手数（ply）を返します。USI 標準に準拠しています。

**戻り値:**
- `int`: 手数

**使用例:**

```python
board = rsshogi.core.Board()
print(board.game_ply)  # => 1
board.apply_usi("7g7f")
print(board.game_ply)  # => 2
```

---

#### `is_in_check()`

```python
board.is_in_check() -> bool
```

現在の手番の玉が王手にさらされているかを判定します。

**戻り値:**
- `bool`: 王手されている場合 `True`

**使用例:**

```python
board = rsshogi.core.Board()
if board.is_in_check():
    print("王手!")
```

---

#### `is_mated()`

```python
board.is_mated() -> bool
```

現在局面が詰み（王手かつ合法手なし）かどうかを判定します。

**戻り値:**
- `bool`: 詰みの場合 `True`

**使用例:**

```python
board = rsshogi.core.Board()
if board.is_mated():
    print("詰み!")
```

---

#### `repetition_state()`

```python
board.repetition_state() -> RepetitionState
```

千日手の状態を返します。

**戻り値:**
- `RepetitionState`: `NONE`, `WIN`, `LOSE`, `DRAW`, `SUPERIOR`, `INFERIOR`

**使用例:**

```python
board = rsshogi.core.Board()
state = board.repetition_state()
if state.to_usi() == "rep_draw":
    print("千日手引き分け")
```

---

#### `is_repetition(threshold=3)`

```python
board.is_repetition(threshold: int = 3) -> bool
```

千日手が発生しているかを返します。

**引数:**
- **threshold**: `int` - 千日手と判定する繰り返し回数（通常は 3）

**戻り値:**
- `bool`: 千日手の場合 `True`

**使用例:**

```python
board = rsshogi.core.Board()
if board.is_repetition():
    print("千日手発生")
```

---

#### `to_svg(last_move=None, scale=1.0)`

```python
board.to_svg(last_move=None, scale=1.0) -> Svg
```

現在の局面を SVG 画像として生成します。

**引数:**
- **last_move**: `Move | Move32 | None` - 最後の指し手（ハイライト表示）。`Move` または `Move32` のみ受け付けます。
- **scale**: `float` - 拡大率（デフォルト `1.0`）

**戻り値:**
- `Svg`: SVG 画像オブジェクト（Jupyter Notebook では自動的に画像表示）

**使用例:**

```python
from rsshogi.core import Board, Move

board = Board()
move = Move.from_usi("7g7f")
board.apply_move(move)
svg = board.to_svg(last_move=move)
```

---

## 使用例

### 基本的な対局の流れ

```python
import rsshogi

# 平手初期局面
board = rsshogi.core.Board()

# 指し手を適用
board.apply_usi("7g7f")
board.apply_usi("3c3d")
board.apply_usi("2g2f")

# 局面情報を表示
print(f"手番: {board.turn}")
print(f"手数: {board.game_ply}")
print(f"SFEN: {board.to_sfen()}")

# 王手判定
if board.is_in_check():
    print("王手!")

# 合法手を列挙
print("合法手:")
for move in board.legal_moves():
    print(f"  {move.to_usi()}")
```

---

### SFEN から局面を読み込む

```python
import rsshogi

# 特定の局面を SFEN で設定
sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"
board = rsshogi.core.Board(sfen=sfen)

print(board.to_sfen())
```

---

### 指し手の履歴を管理（アプリ側で保持）

```python
import rsshogi

board = rsshogi.core.Board()

# 複数手指す（履歴は自分で保持）
history = []
moves = ["7g7f", "3c3d", "2g2f", "4c4d"]
for usi in moves:
    mv = rsshogi.core.Move.from_usi(usi)
    board.apply_move(mv)
    history.append(mv)

# 履歴を表示
print("指し手履歴:")
for move in history:
    print(f"  {move.to_usi()}")

# 2手戻す
for _ in range(2):
    last_move = board.last_move()
    if last_move:
        board.undo_move32(last_move)

print(f"\n2手戻した後の手数: {board.game_ply}")
```

---

### ゲーム終了判定

```python
import rsshogi

board = rsshogi.core.Board()

# 対局を進める（実際には多くの手が必要）
while True:
    moves = board.legal_moves()
    if not moves:
        break
    # 最初の合法手を選択（実際にはもっと賢い選択が必要）
    board.apply_move(moves[0])
    
    if board.game_ply > 100:  # 無限ループ防止
        break
    if board.repetition_state().to_usi() != "rep_none":
        break

# 終局状態を判定
if board.is_mated():
    print("詰み")
elif board.is_repetition():
    state = board.repetition_state()
    print(f"千日手: {state.to_usi()}")
```

---

## PositionState

```python
rsshogi.core.PositionState(sfen=None)
```

`Board.to_position_state()` が返す raw-state オブジェクトです。盤面 81 マス、持ち駒、手番、手数を
構造化して保持します。

- `piece_on(square)` / `set_piece(square, piece)` で盤上駒を編集
- `hand(color)` / `set_hand(color, hand)` で持ち駒を編集
- `side_to_move`, `ply` を直接変更
- `to_sfen(include_ply=True)` で SFEN へ戻す
- `validate()` / `is_valid()` / `validate_all()` で検証

`board` プロパティは 81 要素の `list[Piece]`、`hands` プロパティは
`(black_hand, white_hand)` の `tuple[Hand, Hand]` を返します。

## ValidationReport

```python
rsshogi.core.ValidationReport
```

`validate_all()` の戻り値です。

- `is_valid()` で問題がないか確認
- `issues` プロパティで `ValidationIssue` の一覧を取得

## ValidationIssue

```python
rsshogi.core.ValidationIssue
```

個別の validation 問題です。主なプロパティ:

- `kind`: `NO_KING`, `TWO_KINGS`, `DOUBLE_PAWN`, `INVALID_HAND_COUNT`, `INVALID_PLACEMENT`
- `color`: 色に紐づく問題なら `Color`
- `file_usi`: 二歩なら `"1"`..`"9"`
- `square`: 配置違反なら対象マス
- `piece_type`: 配置違反や持ち駒上限違反の対象駒種
- `count`: 持ち駒上限違反時の枚数
- `message`: 人間向けメッセージ

---

## 関連項目

- [`Move32`](move32.md) - 32bit 指し手型
- [`Move`](move.md) - 16bit 指し手型
- [`GameResult`](game_result.md) - 終局結果
- [`Record`](record.md) - 棋譜レコード
- [クイックスタート](../getting-started/quickstart.md)
- [例とパターン](../getting-started/examples.md)
