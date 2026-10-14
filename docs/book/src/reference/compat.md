# cshogi との API 対応表

`rsshogi` と `cshogi` の機能対応、指し手ビットレイアウトの違い、および移行時に注意すべき API 差分をまとめる。

## 1. 機能カテゴリ別の対応一覧

| カテゴリ | 対応状況 | `rsshogi` の主な API | 参照ページ |
| :--- | :--- | :--- | :--- |
| 盤面管理と着手 | ○ | `Board`, `push_move`, `push_move32`, `push_usi`, `pop`, `last_move` | [Board API](../python/board.md) |
| 合法手生成 | ○ | `Board.legal_moves()`, `Board.pseudo_legal_moves()` | [Board API](../python/board.md) |
| 終局と宣言判定 | ○ | `is_in_check()`, `is_mated()`, `is_repetition()`, `can_declare_win()` | [Board API](../python/board.md) |
| SFEN / USI 変換 | ○ | `to_sfen()`, `set_sfen()`, `apply_usi()`, `Move.from_usi()`, `Move.to_usi()` | [Move API](../python/move.md) |
| 局面特徴量出力 | ○ | `Board.to_position_state()`, `PositionState` | [Board API](../python/board.md) |
| 局面圧縮と学習データ | ○ | `to_packed_sfen()`, `set_packed_sfen()`, `to_psv()`, `to_hcp()`, `set_hcp()`, `to_hcpe()` | [学習用バイナリ仕様](formats/training-binaries.md) |
| Apery 互換指し手 | ○ | `AperyMove`, `AperyMove32`, `Move.to_apery()`, `Board.apery_move32_from_move()` | [Move API](../python/move.md) |
| Policy ラベル変換 | ○ | `rsshogi.policy.move_label()`, `compact_move_label()` | [Policy ラベル](../internals/types/policy-labels.md) |
| 棋譜入出力 | ○ | `Record.from_kif_str()`, `from_ki2_str()`, `from_csa_str()`, `from_jkf_str()`, `to_kif()`, `to_ki2()`, `to_csa()`, `to_jkf()` | [Record API](../python/record.md) |
| USI プロトコル解析 | ○ | `rsshogi.usi.parse_info()`, `parse_bestmove()`, `UsiGoCommand` | [USI パーサ](../python/usi.md) |

## 2. 16-bit 指し手（`Move` vs `AperyMove`）のビットレイアウトの違い

`cshogi` から `rsshogi` へ移行する際、最も重要な違いは **16-bit 指し手の内部ビット表現** である。

- **`rsshogi.Move`（YaneuraOu `Move16` 互換）**：ビット 14 が駒打ちフラグ、ビット 15 が成りフラグである。`StaticBook`、`PackedSfenValue`（PSV）、`sbinpack`、`sazpack`、`YbbBook` はすべてこの表現を使用する。
- **`rsshogi.AperyMove`（Apery / `cshogi.move16` 互換）**：移動元フィールド `from` に `81..=87` を格納して駒打ちを表し、ビット 14 が成りフラグである。`HuffmanCodedPosAndEval`（HCPE）および `PACK` 形式はこの表現を使用する。

| ビット領域 | `rsshogi.Move`（YaneuraOu 方式） | `rsshogi.AperyMove`（Apery / cshogi 方式） |
| :--- | :--- | :--- |
| bits `0..=6` (7 bits) | 移動先マス番号 `to` (`0..=80`) | 移動先マス番号 `to` (`0..=80`) |
| bits `7..=13` (7 bits) | 移動元マス番号 `from` (`0..=80`)、または打つ駒種 `PieceType` (`1..=7`) | 移動元マス番号 `from` (`0..=80`)、または駒打ちコード (`81..=87`) |
| bit `14` (1 bit) | **駒打ちフラグ**（`1` のとき駒打ち） | **成りフラグ**（`1` のとき成り。駒打ち時は常に `0`） |
| bit `15` (1 bit) | **成りフラグ**（`1` のとき成り） | 常に `0` |

> [!WARNING]
> `rsshogi.Move` の raw 整数値（`int(move)`）と `cshogi` の 16-bit 指し手整数値は、成り手と駒打ち手で異なる値になる。Apery / cshogi 系のバイナリデータと整数値をやり取りする際は、必ず `move.to_apery()` および `AperyMove(raw).to_move()` を介して変換すること。

### 指し手型の命名対応

| ビット幅 | `rsshogi` の型名 | YaneuraOu の型名 | `cshogi` / Apery の型名 |
| :--- | :--- | :--- | :--- |
| 16-bit | `Move` / `AperyMove` | `Move16` | `Move16`（`move16(m)`） |
| 32-bit | `Move32` / `AperyMove32` | `Move` | `Move`（デフォルト指し手整数） |

### `AperyMove32` と `Move32` の相互変換に `Board` が必要な理由

`AperyMove32` は先後を持たない `PieceType`（移動前の駒種と捕獲駒種）を保持するのに対し、`rsshogi.Move32` は手番色付きの移動後 `Piece` を保持する。
そのため、`Move32.to_apery(board)` および `AperyMove32.to_move32(board)` は、その手を指す直前の `Board` を引数に取る。

## 3. `Board` と `Move` / `Move32` の主要メソッド対応

### 盤面操作と状態取得

