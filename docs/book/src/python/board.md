# 盤面管理（`Board` / `PositionState`）

`rsshogi.core.Board` は将棋の盤面状態、指し手の適用と取り消し、合法手生成、対局状態の判定を担う中核クラスである。
また、盤面の個別マスや持ち駒を直接編集する用途向けに、構造化データクラス `PositionState` と妥当性検証クラス `ValidationReport`、`ValidationIssue` を提供する。

```python
from rsshogi.core import (
    Board,
    PositionState,
    ValidationIssue,
    ValidationReport,
)
```

## クラス一覧と例外ポリシー

| クラス | 役割 |
|---|---|
| `Board` | 合法手生成、指し手の進退、王手や千日手の判定、各種シリアライズを行う盤面クラス。 |
| `PositionState` | 盤上 81 マス、持ち駒、手番、手数を直接読み書きできる局面データクラス。 |
| `ValidationReport` | `validate_all()` が返す妥当性検証結果の集計オブジェクト。 |
| `ValidationIssue` | 局面の妥当性検証で検出された個別の問題項目。 |

`Board` および `PositionState` のメソッドは以下の例外ポリシーに従う。

- **型エラー（`TypeError`）**：引数の型がシグネチャと一致しない場合に送出する。
- **値エラー（`ValueError`）**：不正な SFEN 文字列、非合法手、表現範囲外の数値、盤面上で解決できない指し手などを渡した場合に送出する。
- **未存在の表現（`None`）**：`last_move()` や `pop()` で着手履歴が空の場合、または `solve_mate_in_one()` で 1 手詰めが存在しない場合は例外を送出せず `None` を返す。

## メソッドとプロパティの早見表

