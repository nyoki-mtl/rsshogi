# 学習用バイナリ仕様（HCP / HCPE / PackedSfen / PSV / PACK）

`rsshogi` は、将棋 AI の教師あり学習、強化学習、および NNUE 学習パイプラインで使用される固定長および可変長のバイナリフォーマット群をサポートしている。
本ページでは、6 種類の主要な学習用バイナリ形式の比較と、`HCP`、`HCPE`、`PackedSfen`、`PackedSfenValue`（PSV）、および `PACK` のワイヤー仕様を解説する。

## 1. 学習用バイナリフォーマットの比較

| フォーマット | 1 要素のサイズ | 単位 | 指し手表現 | 互換系統と主な用途 |
| :--- | :--- | :--- | :--- | :--- |
| `HuffmanCodedPos` (HCP) | 32 バイト固定 | 1 局面 | なし | Apery / cshogi / dlshogi 互換のハフマン符号化局面。 |
| `HuffmanCodedPosAndEval` (HCPE) | 38 バイト固定 | 1 局面 | `AperyMove` (`u16`) | Apery / cshogi / dlshogi 互換の教師データ（局面 + 評価値 + 最善手 + 勝敗）。 |
| `PackedSfen` | 32 バイト固定 | 1 局面 | なし | YaneuraOu 互換のビットパック局面。 |
| `PackedSfenValue` (PSV) | 40 バイト固定 | 1 局面 | `Move` (`u16`) | YaneuraOu 互換の NNUE 教師データ（局面 + 評価値 + 指し手 + 手数 + 勝敗）。 |
| `PACK` | 可変長 | 1 対局 | `AperyMove` (`u16`) | ScriptCollection 互換の対局圧縮形式（開始局面 + 手順と評価値 + 終局結果）。 |
| [`SBINPACK` (`SBN2`)](../sbinpack.md) | 可変長（チャンク / チェーン） | 連続局面列 | 合法手インデックス (ULEB128) | Stockfish binpack の将棋版。合法手順序と評価値差分による高圧縮ストリーム。 |
| [`SAZPACK` (`SAZ2` v4)](../sazpack.md) | 可変長（チャンク） | 1 対局 + 局面列 | `Move` (`u16`) | AlphaZero 型 MCTS 自己対局の WDL、探索統計、疎な方策（policy）分布の保存。 |

> [!IMPORTANT]
> `HuffmanCodedPos`（32 バイト）と `PackedSfen`（32 バイト）はバイト長が同じであるが、ビット配置とハフマン符号表が完全に異なる別形式である。一方のバイト列を他方のデコーダに入力することはできない。
> 各 32 バイト局面形式のビット単位の符号化アルゴリズムについては、[局面圧縮の仕組み（HCP / PackedSfen）](../../internals/serialization/compression.md) を参照。

## 2. `HuffmanCodedPos` (32B) と `HuffmanCodedPosAndEval` (38B)

Apery、cshogi、および dlshogi 系の学習パイプラインで使われる固定長フォーマットである。
盤上に先手玉と後手玉が 1 枚ずつ存在し、各駒種の枚数が標準の初期枚数以下である局面を対象とする。

### `HuffmanCodedPosAndEval` (HCPE) のワイヤーレイアウト（38 バイト）

すべてのマルチバイト整数はリトルエンディアン（LE）である。

| オフセット | サイズ | 型 | フィールド名 | 内容 |
| :--- | :--- | :--- | :--- | :--- |
| `0x00` | 32B | `[u8; 32]` | `hcp` | `HuffmanCodedPos`（32 バイトのハフマン符号化局面）。 |
| `0x20` | 2B | `i16` | `eval` | 手番側から見た評価値（単位：cp）。 |
| `0x22` | 2B | `u16` | `bestMove16` | **Apery 形式の 16-bit 指し手**（`AperyMove::raw()`）。`rsshogi::Move` とはビット配置が異なる点に注意。 |
| `0x24` | 1B | `u8` | `gameResult` | 対局結果（`0`：引き分け、`1`：先手勝ち、`2`：後手勝ち）。 |
| `0x25` | 1B | `u8` | `padding` | パディング（`0`）。 |

### Python `Board.to_hcpe()` の引数仕様

`Board.to_hcpe(best_move, score, game_result, out=None)` は、利便性のため `best_move` と `game_result` に複数の型を受け付ける。
いずれの型で渡した場合も、出力される `bestMove16` フィールドには自動的に **Apery 形式の 16-bit 値** が書き込まれる。

