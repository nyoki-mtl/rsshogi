# GameResult クラス

```python
rsshogi.record.GameResult
```

終局結果を表す列挙型です。18 種類の終局結果が定義されており、
勝敗判定のためのメソッドも提供しています。

```python
from rsshogi.record import GameResult

# 定数として使用
if result == GameResult.BLACK_WIN:
    print("先手の勝ち")
```

## クラス定数

18種類の終局結果が定義されています。

### 勝敗

| 定数 | 値 | 説明 |
|------|-----|------|
| `BLACK_WIN` | 0 | 先手の勝ち |
| `WHITE_WIN` | 1 | 後手の勝ち |

### 引き分け

| 定数 | 値 | 説明 |
|------|-----|------|
| `DRAW_BY_REPETITION` | 2 | 千日手による引き分け |
| `DRAW_BY_MAX_PLIES` | 6 | 最大手数による引き分け |
| `DRAW_BY_IMPASSE` | 10 | 持将棋（点数による引き分け） |

### 宣言勝ち

| 定数 | 値 | 説明 |
|------|-----|------|
| `BLACK_WIN_BY_DECLARATION` | 4 | 先手の入玉宣言勝ち |
| `WHITE_WIN_BY_DECLARATION` | 5 | 後手の入玉宣言勝ち |

### 没収勝ち・反則勝ち

| 定数 | 値 | 説明 |
|------|-----|------|
| `BLACK_WIN_BY_FORFEIT` | 8 | 先手の不戦勝 |
| `WHITE_WIN_BY_FORFEIT` | 9 | 後手の不戦勝 |
| `BLACK_WIN_BY_ILLEGAL_MOVE` | 12 | 後手の反則負け（先手勝ち） |
| `WHITE_WIN_BY_ILLEGAL_MOVE` | 13 | 先手の反則負け（後手勝ち） |
| `BLACK_WIN_BY_TIMEOUT` | 16 | 後手の時間切れ（先手勝ち） |
| `WHITE_WIN_BY_TIMEOUT` | 17 | 先手の時間切れ（後手勝ち） |
| `BLACK_WIN_BY_TRY_RULE` | 20 | 先手のトライルール勝ち |
| `WHITE_WIN_BY_TRY_RULE` | 21 | 後手のトライルール勝ち |

### その他

| 定数 | 値 | 説明 |
|------|-----|------|
| `ERROR` | 3 | エラー |
| `INVALID` | 7 | 無効な結果 |
| `PAUSED` | 11 | 中断 |

---

## コンストラクタ

```python
GameResult(value: int) -> GameResult
```

内部値から `GameResult` を生成します。

**引数:**
- `value`: 上記定数と同等の整数値（0-21、ただし 14・15・18・19 は未使用）

**例外:**
- `ValueError`: 不正な値を渡した場合

---

## プロパティ

| プロパティ | 型 | 説明 |
|------------|-----|------|
| `value` | `int` | 内部値（u8） |
| `name` | `str` | 定数名（例: `"BLACK_WIN"`） |

---

## メソッド

### 勝敗判定

| メソッド | 戻り値 | 説明 |
|----------|--------|------|
| `is_black_win()` | `bool` | 先手勝ちか（宣言勝ち・反則勝ち・時間勝ちを含む） |
| `is_white_win()` | `bool` | 後手勝ちか（宣言勝ち・反則勝ち・時間勝ちを含む） |
| `is_win()` | `bool` | 勝敗が付いたか（引き分け・中断以外） |
| `is_draw()` | `bool` | 引き分けか |
| `is_win_by_declaration()` | `bool` | 入玉宣言勝ちか |

### 名前解決

| メソッド | 戻り値 | 説明 |
|----------|--------|------|
| `GameResult.from_str(name)` | `GameResult` | 定数名から `GameResult` を生成（大文字・小文字は区別しない） |

### 特殊メソッド

| メソッド | 説明 |
|----------|------|
| `int(result)` | 内部値を返す |
| `str(result)` | 定数名を返す |
| `hash(result)` | ハッシュ値を返す（`dict`/`set` のキーに利用可能） |
| `result == other` | 比較（`GameResult` または `int` と比較可能） |

---

## 使用例

### 基本的な使い方

