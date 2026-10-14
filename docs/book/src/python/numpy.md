# NumPy 連携（`rsshogi.numpy`）

`rsshogi.numpy` は機械学習や探索データの入出力向けに、YaneuraOu 系の `PackedSfen` / `PackedSfenValue`（PSV）および Apery / `cshogi` 系の `HuffmanCodedPos`（HCP）/ `HuffmanCodedPosAndEval`（HCPE）に対応する NumPy 構造化 `dtype` を提供する。

```python
from rsshogi.numpy import (
    HuffmanCodedPos,
    HuffmanCodedPosAndEval,
    PackedSfen,
    PackedSfenValue,
)
```

このモジュールをインポートするには環境に `numpy` がインストールされている必要がある。

```console
python -m pip install rsshogi numpy
```

---

## 構造化 `dtype` 一覧

| `dtype` 定数 | 1 要素のバイト数 | 対応する `Board` / `Record` メソッド | 用途 |
|---|---:|---|---|
| `PackedSfen` | 32 バイト | `Board.to_packed_sfen()` / `Board.set_packed_sfen()` | 圧縮された局面データの格納。 |
| `PackedSfenValue` | 40 バイト | `Board.to_psv()` / `Record.to_psv()` | やねうら王形式の教師データ（PSV）の読み書き。 |
| `HuffmanCodedPos` | 32 バイト | `Board.to_hcp()` / `Board.set_hcp()` | Apery / `cshogi` 互換のハフマン符号化局面の格納。 |
| `HuffmanCodedPosAndEval` | 38 バイト | `Board.to_hcpe()` | Apery / `cshogi` 互換の教師データ（HCPE）の読み書き。 |

### `PackedSfen`

| フィールド名 | NumPy 型 | オフセット | 説明 |
|---|---|---:|---|
| `sfen` | `uint8[32]` | `0..32` | 32 バイトの圧縮局面データ。 |

### `PackedSfenValue`

| フィールド名 | NumPy 型 | オフセット | 説明 |
|---|---|---:|---|
| `sfen` | `uint8[32]` | `0..32` | 32 バイトの圧縮局面データ。 |
| `score` | `int16` | `32..34` | 手番側視点の評価値（センチポーン）。 |
| `move` | `uint16` | `34..36` | 16 ビットの指し手値（`Move.value`）。 |
| `game_ply` | `uint16` | `36..38` | 初期局面からの手数。 |
| `game_result` | `int8` | `38..39` | 対局結果コード（`GameResult.value`）。 |
| `padding` | `uint8` | `39..40` | アライメント用パディング（`0`）。 |

### `HuffmanCodedPos`

| フィールド名 | NumPy 型 | オフセット | 説明 |
|---|---|---:|---|
| `hcp` | `uint8[32]` | `0..32` | 32 バイトのハフマン符号化局面データ。 |

### `HuffmanCodedPosAndEval`

| フィールド名 | NumPy 型 | オフセット | 説明 |
|---|---|---:|---|
| `hcp` | `uint8[32]` | `0..32` | 32 バイトのハフマン符号化局面データ。 |
| `eval` | `int16` | `32..34` | 手番側視点の評価値（センチポーン）。 |
| `bestMove16` | `uint16` | `34..36` | Apery 形式の 16 ビット指し手値（`AperyMove.value`）。 |
| `gameResult` | `int8` | `36..37` | 勝敗結果コード（`0`：引き分け、`1`：先手勝ち、`2`：後手勝ち）。 |
| `dummy` | `uint8` | `37..38` | アライメント用パディング（`0`）。 |

---

## 使用例

### `Board` から構造化 `ndarray` へのインプレース書き込み

`Board.to_packed_sfen()`、`to_psv()`、`to_hcp()`、`to_hcpe()` の `out` 引数には、要素数 1 の構造化 `ndarray`（shape `(1,)`）だけでなく、配列の 0 次元スライス（`arr[i]`）を直接渡すことができる。
同様に、`set_packed_sfen()` と `set_hcp()` にも `arr[i]` や `arr[i]["sfen"]`、`arr[i]["hcp"]` を直接渡せる。

```python
import numpy as np
from rsshogi.core import Board
from rsshogi.numpy import HuffmanCodedPosAndEval, PackedSfenValue
from rsshogi.record import GameResult

board = Board()
usi_moves = ["7g7f", "3c3d", "2g2f", "4c4d"]
scores = [25, -20, 35, -15]

psv_data = np.zeros(len(usi_moves), dtype=PackedSfenValue)
hcpe_data = np.zeros(len(usi_moves), dtype=HuffmanCodedPosAndEval)

for i, (usi, score) in enumerate(zip(usi_moves, scores)):
    # 0 次元スライス psv_data[i] / hcpe_data[i] へ直接書き込む
    board.to_psv(
        mv=usi,
        score=score,
        game_result=GameResult.BLACK_WIN,
        out=psv_data[i],
    )
    board.to_hcpe(
        best_move=usi,
        score=score,
        game_result=GameResult.BLACK_WIN,
        out=hcpe_data[i],
    )
    board.apply_usi(usi)

print(psv_data["score"].tolist())  # => [25, -20, 35, -15]
print(hcpe_data["eval"].tolist())  # => [25, -20, 35, -15]
```

### バイナリ教師データファイル（`.psv` / `.hcpe`）の読み書き

```python
import numpy as np
from rsshogi.core import AperyMove, Board, Move
from rsshogi.numpy import HuffmanCodedPosAndEval, PackedSfenValue

# 1. Record.to_psv() から NumPy 配列を構築してファイルへ保存する
from rsshogi.record import Record

record = Record.from_usi_main_line(
    "startpos",
    ["7g7f", "3c3d"],
    evals=[30, -25],
    result="BLACK_WIN",
)
psv_bytes = b"".join(record.to_psv())
psv_array = np.frombuffer(psv_bytes, dtype=PackedSfenValue)
psv_array.tofile("train.psv")

# 2. 保存した PSV ファイルを読み込んで盤面と指し手を復元する
loaded_psv = np.fromfile("train.psv", dtype=PackedSfenValue)
board = Board()
board.set_packed_sfen(loaded_psv[0])
mv = Move(int(loaded_psv[0]["move"]))
print(board.to_sfen(), mv.to_usi(), int(loaded_psv[0]["score"]))

# 3. HCPE 要素から盤面と AperyMove を復元する
hcpe_array = np.zeros(1, dtype=HuffmanCodedPosAndEval)
board.to_hcpe(best_move="7g7f", score=40, game_result=1, out=hcpe_array[0])

board.set_hcp(hcpe_array[0], game_ply=1)
apery_mv = AperyMove(int(hcpe_array[0]["bestMove16"]))
print(apery_mv.to_usi())  # => "7g7f"
```

## 関連項目

- [盤面管理（`Board`）](board.md)
- [指し手クラス（`Move` / `AperyMove`）](move.md)
- [棋譜管理（`Record`）](record.md)
- [棋譜フォーマット一覧](../reference/formats/index.md)