- **`best_move` に指定可能な型**：
  - `AperyMove` または `AperyMove32`
  - `Move` または `Move32`（内部で `to_apery()` 相当のビット変換が行われる）
  - `int`（Apery 16-bit 指し手の raw 整数値として扱われる）
  - `str`（`"7g7f"` などの USI 指し手文字列）
- **`score` に指定可能な型**：`int`（`i16` の範囲 `-32768..=32767`）。
- **`game_result` に指定可能な型**：
  - `GameResult` 列挙値：`GameResult.BLACK_WIN`、`GameResult.WHITE_WIN`、`GameResult.DRAW_BY_REPETITION`、`GameResult.DRAW_BY_MAX_PLIES`、`GameResult.DRAW_BY_IMPASSE`、`GameResult.DRAW_BY_ADJUDICATION`。
  - `int`：`0`（引き分け）、`1`（先手勝ち）、`2`（後手勝ち）
  - `str`：`"DRAW"`、`"BLACK_WIN"`、`"WHITE_WIN"`（大文字と小文字を区別しない）

## 3. `PackedSfen` (32B) と `PackedSfenValue` (40B)

YaneuraOu 系のエンジンや NNUE 学習器で使われる固定長フォーマットである。

### `PackedSfenValue` (PSV) のワイヤーレイアウト（40 バイト）

すべてのマルチバイト整数はリトルエンディアン（LE）である。

| オフセット | サイズ | 型 | フィールド名 | 内容 |
| :--- | :--- | :--- | :--- | :--- |
| `0x00` | 32B | `[u8; 32]` | `sfen` | `PackedSfen`（32 バイトのビットパック局面）。 |
| `0x20` | 2B | `i16` | `score` | 手番側から見た評価値（単位：cp）。 |
| `0x22` | 2B | `u16` | `move` | **YaneuraOu 互換の 16-bit 指し手**（`rsshogi::Move::raw()`）。HCPE とは異なり `Move` の raw 値がそのまま格納される。 |
| `0x24` | 2B | `u16` | `gamePly` | 局面の手数（ply）。 |
| `0x26` | 1B | `i8` | `game_result` | **手番側から見た勝敗結果**（`1`：手番側の勝ち、`0`：引き分け、`-1`：手番側の負け）。 |
| `0x27` | 1B | `u8` | `padding` | パディング（`0`）。 |

> [!WARNING]
> HCPE の `gameResult` が絶対視点（`1` = 先手勝ち、`2` = 後手勝ち、`0` = 引き分け）であるのに対し、PSV の `game_result` は **その局面の手番側から見た相対結果（`1` = 勝ち、`0` = 引き分け、`-1` = 負け）** である。また、指し手フィールドも HCPE は `AperyMove`、PSV は `rsshogi::Move`（YaneuraOu `Move16`）を使用する。

## 4. `PACK` フォーマット仕様

`PACK`（`rsshogi::records::formats::pack`）は、ScriptCollection（`pack2hcpe.py` 等）で使われる対局単位の可変長バイナリフォーマットである。
複数局を格納する場合は、ファイルヘッダを持たず、1 局分のバイト列をそのまま連結するストリーム構造をとる。

### 1 局分のワイヤーレイアウト

```text
+-----------------------------------------------------------------------+
| StartPosition                                                         |
|   start_tag == 1 : 平手初期局面 (1 byte)                              |
|   start_tag == 0 : 0u8 (1B) + HuffmanCodedPos (32B) + game_ply (u16)  |
+-----------------------------------------------------------------------+
| Plies (4 bytes * 手数)                                                |
|   mv: AperyMove (u16 LE) + eval: i16 LE                               |
+-----------------------------------------------------------------------+
| Terminal (3 bytes)                                                    |
|   result_marker: u16 LE + end_reason: u8                              |
+-----------------------------------------------------------------------+
```

| セクション | フィールド | 型 | 内容と検証規則 |
| :--- | :--- | :--- | :--- |
| `StartPosition` | `start_tag` | `u8` | `1` の場合は平手初期局面（追加バイトなし）。`0` の場合は直後に `hcp: [u8; 32]` と `game_ply: u16`（LE）の計 34 バイトが続く。`0` と `1` 以外は `PackError::InvalidStartState`。 |
| `Plies` | `mv` | `u16` LE | `AperyMove` の 16-bit 値。終局マーカー値（`0x0000`, `0x0081`, `0x0102`）が現れるまで繰り返す。 |
| `Plies` | `eval` | `i16` LE | その手を指す直前の手番側から見た評価値（cp）。 |
| `Terminal` | `result_marker` | `u16` LE | 勝敗結果を表す 2 バイトのマーカー。引き分け（`0`）は `0x0000`、先手勝ち（`1`）は `0x0081`、後手勝ち（`2`）は `0x0102`（`v \| (v << 7)`）。 |
| `Terminal` | `end_reason` | `u8` | 終局理由コード（`0`：投了、`1`：千日手、`2`：最大手数、`3`：中断、`4`：切れ負け、`5`：反則、`6`：連続王手の千日手、`10`：24 点法宣言勝ち、`11`：24 点法持将棋、`12`：27 点法宣言勝ち、`13`：トライルール勝ち）。 |