| 操作内容 | `cshogi` | `rsshogi` | 備考 |
| :--- | :--- | :--- | :--- |
| 手番の取得 | `board.turn` (`int`) | `board.turn` (`Color`) | `Color.BLACK` (`0`) / `Color.WHITE` (`1`)。 |
| 合法手の列挙 | `board.legal_moves` | `board.legal_moves()` | `rsshogi` ではメソッド呼び出し。 |
| 1 手進める（指し手） | `board.push(move)` | `board.push_move(move)` / `board.push_move32(mv32)` | 適用した手を `Move32` として返す。 |
| 1 手進める（USI） | `board.push_usi("7g7f")` | `board.push_usi("7g7f")` | 適用した手を `Move32` として返す。 |
| 1 手戻す | `board.pop()` | `board.pop()` | 取り消した手を `Move32`（履歴が空なら `None`）で返す。 |
| 直前の手を取得 | `board.history[-1]` | `board.last_move()` | 直前の手を `Move32`（履歴が空なら `None`）で返す。 |
| Zobrist ハッシュ | `board.key()` | `board.zobrist_hash()` | 64-bit 整数を返す。 |
| 入玉宣言勝ち判定 | `board.is_nyugyoku()` | `board.can_declare_win()` | 宣言勝ち条件を満たすかを真偽値で返す。 |
| 局面特徴量スナップショット | （個別配列操作） | `board.to_position_state()` | 盤面、持ち駒、王手情報等をまとめた `PositionState` を返す。 |

### 学習データ出力（HCP / HCPE / PackedSfen / PSV）

| 操作内容 | `cshogi` | `rsshogi` | 備考 |
| :--- | :--- | :--- | :--- |
| 32B HCP 出力 | `board.to_hcp(df)` | `board.to_hcp(out=None)` | `out=None` で `bytes` を返し、NumPy 配列指定時はインプレース書き込み。 |
| 38B HCPE 出力 | （手動構築） | `board.to_hcpe(best_move, score, game_result, out=None)` | `best_move` には `Move`, `AperyMove`, `str`, `int` を指定可能。 |
| 32B PackedSfen 出力 | （非対応） | `board.to_packed_sfen(out=None)` | YaneuraOu 互換の 32 バイト局面圧縮。 |
| 40B PSV 出力 | （非対応） | `board.to_psv(score, move, game_ply=None, game_result=0, out=None)` | YaneuraOu 互換の 40 バイト教師データ出力。 |

### CSA 表記（`Move32.to_csa()`）と KI2 表記（`Move.to_ki2(board)`）の違い

指し手を文字列フォーマットへ変換する際、CSA 表記と KI2 表記では必要となる情報が異なる。

- **CSA 表記（`Move32.to_csa()`）**：CSA 文字列（例：`+7776FU`、`+8822UM`）の生成には、手番、移動元、移動先、および移動後の駒種があれば十分であり、盤面上の他駒との曖昧性解消は不要である。32-bit の `Move32` は移動後の色付き駒種を保持しているため、`mv32.to_csa()` は `Board` 引数なしで単独で CSA 文字列を生成できる（16-bit の `Move` から作る場合は `board.move32_from_move(mv).to_csa()` を使う）。
- **KI2 表記（`Move.to_ki2(board)` / `Move32.to_ki2(board)`）**：KI2 文字列（例：`▲７六歩`、`▲５八金右`）の生成には、同じマスへ移動可能な同種駒の有無（相対動作修飾子 `右`、`左`、`上`、`寄`、`引`、`打` の判定）や直前手との移動先一致（`同` の判定）が必要である。そのため、`Move.to_ki2(board)` と `Move32.to_ki2(board)` はいずれも指し手適用前の `Board` を引数に取る。

```python
from rsshogi import Board, Move

board = Board()
mv = Move.from_usi("7g7f")

# KI2 表記は盤面上の合法手集合と照合するため board を渡す
print(mv.to_ki2(board))  # "▲７六歩"

# Move32 に変換すれば単独で CSA 表記を取得できる
mv32 = board.move32_from_move(mv)
print(mv32.to_csa())     # "+7776FU"
```

## 4. Policy ラベル変換（`rsshogi.policy`）

dlshogi / cshogi 系で使われる 27 方向 $\times$ 81 マス（計 2187 クラス）の指し手ラベルと、構造的に出現し得ないクラスを除いた 1496 クラスの圧縮ラベルを提供する。

| 関数 | 入力 | 出力 | 説明 |
| :--- | :--- | :--- | :--- |
| `rsshogi.policy.move_label(mv, turn)` | `Move` / `Move32` / `str` / `int`, `Color` | `int` (`0..2187`) | `direction * 81 + to_sq`（後手番は盤面を 180 度回転して先手視点化）。 |
| `rsshogi.policy.compact_move_label(mv, turn)` | `Move` / `Move32` / `str` / `int`, `Color` | `int` (`0..1496`) | 構造的に有効な 1496 手種への圧縮ラベル。 |
| `rsshogi.policy.move_label_to_compact(label)` | `int` (`0..2187`) | `int` (`0..1496`) | 2187 クラスから 1496 クラスへの変換。 |
| `rsshogi.policy.compact_move_label_to_move_label(compact)` | `int` (`0..1496`) | `int` (`0..2187`) | 1496 クラスから 2187 クラスへの逆変換。 |

## 関連項目

- [Board API（Python）](../python/board.md)
- [Move API（Python）](../python/move.md)
- [Record API（Python）](../python/record.md)
- [USI パーサ（Python）](../python/usi.md)