```python
import rsshogi as rs

# 定数として使用
result = rs.record.GameResult.BLACK_WIN
print(result.name)   # => "BLACK_WIN"
print(int(result))   # => 0

# 勝敗判定
if result.is_win():
    if result.is_black_win():
        print("先手の勝ち")
    else:
        print("後手の勝ち")
elif result.is_draw():
    print("引き分け")
```

### 棋譜の結果を判定

```python
import rsshogi as rs

record = rs.record.Record.from_kif_str(kif_text)
result = record.result

# 詳細な結果を表示
print(f"結果: {result.name}")

if result == rs.record.GameResult.BLACK_WIN:
    print("先手の勝ち（通常）")
elif result == rs.record.GameResult.WHITE_WIN:
    print("後手の勝ち（通常）")
elif result.is_win_by_declaration():
    winner = "先手" if result.is_black_win() else "後手"
    print(f"{winner}の入玉宣言勝ち")
elif result == rs.record.GameResult.DRAW_BY_REPETITION:
    print("千日手による引き分け")
elif result == rs.record.GameResult.BLACK_WIN_BY_TIMEOUT:
    print("後手の時間切れで先手の勝ち")
elif result == rs.record.GameResult.WHITE_WIN_BY_TIMEOUT:
    print("先手の時間切れで後手の勝ち")
```

### 結果の種類ごとにカウント

```python
import rsshogi as rs
from collections import Counter

results = []
for kif_text in kif_files:
    record = rs.record.Record.from_kif_str(kif_text)
    results.append(record.result.name)

# 集計
counter = Counter(results)
for name, count in counter.most_common():
    print(f"{name}: {count}局")
```

### 数値からの変換

```python
import rsshogi as rs

# 数値から GameResult を生成
result = rs.record.GameResult(0)  # BLACK_WIN
print(result.name)  # => "BLACK_WIN"

# 比較
assert result == rs.record.GameResult.BLACK_WIN
assert result == 0  # int との比較も可能
```

### 名前からの変換と `__members__`

```python
import rsshogi as rs

result = rs.record.GameResult.from_str("black_win")
assert result == rs.record.GameResult.BLACK_WIN

members = rs.record.GameResult.__members__
assert members["BLACK_WIN"] == rs.record.GameResult.BLACK_WIN
```

### `dict` / `set` のキーとして使う

```python
import rsshogi as rs

result = rs.record.GameResult.BLACK_WIN
stats = {result: 1}
assert stats[rs.record.GameResult(0)] == 1
assert rs.record.GameResult(0) in {result}
```

---

## 定数の一覧表

| 定数 | 値 | `is_black_win()` | `is_white_win()` | `is_draw()` | `is_win()` |
|------|-----|------------------|------------------|-------------|------------|
| `BLACK_WIN` | 0 | ✓ | | | ✓ |
| `WHITE_WIN` | 1 | | ✓ | | ✓ |
| `DRAW_BY_REPETITION` | 2 | | | ✓ | |
| `ERROR` | 3 | | | | |
| `BLACK_WIN_BY_DECLARATION` | 4 | ✓ | | | ✓ |
| `WHITE_WIN_BY_DECLARATION` | 5 | | ✓ | | ✓ |
| `DRAW_BY_MAX_PLIES` | 6 | | | ✓ | |
| `INVALID` | 7 | | | | |
| `BLACK_WIN_BY_FORFEIT` | 8 | ✓ | | | ✓ |
| `WHITE_WIN_BY_FORFEIT` | 9 | | ✓ | | ✓ |
| `DRAW_BY_IMPASSE` | 10 | | | ✓ | |
| `PAUSED` | 11 | | | | |
| `BLACK_WIN_BY_ILLEGAL_MOVE` | 12 | ✓ | | | ✓ |
| `WHITE_WIN_BY_ILLEGAL_MOVE` | 13 | | ✓ | | ✓ |
| `BLACK_WIN_BY_TIMEOUT` | 16 | ✓ | | | ✓ |
| `WHITE_WIN_BY_TIMEOUT` | 17 | | ✓ | | ✓ |
| `BLACK_WIN_BY_TRY_RULE` | 20 | ✓ | | | ✓ |
| `WHITE_WIN_BY_TRY_RULE` | 21 | | ✓ | | ✓ |

---

## 関連項目

- [`Record`](record.md) - 棋譜レコード
- [`Board`](board.md) - 盤面管理クラス