### `PACK` における `AperyMove` ビット配置と制約

`PACK` 内の指し手 `mv` は `AperyMove` の raw `u16` で記録される。

- **ビット `0..=6`（7 bits）**：移動先マス番号 `to`（`0..=80`）。
- **ビット `7..=13`（7 bits）**：盤上移動のときは移動元マス番号 `from`（`0..=80`、`from != to`）。駒打ちのときは `81..=87`（順に歩、香、桂、銀、金、角、飛）。
- **ビット `14`（1 bit）**：成りフラグ（駒打ちのときは必ず `0`）。
- **ビット `15`（1 bit）**：常に `0`。

`Record` から `PACK` へ変換する際（`Record.to_pack()` / `game_from_record`）には、以下の条件がすべて満たされている必要がある。

1. 本手順のすべての指し手に `EngineInfo.eval` が設定されていること（未設定の手がある場合は `PackError::MissingEval`）。なお、評価値は `Eval::CP_MIN..=Eval::CP_MAX`（`-32000..=32000`）にクランプして書き出される。
2. 本手順に終局特殊手（`main_terminal`）が存在し、勝敗または引分結果が確定していること（欠落時は `PackError::MissingTerminal`）。
3. 勝敗結果（`PackGameResult`）と終局理由（`PackEndReason`）の組み合わせが整合していること（例えば千日手なのに先手勝ちとなっているデータは `PackError::InconsistentOutcome` として拒否される）。

## 5. コード例

### Python（NumPy 連携および PACK 変換）

```python
import numpy as np
from rsshogi.core import Board
from rsshogi.record import EngineInfo, GameResult, MoveEntry, Record, SpecialMoveEntry
from rsshogi.numpy import HuffmanCodedPos, HuffmanCodedPosAndEval, PackedSfenValue
from rsshogi.record import decode_pack, write_pack

board = Board()

# 1. NumPy 構造化配列への HCPE / PSV 書き出し
hcpe_arr = np.zeros(1, dtype=HuffmanCodedPosAndEval)
board.to_hcpe(best_move="7g7f", score=120, game_result=GameResult.BLACK_WIN, out=hcpe_arr)

psv_arr = np.zeros(1, dtype=PackedSfenValue)
board.to_psv(score=120, mv="7g7f", game_ply=1, game_result=1, out=psv_arr)

# 2. Record と PACK バイナリの相互変換
record = Record.from_main_line(
    board.to_sfen(),
    [
        MoveEntry("7g7f", engine_info=EngineInfo(eval=100)),
        MoveEntry("3c3d", engine_info=EngineInfo(eval=-80)),
    ],
    SpecialMoveEntry("RESIGN", GameResult.WHITE_WIN),
)
pack_bytes = record.to_pack()
restored = Record.from_pack(pack_bytes)

# 複数局の PACK 連結と一括デコード
multi_bytes = write_pack([record, restored])
records = decode_pack(multi_bytes)
```

### Rust

```rust
use rsshogi::board::{self, HuffmanCodedPosAndEval, PackedSfenValue};
use rsshogi::records::formats::pack::{game_from_record, record_from_game, encode_game, decode_game};
use rsshogi::types::{GameResult, Move};

board::init();
let pos = board::hirate_position();
let mv = Move::from_usi("7g7f").unwrap();

// HCPE (38 bytes) と PSV (40 bytes) の生成
let hcpe = HuffmanCodedPosAndEval::from_position(&pos, mv.to_apery(), 120, GameResult::BlackWin).unwrap();
let psv = PackedSfenValue::from_position(&pos, 120, mv, 1);

// PACK のエンコードとデコード
let pack_game = game_from_record(&record)?;
let bytes = encode_game(&pack_game);
let (decoded_game, consumed_len) = decode_game(&bytes)?;
let restored_record = record_from_game(&decoded_game)?;
```

## 関連項目

- [局面圧縮の仕組み（HCP / PackedSfen）](../../internals/serialization/compression.md)
- [sbinpack フォーマット仕様](../sbinpack.md)
- [sazpack フォーマット仕様](../sazpack.md)
- [cshogi との API 対応表](../compat.md)