| カテゴリ | メソッドとプロパティ |
|---|---|
| [局面の初期化と入出力](#局面の初期化と入出力) | `Board(sfen=None)`, `copy()`, `reset()`, `to_sfen()`, `set_sfen()`, `set_usi_position()`, `to_position_state()`, `set_position_state()`, `to_csa()`, `to_bod()`, `to_svg()` |
| [局面の妥当性検証](#局面の妥当性検証) | `validate()`, `is_valid()`, `validate_all()` |
| [バイナリシリアライズと Apery 互換変換](#バイナリシリアライズと-apery-互換変換) | `to_packed_sfen()`, `set_packed_sfen()`, `to_psv()`, `to_hcp()`, `set_hcp()`, `to_hcpe()`, `apery_move32_from_move()`, `apery_move32_from_move32()` |
| [指し手の適用と取り消し](#指し手の適用と取り消し) | `apply_move()`, `apply_move32()`, `apply_usi()`, `apply_csa()`, `push_move()`, `push_move32()`, `push_usi()`, `push_usi_with_delta()`, `undo_move()`, `undo_move32()`, `pop()`, `last_move()`, `move32_from_move()`, `move_from_csa()` |
| [盤上の駒、持ち駒、ビットボードの参照](#盤上の駒持ち駒ビットボードの参照) | `piece_on()`, `is_square_empty()`, `empties()`, `hand()`, `hand_counts()`, `king_square()`, `iter_pieces()`, `pieces()`, `pieces_by_type()`, `pieces_by_color()`, `pieces_for()`, `golds()`, `bishop_horse()`, `rook_dragon()` |
| [利きと攻撃の判定](#利きと攻撃の判定) | `attackers_to()`, `attackers_to_color()`, `attackers_to_color_current()`, `attacks_by()`, `is_attacked_by()` |
| [合法手と擬似合法手の生成および判定](#合法手と擬似合法手の生成および判定) | `legal_moves()`, `legal_moves_move32()`, `pseudo_legal_moves()`, `pseudo_legal_moves_move32()`, `is_legal_move()`, `is_legal_move32()` |
| [対局状態、入玉宣言、1 手詰めの判定](#対局状態入玉宣言1-手詰めの判定) | `turn`, `game_ply`, `zobrist_hash()`, `is_in_check()`, `is_mated()`, `repetition_state()`, `is_repetition()`, `can_declare_win()`, `evaluate_declaration()`, `solve_mate_in_one()` |

---

## 局面の初期化と入出力

```python
Board(sfen: str | None = None) -> Board
```

`sfen` に `None` を渡すと平手初期局面で初期化する。
任意の初期局面から開始する場合は SFEN 文字列を指定する。

> [!NOTE]
> `Board()` は引数省略時に平手初期局面になるが、後述する [`PositionState()`](#positionstate) は引数省略時に駒が置かれていない空盤面（`"9/9/9/9/9/9/9/9/9 b - 1"`）になる。

| メソッド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `copy()` / `__copy__()` | なし | `Board` | 現在の盤面と着手履歴を複製した新しい `Board` を返す。 |
| `reset()` | なし | `None` | 盤面を平手初期局面に戻し、着手履歴をクリアする。 |
| `to_sfen()` | なし | `str` | 現在局面を SFEN 文字列として返す。 |
| `set_sfen(sfen)` | `sfen: str` | `None` | 指定した SFEN 文字列で盤面を上書きする。不正な SFEN は `ValueError` を送出する。 |
| `set_usi_position(position)` | `position: str` | `None` | USI の `position` コマンド文字列から初期局面と手順を読み込んで適用する。 |
| `to_position_state()` | なし | `PositionState` | 現在局面を編集可能な `PositionState` として書き出す。 |
| `set_position_state(state)` | `state: PositionState` | `None` | `PositionState` から盤面を再構築する。 |
| `to_csa()` | なし | `str` | 現在局面を CSA 形式の局面行（`P1` から `P9`、`P+`、`P-`、手番行）として返す。 |
| `to_bod()` | なし | `str` | 現在局面を BOD 形式の文字列として返す。直前の着手がある場合は最終手表記も含める。 |
| `to_svg(last_move=None, scale=1.0, *, highlight_squares=True, bold_destination=False, board_background=None)` | 描画オプション | `Svg` | 現在局面を SVG オブジェクトとして出力する（詳細は [SVG 出力](svg.md) を参照）。 |
| `str(board)` | なし | `str` | `to_sfen()` と同じ SFEN 文字列を返す。 |

`set_usi_position(position)` は以下の形式を受理し、`moves` 以降の手順を合法手として順番に適用する。

- `"startpos"` または `"position startpos"`
- `"sfen <board> <turn> <hand> <ply>"` または `"position sfen <board> <turn> <hand> <ply>"`
- 上記に `"moves <usi1> <usi2> ..."` を続けた文字列

```python
from rsshogi.core import Board

board = Board()
board.set_usi_position("position startpos moves 7g7f 3c3d")
print(board.to_sfen())
# => "lnsgkgsnl/1r5b1/pppppp1pp/6p2/9/2P6/PP1PPPPPP/1B5R1/LNSGKGSNL b - 3"

clone = board.copy()
print(board.to_csa())
print(board.to_bod())
```

---

## 局面の妥当性検証

`set_position_state()` などで構築した盤面が将棋のルール上妥当な配置を満たしているかを検証する。

| メソッド | 戻り値 | 説明 |
|---|---|---|
| `validate()` | `None` | 現在局面を検証し、最初に見つかった問題を `ValueError` として送出する。問題がなければ `None` を返す。 |
| `is_valid()` | `bool` | `validate()` と同一の条件で局面が妥当であれば `True`、問題があれば `False` を返す。 |
| `validate_all()` | `ValidationReport` | 現在局面で検出されたすべての問題を `ValidationReport` にまとめて返す。 |

---

## バイナリシリアライズと Apery 互換変換

学習データや教師局面ファイルの入出力向けに、YaneuraOu 系の `PackedSfen` / `PackedSfenValue`（PSV）および Apery / `cshogi` 系の `HuffmanCodedPos`（HCP）/ `HuffmanCodedPosAndEval`（HCPE）への直接変換を提供する。
NumPy の構造化配列との連携手順は [NumPy 連携](numpy.md) を参照。

| メソッド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `to_packed_sfen(out=None)` | `out`: 出力先バッファまたは `None` | `bytes \| None` | 現在局面を 32 バイトの `PackedSfen` に変換する。`out=None` なら `bytes` を返し、`out` 指定時はインプレース書き込みを行い `None` を返す。 |
| `set_packed_sfen(psfen)` | `psfen`: 32 バイトデータ | `None` | 32 バイトの `PackedSfen` から盤面を復元する。 |
| `to_psv(mv=None, score=0, game_result=None, game_ply=None, out=None)` | 教師データ各項目と `out` | `bytes \| None` | 現在局面から 40 バイトの `PackedSfenValue` を生成する。 |
| `to_hcp(out=None)` | `out`: 出力先バッファまたは `None` | `bytes \| None` | 現在局面を 32 バイトの `HuffmanCodedPos` に変換する。 |
| `set_hcp(hcp, game_ply=1)` | `hcp`: 32 バイトデータ, `game_ply: int = 1` | `None` | 32 バイトの `HuffmanCodedPos` と指定手数から盤面を復元する。 |
| `to_hcpe(best_move=None, score=0, game_result=None, out=None)` | 教師データ各項目と `out` | `bytes \| None` | 現在局面から 38 バイトの `HuffmanCodedPosAndEval` を生成する。 |
| `apery_move32_from_move(mv)` | `mv: Move` | `AperyMove32` | 現在局面の駒情報を参照して `Move` を 32 ビットの `AperyMove32` に変換する。 |
| `apery_move32_from_move32(mv)` | `mv: Move32` | `AperyMove32` | 現在局面の取得駒情報を補完して `Move32` を `AperyMove32` に変換する。 |

### 出力先バッファ `out` と入力引数の仕様

- **`out` バッファの受け入れ型**：
  - `to_packed_sfen` / `to_hcp`：書き込み可能な長さ 32 の `bytearray` や `uint8` 1 次元 `ndarray`、または要素数 1 の `PackedSfen` / `HuffmanCodedPos` 構造化 `ndarray`（shape `(1,)` または 0 次元スライス `arr[i]`）。
  - `to_psv`：書き込み可能な長さ 40 の `bytearray` や `uint8` 1 次元 `ndarray`、または `PackedSfenValue` 構造化 `ndarray`（shape `(1,)` または 0 次元スライス `arr[i]`）。
  - `to_hcpe`：書き込み可能な長さ 38 の `bytearray` や `uint8` 1 次元 `ndarray`、または `HuffmanCodedPosAndEval` 構造化 `ndarray`（shape `(1,)` または 0 次元スライス `arr[i]`）。
- **`set_packed_sfen` / `set_hcp` の入力型**：
  - 長さ 32 の `bytes`、`bytearray`、`Sequence[int]`、`uint8` 1 次元 `ndarray`、または要素数 1 の構造化 `ndarray`（shape `(1,)` または 0 次元スライス `arr[i]`）を受理する。
- **`to_psv` の引数**：
  - `mv`：`Move`、`Move32`、`int`（`0..=65535`）、USI 文字列、または `None`（省略時は `Move.NONE`）。
  - `score`：`int`（`int16` 範囲のセンチポーン評価値、既定値 `0`）。
  - `game_result`：`GameResult`、`int`、結果名文字列、または `None`（省略時は `GameResult.INVALID`）。
  - `game_ply`：`int | None`（`uint16` 範囲の手数、省略時は `board.game_ply`）。
- **`to_hcpe` の引数**：
  - `best_move`：`AperyMove`、`AperyMove32`、`Move`、`Move32`、`int`（`0..=65535` の `AperyMove` 値）、USI 文字列、または `None`（省略時は `AperyMove(0)`）。渡された指し手は盤面参照なしで 16 ビットの `AperyMove` に直接変換される。
  - `score`：`int`（`int16` 範囲のセンチポーン評価値、既定値 `0`）。
  - `game_result`：必須引数（`None` は `ValueError`）。`GameResult`（引き分け系、先手勝ち系、後手勝ち系の各定数）、文字列（`"DRAW"`、`"BLACK_WIN"`、`"WHITE_WIN"`、大文字小文字不問）、または HCPE 数値コード（`0`：引き分け、`1`：先手勝ち、`2`：後手勝ち）を指定する。

> [!WARNING]
> 片玉、玉なし、または標準枚数を超える駒を含む局面は `PackedSfen` および `HuffmanCodedPos` では表現できず、変換時に `ValueError` を送出する。

```python
import numpy as np
from rsshogi.core import Board
from rsshogi.numpy import HuffmanCodedPosAndEval, PackedSfenValue
from rsshogi.record import GameResult

board = Board()

# PSV 配列への直接書き込み（0 次元スライスへの書き込みにも対応）
psv_arr = np.zeros(2, dtype=PackedSfenValue)
board.to_psv(mv="7g7f", score=50, game_result=GameResult.BLACK_WIN, out=psv_arr[0])

# HCPE 配列への直接書き込み
hcpe_arr = np.zeros(1, dtype=HuffmanCodedPosAndEval)
board.to_hcpe(best_move="7g7f", score=50, game_result="BLACK_WIN", out=hcpe_arr)
```

---

## 指し手の適用と取り消し

盤面に対して指し手を適用して手を進めるメソッドと、直前の手を取り消して戻すメソッドを提供する。

| メソッド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `apply_move(mv)` | `mv: Move` | `None` | 16 ビットの `Move` を適用して局面を進める。非合法手は `ValueError` を送出する。 |
| `apply_move32(mv)` | `mv: Move32` | `None` | 32 ビットの `Move32` を適用して局面を進める。非合法手は `ValueError` を送出する。 |
| `apply_usi(usi)` | `usi: str` | `None` | USI 文字列（例：`"7g7f"`、`"P*5e"`）を解析して適用する。 |
| `apply_csa(csa)` | `csa: str` | `None` | CSA 文字列（例：`"7776FU"`、`"+7776FU"`）を解析して適用する。 |
| `push_move(mv)` | `mv: Move` | `Move32` | `Move` を適用し、駒情報が付与された完全な `Move32` を返す。 |
| `push_move32(mv)` | `mv: Move32` | `Move32` | `Move32` を適用し、適用された完全な `Move32` を返す。 |
| `push_usi(usi)` | `usi: str` | `Move32` | USI 文字列で手を進め、適用された完全な `Move32` を返す。 |
| `push_usi_with_delta(usi)` | `usi: str` | `dict[str, Any]` | USI 文字列で手を進め、適用された指し手と盤面更新差分を辞書で返す。 |
| `undo_move(mv)` | `mv: Move` | `None` | 直前に適用した `Move` を取り消して 1 手戻す。直前の手と一致しない場合は `ValueError` を送出する。 |
| `undo_move32(mv)` | `mv: Move32` | `None` | 直前に適用した `Move32` を取り消して 1 手戻す。直前の手と一致しない場合は `ValueError` を送出する。 |
| `pop()` | なし | `Move32 \| None` | 直前の指し手を取り消して `Move32` を返す。履歴が空の場合は `None` を返す。 |
| `last_move()` | なし | `Move32 \| None` | 盤面を変更せずに直前の指し手を `Move32` で返す。履歴が空の場合は `None` を返す。 |
| `move32_from_move(mv)` | `mv: Move` | `Move32` | 盤面を変更せずに、現在局面の駒情報を補完して `Move` を完全な `Move32` に変換する。通常の盤上手または駒打ちとして解決できない場合は `ValueError` を送出する。 |
| `move_from_csa(csa)` | `csa: str` | `Move32` | 盤面を変更せずに、CSA 指し手文字列を現在局面上の完全な `Move32` に変換する。 |

### `push_usi_with_delta(usi)` の戻り値スキーマ

`push_usi_with_delta(usi)` が返す辞書は、すべての手で共通のキーと、`kind`（`"BOARD"` または `"DROP"`）に応じた固有キーを持つ。

| キー | 条件 | 値の型 | 説明 |
|---|---|---|---|
| `"move"` | 共通 | `Move32` | 適用された完全な 32 ビット指し手。 |
| `"move_usi"` | 共通 | `str` | 指し手の USI 文字列。 |
| `"kind"` | 共通 | `str` | 盤上の移動なら `"BOARD"`、駒打ちなら `"DROP"`。 |
| `"color"` | 共通 | `Color` | 着手した手番。 |
| `"to"` | 共通 | `Square` | 移動先または打ったマス。 |
| `"from"` | `"kind" == "BOARD"` | `Square` | 移動元のマス。 |
| `"moved_piece_before"` | `"kind" == "BOARD"` | `Piece` | 移動前の駒（成る前の状態）。 |
| `"moved_piece_after"` | `"kind" == "BOARD"` | `Piece` | 移動後の駒（成った場合は成駒）。 |
| `"captured"` | `"kind" == "BOARD"` | `dict \| None` | 駒を取らない手では `None`。駒を取った手では `{"piece": Piece, "hand_count_before": int}`（取った盤上駒と、取得前の手番側の同種持ち駒枚数）。 |
| `"piece_type"` | `"kind" == "DROP"` | `PieceType` | 打った駒の駒種。 |
| `"hand_count_before"` | `"kind" == "DROP"` | `int` | 駒を打つ前の、手番側の同種持ち駒枚数。 |

```python
from rsshogi.core import Board

board = Board()
delta = board.push_usi_with_delta("7g7f")
assert delta["kind"] == "BOARD"
assert delta["captured"] is None

popped = board.pop()
assert popped is not None and popped.to_usi() == "7g7f"
```

---

## 盤上の駒、持ち駒、ビットボードの参照

盤上のマスごとの駒配置、手番ごとの持ち駒、および条件別のビットボードを取得する。

| メソッド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `piece_on(square)` | `square: Square` | `Piece` | 指定したマスにある駒を返す。空きマスの場合は `Piece(0)`（`PieceType.NO_PIECE_TYPE`）を返す。`Square.NONE` は `ValueError` を送出する。 |
| `is_square_empty(square)` | `square: Square` | `bool` | 指定したマスが空であれば `True` を返す。`Square.NONE` は `ValueError` を送出する。 |
| `empties()` | なし | `Bitboard` | 盤上のすべての空きマスを表す `Bitboard` を返す。 |
| `hand(color)` | `color: Color` | `Hand` | 指定した手番の持ち駒を `Hand` オブジェクトとして返す。 |
| `hand_counts(color)` | `color: Color` | `dict[str, int]` | 指定した手番の持ち駒枚数を `"P"`, `"L"`, `"N"`, `"S"`, `"B"`, `"R"`, `"G"` の 7 キーを持つ辞書で返す。 |
| `king_square(color)` | `color: Color` | `Square` | 指定した手番の玉の位置を返す。盤上に玉が存在しない場合は `Square.NONE` を返す。 |
| `iter_pieces()` | なし | `list[tuple[Square, Piece]]` | 盤上に存在するすべての駒を `(Square, Piece)` のリストとしてマス番号の昇順（`1a`, `1b`, ..., `9i`）で返す。 |
| `pieces()` | なし | `Bitboard` | 先後両方のすべての盤上駒が存在するマスの `Bitboard` を返す。 |
| `pieces_by_type(piece_type)` | `piece_type: PieceType` | `Bitboard` | 先後両方の指定した駒種が存在するマスの `Bitboard` を返す。`PieceType.NO_PIECE_TYPE` は `ValueError` を送出する。 |
| `pieces_by_color(color)` | `color: Color` | `Bitboard` | 指定した手番のすべての盤上駒が存在するマスの `Bitboard` を返す。 |
| `pieces_for(piece_type, color)` | `piece_type: PieceType`, `color: Color` | `Bitboard` | 指定した手番かつ指定した駒種が存在するマスの `Bitboard` を返す。 |
| `golds()` | なし | `Bitboard` | 先後両方の金と同じ動きをする駒（金、と金、成香、成桂、成銀）の `Bitboard` を返す。 |
| `bishop_horse()` | なし | `Bitboard` | 先後両方の角と馬の `Bitboard` を返す。 |
| `rook_dragon()` | なし | `Bitboard` | 先後両方の飛と龍の `Bitboard` を返す。 |

```python
from rsshogi.core import Board
from rsshogi.types import Color, PieceType, Square

board = Board()
print(board.piece_on(Square.from_usi("7g")))  # => P
print(board.king_square(Color.BLACK).to_usi())  # => "5i"
print(board.pieces_for(PieceType.PAWN, Color.BLACK).count())  # => 9
print(board.hand_counts(Color.BLACK))  # => {'P': 0, 'L': 0, 'N': 0, 'S': 0, 'B': 0, 'R': 0, 'G': 0}
```

---

## 利きと攻撃の判定

特定のマスに対する攻撃駒（アタッカー）の列挙や、特定の手番と駒種による利き範囲の計算を行う。

| メソッド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `attackers_to(square, occupied)` | `square: Square`, `occupied: Bitboard` | `Bitboard` | 指定した占有ビットボード `occupied` を前提として、`square` に利いている先後両方の駒の位置を返す。 |
| `attackers_to_color(color, square, occupied)` | `color: Color`, `square: Square`, `occupied: Bitboard` | `Bitboard` | 指定した `occupied` を前提として、`square` に利いている `color` 側の駒の位置を返す。 |
| `attackers_to_color_current(color, square)` | `color: Color`, `square: Square` | `Bitboard` | 現在の盤面の占有状態を前提として、`square` に利いている `color` 側の駒の位置を返す。 |
| `attacks_by(color, piece_type)` | `color: Color`, `piece_type: PieceType` | `Bitboard` | 現在の盤面における、`color` 側の `piece_type` の駒による利き先の集合を返す。 |
| `is_attacked_by(square, by_color)` | `square: Square`, `by_color: Color` | `bool` | 現在の盤面で `square` に `by_color` 側の駒の利きが 1 つ以上あれば `True` を返す。 |

```python
from rsshogi.core import Board
from rsshogi.types import Color, PieceType, Square

board = Board()
board.apply_usi("7g7f")

target = Square.from_usi("7e")
print(board.is_attacked_by(target, Color.BLACK))  # => True
print(board.attacks_by(Color.BLACK, PieceType.BISHOP).contains(Square.from_usi("3c")))  # => True
```

---

## 合法手と擬似合法手の生成および判定

| メソッド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `legal_moves()` | なし | `list[Move]` | 現在局面の全合法手を `Move`（16 ビット）のリストとして返す。歩、香、角、飛の不成も含めて生成する。 |
| `legal_moves_move32()` | なし | `list[Move32]` | 現在局面の全合法手を駒情報付きの `Move32`（32 ビット）のリストとして返す。 |
| `pseudo_legal_moves()` | なし | `list[Move]` | 現在局面の擬似合法手（自玉の王手放置を含む可能性のある手）を `Move` のリストとして返す。 |
| `pseudo_legal_moves_move32()` | なし | `list[Move32]` | 現在局面の擬似合法手を `Move32` のリストとして返す。 |
| `is_legal_move(mv)` | `mv: Move` | `bool` | 指定した `Move` が現在局面で合法手であれば `True` を返す。 |
| `is_legal_move32(mv)` | `mv: Move32` | `bool` | 指定した `Move32` が現在局面で合法手であれば `True` を返す。 |

```python
from rsshogi.core import Board, Move

board = Board()
moves = board.legal_moves()
print(len(moves))  # => 30

candidate = Move.from_usi("7g7f")
if board.is_legal_move(candidate):
    board.apply_move(candidate)
```

---

## 対局状態、入玉宣言、1 手詰めの判定

手番、手数、Zobrist ハッシュ、王手、詰み、千日手、入玉宣言勝ち、1 手詰めの各判定を行う。

| プロパティ / メソッド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `turn` | プロパティ（読取専用） | `Color` | 現在の手番（`Color.BLACK` または `Color.WHITE`）。 |
| `game_ply` | プロパティ（読取専用） | `int` | 初期局面からの手数（平手初期局面では `1`）。 |
| `zobrist_hash()` | なし | `int` | 現在局面の 64 ビット Zobrist ハッシュ値を返す。 |
| `is_in_check()` | なし | `bool` | 現在手番の玉に王手がかかっていれば `True` を返す。 |
| `is_mated()` | なし | `bool` | 現在局面が詰み（王手がかかっており合法手が 0 件）であれば `True` を返す。 |
| `repetition_state()` | なし | `RepetitionState` | 現在局面の千日手および優劣局面判定を `RepetitionState` で返す。 |
| `is_repetition(threshold=3)` | `threshold: int = 3` | `bool` | 同一局面の出現回数が `threshold` 回以上であれば `True` を返す。 |
| `can_declare_win()` | なし | `bool` | 現在手番が入玉宣言勝ちの条件を満たしていれば `True` を返す。 |
| `evaluate_declaration()` | なし | `dict[str, Any]` | 入玉宣言勝ちの判定結果と内訳を辞書で返す。 |
| `solve_mate_in_one()` | なし | `Move32 \| None` | 現在局面に 1 手詰めが存在すればその詰め手を `Move32` で返し、存在しなければ `None` を返す。 |

### `evaluate_declaration()` の戻り値スキーマ

`evaluate_declaration()` が返す辞書は、共通キーと `detail_type` に応じた詳細キーを持つ。

| キー | 条件 | 値の型 | 説明 |
|---|---|---|---|
| `"rule"` | 共通 | `str` | 適用された入玉ルール名（`"Point27"`、`"Point24"`、`"TryRule"`、`"None"`、`"Unset"` など）。 |
| `"can_declare"` | 共通 | `bool` | 入玉宣言勝ちが可能であれば `True`。 |
| `"detail_type"` | 共通 | `str` | `"POINT_RULE"`、`"TRY_RULE"`、`"NO_RULE"` のいずれか。 |
| `"not_in_check"` | `"detail_type" == "POINT_RULE"` | `bool` | 手番側の玉に王手がかかっていないか。 |
| `"king_in_enemy_camp"` | `"detail_type" == "POINT_RULE"` | `bool` | 手番側の玉が敵陣三段目以内に入っているか。 |
| `"pieces_in_camp"` | `"detail_type" == "POINT_RULE"` | `int` | 敵陣三段目以内にある玉以外の自駒の枚数。 |
| `"enough_pieces"` | `"detail_type" == "POINT_RULE"` | `bool` | 敵陣内の玉以外の自駒が 10 枚以上あるか。 |
| `"points"` | `"detail_type" == "POINT_RULE"` | `int` | 宣言規定に基づく合計点数（大駒 5 点、小駒 1 点）。 |
| `"required_points"` | `"detail_type" == "POINT_RULE"` | `int` | 宣言に必要な点数（先手 28 点、後手 27 点など）。 |
| `"king_square"` | `"detail_type" == "TRY_RULE"` | `Square \| None` | 手番側の玉の現在位置（盤上に玉がない場合は `None`）。 |
| `"try_square"` | `"detail_type" == "TRY_RULE"` | `Square` | トライ規定の目標マス（先手なら `5a`、後手なら `5i`）。 |
| `"can_reach"` | `"detail_type" == "TRY_RULE"` | `bool` | 玉が目標マスへ移動できる位置にいるか。 |
| `"can_occupy"` | `"detail_type" == "TRY_RULE"` | `bool` | 目標マスに自駒が置かれていないか。 |
| `"is_safe"` | `"detail_type" == "TRY_RULE"` | `bool` | 目標マスに相手駒の利きがないか。 |

```python
from rsshogi.core import Board

board = Board()
info = board.evaluate_declaration()
print(info["rule"], info["can_declare"], info["points"], info["required_points"])
# => Point27 False 27 28
```

---

## `PositionState` と妥当性検証クラス

### `PositionState`

```python
PositionState(sfen: str | None = None) -> PositionState
PositionState.from_sfen(sfen: str) -> PositionState
```

`PositionState` は盤上 81 マス、先後の持ち駒、手番、手数を保持する構造化データクラスである。
`sfen=None` で生成した場合は、すべてのマスが空で持ち駒なしの空盤面（`"9/9/9/9/9/9/9/9/9 b - 1"`、`side_to_move=Color.BLACK`、`ply=1`）となる。

| プロパティ / メソッド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `side_to_move` | プロパティ（読取および書込） | `Color` | 手番（`Color.BLACK` または `Color.WHITE`）。 |
| `ply` | プロパティ（読取および書込） | `int` | 手数（`0..=65535`、範囲外の代入は `ValueError`）。 |
| `board` | プロパティ（読取専用） | `list[Piece]` | 盤上 81 マスの駒を格納した長さ 81 のリスト。 |
| `hands` | プロパティ（読取専用） | `tuple[Hand, Hand]` | `(先手の持ち駒, 後手の持ち駒)` のタプル。 |
| `piece_on(square)` | `square: Square` | `Piece` | 指定マスの駒を返す。`Square.NONE` は `ValueError` を送出する。 |
| `set_piece(square, piece)` | `square: Square`, `piece: Piece` | `None` | 指定マスに駒を配置する。空きマスにする場合は `Piece(0)` を渡す。 |
| `hand(color)` | `color: Color` | `Hand` | 指定手番の持ち駒を返す。 |
| `set_hand(color, hand)` | `color: Color`, `hand: Hand` | `None` | 指定手番の持ち駒を設定する。 |
| `to_sfen(include_ply=True)` | `include_ply: bool = True` | `str` | SFEN 文字列へ変換する。`include_ply=False` の場合は末尾の手数を省略する。 |
| `copy()` / `__copy__()` | なし | `PositionState` | `PositionState` の複製を返す。 |
| `validate()` | なし | `None` | 局面を検証し、問題があれば `ValueError` を送出する。 |
| `is_valid()` | なし | `bool` | 局面が妥当であれば `True` を返す。 |
| `validate_all()` | なし | `ValidationReport` | 検出されたすべての問題を `ValidationReport` として返す。 |
| `state == other` | `other: object` | `bool` | 盤面、持ち駒、手番、手数がすべて等しいかを判定する。 |

### `ValidationReport`

`validate_all()` が返す `ValidationReport` は以下のメンバーを持つ。

| プロパティ / メソッド | 型 | 説明 |
|---|---|---|
| `is_valid()` | `bool` | 問題が 0 件であれば `True` を返す。 |
| `issues` | `list[ValidationIssue]` | 検出された `ValidationIssue` のリスト。 |
| `len(report)` | `int` | 検出された問題の件数。 |
| `report == other` | `bool` | 保持する問題リストが等しいかを判定する。 |

### `ValidationIssue`

`ValidationIssue` は検出された個別の問題を表し、以下のプロパティを持つ。

| プロパティ | 型 | 説明 |
|---|---|---|
| `kind` | `str` | 問題種別（下表参照）。 |
| `color` | `Color \| None` | 対象の手番（`"EXCESSIVE_PIECE_COUNT"` 以外の種別で設定される）。 |
| `file_usi` | `str \| None` | 二歩が検出された筋の USI 表記（`"1"` から `"9"`、`"DOUBLE_PAWN"` のみ）。 |
| `square` | `Square \| None` | 配置違反が検出されたマス（`"INVALID_PLACEMENT"` のみ）。 |
| `piece_type` | `PieceType \| None` | 対象の駒種（`"INVALID_HAND_COUNT"`、`"EXCESSIVE_PIECE_COUNT"`、`"INVALID_PLACEMENT"` で設定される）。 |
| `count` | `int \| None` | 検出された枚数（`"INVALID_HAND_COUNT"`、`"EXCESSIVE_PIECE_COUNT"` で設定される）。 |
| `limit` | `int \| None` | 許容される上限枚数（`"EXCESSIVE_PIECE_COUNT"` のみ）。 |
| `message` | `str` | 問題内容の説明文字列（`str(issue)` と同一）。 |

`ValidationIssue.kind` の取り得る値は以下の 6 種類である。

| `kind` の値 | 発生条件 |
|---|---|
| `"NO_KING"` | 指定手番の玉が盤上に存在しない。 |
| `"TWO_KINGS"` | 指定手番の玉が盤上に 2 枚以上存在する。 |
| `"DOUBLE_PAWN"` | 同じ筋に指定手番の未成の歩が 2 枚以上存在する（二歩）。 |
| `"INVALID_HAND_COUNT"` | 指定手番の持ち駒枚数がその駒種の最大枚数を超えている。 |
| `"EXCESSIVE_PIECE_COUNT"` | 盤上と先後の持ち駒を合わせた特定駒種の総数が標準枚数（歩 18 枚、香と桂と銀と金は各 4 枚、角と飛は各 2 枚）を超えている。 |
| `"INVALID_PLACEMENT"` | 行き所のない駒（1 段目の歩と香、1 段目および 2 段目の桂）が配置されている。 |

```python
from rsshogi.core import Board, PositionState
from rsshogi.types import Color, Piece, PieceType, Square

state = PositionState()
state.set_piece(Square.from_usi("5i"), Piece.from_color_type(Color.BLACK, PieceType.KING))
state.set_piece(Square.from_usi("5a"), Piece.from_color_type(Color.WHITE, PieceType.KING))
state.set_piece(Square.from_usi("7g"), Piece.from_color_type(Color.BLACK, PieceType.PAWN))
state.set_piece(Square.from_usi("7f"), Piece.from_color_type(Color.BLACK, PieceType.PAWN))

report = state.validate_all()
assert not report.is_valid()
for issue in report.issues:
    print(issue.kind, issue.file_usi, issue.message)
# => DOUBLE_PAWN 7 Double pawn for Black on file 7

# 修正して Board に反映する
state.set_piece(Square.from_usi("7f"), Piece(0))
assert state.is_valid()
board = Board()
board.set_position_state(state)
```

## 関連項目

- [基本型（`Square` / `Piece` / `Hand` / `Bitboard`）](types.md)
- [指し手クラス（`Move` / `Move32` / `AperyMove`）](move.md)
- [SVG 出力](svg.md)
- [NumPy 連携](numpy.md)
- [棋譜管理（`Record`）](record.md)
