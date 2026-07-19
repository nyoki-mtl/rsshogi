# NumPy 連携

```python
rsshogi.numpy
```

NumPy と連携するための dtype 定義を提供します。
学習データの効率的な処理に使用できます。

```python
from rsshogi.numpy import (
    PackedSfen,
    PackedSfenValue,
    HuffmanCodedPos,
    HuffmanCodedPosAndEval,
)
```

**注意:** NumPy がインストールされていない場合、このモジュールは利用できません。

## dtype 一覧

### PackedSfen

圧縮された局面データ（32 バイト）を格納する dtype です。

```python
import numpy as np
from rsshogi.numpy import PackedSfen

# 配列の作成
data = np.zeros(100, dtype=PackedSfen)
print(data.dtype)  # => dtype([('sfen', 'u1', (32,))])
```

#### フィールド

| フィールド | 型 | サイズ | 説明 |
|------------|-----|--------|------|
| `sfen` | `uint8` | 32 | 圧縮された局面データ |

### PackedSfenValue

局面データと評価値などの付加情報を含む dtype です。
学習データとして使用されます。

```python
import numpy as np
from rsshogi.numpy import PackedSfenValue

# 配列の作成
data = np.zeros(100, dtype=PackedSfenValue)
print(data.dtype)
```

#### フィールド

| フィールド | 型 | サイズ | 説明 |
|------------|-----|--------|------|
| `sfen` | `uint8` | 32 | 圧縮された局面データ |
| `score` | `int16` | 1 | 評価値（センチポーン） |
| `move` | `uint16` | 1 | 指し手（16bit エンコーディング） |
| `game_ply` | `uint16` | 1 | 手数 |
| `game_result` | `int8` | 1 | 対局結果 |
| `padding` | `uint8` | 1 | パディング |

### HuffmanCodedPos

`cshogi` / Apery 互換の HCP 局面（32 バイト）を格納する dtype です。

```python
import numpy as np
from rsshogi.numpy import HuffmanCodedPos

data = np.zeros(100, dtype=HuffmanCodedPos)
print(data.dtype)  # => dtype([('hcp', 'u1', (32,))])
```

| フィールド | 型 | サイズ | 説明 |
|------------|-----|--------|------|
| `hcp` | `uint8` | 32 | HuffmanCodedPos バイト列 |

### HuffmanCodedPosAndEval

`hcpe` 学習データ 1 件分（38 バイト）を格納する dtype です。

```python
import numpy as np
from rsshogi.numpy import HuffmanCodedPosAndEval

data = np.zeros(100, dtype=HuffmanCodedPosAndEval)
print(data.dtype)
```

| フィールド | 型 | サイズ | 説明 |
|------------|-----|--------|------|
| `hcp` | `uint8` | 32 | HuffmanCodedPos バイト列 |
| `eval` | `int16` | 1 | 評価値 |
| `bestMove16` | `uint16` | 1 | Apery 16bit 指し手 |
| `gameResult` | `int8` | 1 | `Draw` / `BlackWin` / `WhiteWin` |
| `dummy` | `uint8` | 1 | パディング |

## 使用例

### Board との連携

```python
import numpy as np
from rsshogi.core import Board
from rsshogi.numpy import PackedSfen

board = Board()
board.apply_usi("7g7f")

# 局面を PackedSfen 形式で取得
psfen_bytes = board.to_packed_sfen()

# NumPy 配列として格納
data = np.zeros(1, dtype=PackedSfen)
data[0]['sfen'] = np.frombuffer(psfen_bytes, dtype=np.uint8)
```

### 学習データの読み込み

```python
import numpy as np
from rsshogi.numpy import PackedSfenValue

# バイナリファイルから読み込み
with open("training_data.bin", "rb") as f:
    data = np.frombuffer(f.read(), dtype=PackedSfenValue)

print(f"データ数: {len(data)}")
print(f"平均評価値: {data['score'].mean():.1f}")
```

### hcpe の読み込み

```python
import numpy as np
from rsshogi.core import Board
from rsshogi.numpy import HuffmanCodedPosAndEval

entries = np.fromfile("teacher.hcpe", dtype=HuffmanCodedPosAndEval)

board = Board()
board.set_hcp(entries[0]["hcp"])
print(entries[0]["bestMove16"])
print(entries[0]["eval"])
```

### 学習データの作成

```python
import numpy as np
from rsshogi.core import Board
from rsshogi.numpy import PackedSfenValue

# 棋譜から学習データを作成
data = np.zeros(100, dtype=PackedSfenValue)

board = Board()
for i, usi_move in enumerate(usi_moves[:100]):
    # 局面を保存
    psfen = board.to_packed_sfen()
    data[i]['sfen'] = np.frombuffer(psfen, dtype=np.uint8)
    data[i]['score'] = eval_scores[i]
    data[i]['move'] = move_values[i]
    data[i]['game_ply'] = i + 1
    data[i]['game_result'] = result
    
    board.apply_usi(usi_move)

# ファイルに保存
data.tofile("training_data.bin")
```

### hcpe の作成

```python
import numpy as np
from rsshogi.core import Board
from rsshogi.numpy import HuffmanCodedPosAndEval

board = Board()
data = np.zeros(1, dtype=HuffmanCodedPosAndEval)
board.to_hcpe(best_move="7g7f", score=120, game_result="BLACK_WIN", out=data)
data.tofile("teacher.hcpe")
```

## 関連項目

- [`Board`](board.md) - 盤面管理クラス（`to_packed_sfen()`, `to_hcp()`, `to_psv()`, `to_hcpe()`, `set_packed_sfen()`, `set_hcp()`）
- [`Apery / hcpe 互換`](apery.md) - `AperyMove` / `AperyMove32` と HCP/HCPE の概要
- [`Record`](record.md) - 棋譜レコード（`to_sbinpack()`, `from_sbinpack()`, `to_psv()`）
