# 定跡 (Book)

定跡（Opening Book）の読み込み・構築・検索を行うためのクラスと関数です。

```python
from rsshogi.book import StaticBook, MemoryBook, BookBuilder
from rsshogi.book import book_key_from_position, book_key_after
```

rsshogi の定跡には次の 2 系統があります。

- **rsshogi 独自のバイナリ定跡**: `StaticBook`（読み取り専用）/ `MemoryBook`（編集可能）。
  このページで扱います。バイナリのフォーマット仕様は
  [定跡バイナリ形式](../reference/formats/book.md) を参照してください。
- **外部定跡**: DB2016 / SBK。読み取り専用で参照します。
  [外部定跡（DB2016 / SBK）](external-books.md) を参照してください。

## クラス

### BookKey

定跡検索に使用する局面のハッシュキー。
default build では 64bit、`hash-128` feature build では 128bit です。

#### プロパティ

| 名前 | 型 | 説明 |
|------|-----|------|
| `low` | `int` | キーの下位64ビット |
| `high` | `int` | キーの上位64ビット（128ビットハッシュの場合のみ非ゼロ） |

#### メソッド

- `__int__()` → `int`: 128ビット整数（`high << 64 | low`）として返す
- `__repr__()` → `str`: デバッグ用の文字列表現
- `__eq__(other)` → `bool`: 等価比較
- `__hash__()` → `int`: ハッシュ値（辞書のキーとして使用可能）

### BookMove

定跡に登録された1手の情報。

#### プロパティ

| 名前 | 型 | 説明 |
|------|-----|------|
| `mv` | `Move` | 指し手（16ビット表現） |
| `score` | `int` | 評価値（センチポーン） |
| `depth` | `int` | 探索深さ |

#### 例

```python
import rsshogi as rs

book = rs.book.StaticBook.from_file("opening.bin")
board = rs.core.Board()
key = rs.book.book_key_from_position(board)
entry = book.get(key)

if entry:
    for book_move in entry.moves:
        print(f"{book_move.mv.to_usi()} (score={book_move.score}, depth={book_move.depth})")
```

### BookEntry

定跡の1局面に対する候補手の集合。

#### プロパティ

| 名前 | 型 | 説明 |
|------|-----|------|
| `key` | `BookKey` | 局面のハッシュキー |
| `moves` | `list[BookMove]` | 候補手のリスト |

#### メソッド

- `__len__()` → `int`: 候補手の数を返す

### MemoryBook

メモリ上で構築・編集可能な定跡データベース。

#### コンストラクタ

```python
book = MemoryBook()
```

#### メソッド

| メソッド | 説明 |
|---------|------|
| `insert_move(key: BookKey, book_move: BookMove)` | 指定キーに1手を追加 |
| `get(key: BookKey) -> BookEntry \| None` | 指定キーの定跡エントリーを取得 |
| `contains(key: BookKey) -> bool` | 指定キーが存在するか確認 |
| `__len__() -> int` | 登録されている局面数を返す |

#### 例

```python
import rsshogi as rs

# 空の定跡を作成
book = rs.book.MemoryBook()

# 初期局面のキーを取得
board = rs.core.Board()
key = rs.book.book_key_from_position(board)

# 候補手を追加
mv = rs.core.Move.from_usi("7g7f")
book_move = rs.book.BookMove(mv=mv, score=100, depth=10)
book.insert_move(key, book_move)

# 静的定跡に変換
static_book = rs.book.StaticBook.from_memory_book(book)
static_book.write_file("my_book.bin")
```

### StaticBook

読み取り専用のバイナリ定跡ファイル。
`from_bytes()` / `from_file()` は現在の build の `BookKey` 幅と一致するファイルだけを読み込みます。
default build では 64bit、`hash-128` feature build では 128bit の static book が対象です。

#### クラスメソッド

| メソッド | 説明 |
|---------|------|
| `from_bytes(data: bytes) -> StaticBook` | バイト列から定跡を読み込む |
| `from_memory_book(book: MemoryBook) -> StaticBook` | MemoryBookから変換 |
| `from_file(path: str) -> StaticBook` | ファイルから定跡を読み込む |

#### インスタンスメソッド

| メソッド | 説明 |
|---------|------|
| `to_bytes() -> bytes` | バイト列に変換 |
| `write_file(path: str)` | ファイルに書き出す |
| `get(key: BookKey) -> BookEntry \| None` | 指定キーの定跡エントリーを取得 |
| `contains(key: BookKey) -> bool` | 指定キーが存在するか確認 |
| `__len__() -> int` | 登録されている局面数を返す |

#### 例

```python
import rsshogi as rs

# 定跡ファイルを読み込む
book = rs.book.StaticBook.from_file("opening.bin")
print(f"定跡局面数: {len(book)}")

# 初期局面の候補手を取得
board = rs.core.Board()
key = rs.book.book_key_from_position(board)
entry = book.get(key)

if entry:
    print(f"候補手数: {len(entry.moves)}")
    for book_move in entry.moves:
        print(f"  {book_move.mv.to_usi()}: score={book_move.score}")
```

### BookBuilder

棋譜から定跡を構築するためのヘルパークラス。

#### コンストラクタ

```python
builder = BookBuilder()
```

#### メソッド

| メソッド | 説明 |
|---------|------|
| `insert(key: BookKey, mv: Move, score: int, depth: int)` | 指定キーに1手を追加 |
| `insert_for_position(board: Board, mv: Move, score: int, depth: int)` | 局面から自動的にキーを計算して1手を追加 |
| `extend_from_game_record(record: Record, default_score: int = 0, default_depth: int = 0)` | 棋譜から手順を取り込む（変化手順も含む） |
| `into_memory() -> MemoryBook` | MemoryBookとして取り出す |
| `build_static() -> StaticBook` | StaticBookへ変換する |

#### 例

```python
import rsshogi as rs

# 棋譜を読み込む
record = rs.record.Record.from_kif_file("game.kif")

# 定跡を構築
builder = rs.book.BookBuilder()
builder.extend_from_game_record(record, default_score=0, default_depth=1)

# MemoryBookとして取り出す
memory_book = builder.into_memory()
```

## 関数

### book_key_from_position

現在の局面から定跡検索用のキーを計算します。

```python
def book_key_from_position(board: Board) -> BookKey
```

#### 引数

- `board`: 盤面

#### 戻り値

- `BookKey`: 局面のハッシュキー

#### 例

```python
import rsshogi as rs

board = rs.core.Board()
board.apply_usi("7g7f")
key = rs.book.book_key_from_position(board)
```

### book_key_after

1手進めた後の局面のキーを計算します。

```python
def book_key_after(board: Board, mv: Move) -> BookKey
```

#### 引数

- `board`: 現在の盤面
- `mv`: 指す手

#### 戻り値

- `BookKey`: 1手進めた後の局面のハッシュキー

#### 例

```python
import rsshogi as rs

board = rs.core.Board()
mv = rs.core.Move.from_usi("7g7f")
key_after = rs.book.book_key_after(board, mv)
```

### book_builder_from_game_record

棋譜から定跡を構築します（ヘルパー関数）。

```python
def book_builder_from_game_record(
    record: Record,
    default_score: int = 0,
    default_depth: int = 0
) -> MemoryBook
```

#### 引数

- `record`: 棋譜レコード
- `default_score`: デフォルト評価値（評価値がない手に使用）
- `default_depth`: デフォルト深さ（深さがない手に使用）

#### 戻り値

- `MemoryBook`: 構築された定跡（メモリ上）

#### 例

```python
import rsshogi as rs

# 複数の棋譜から定跡を構築（BookBuilder を使用）
builder = rs.book.BookBuilder()

for kif_file in ["game1.kif", "game2.kif", "game3.kif"]:
    record = rs.record.Record.from_kif_file(kif_file)
    builder.extend_from_game_record(record, default_score=0, default_depth=1)

# 静的定跡に変換して保存
static_book = builder.build_static()
static_book.write_file("combined.bin")
```

1 棋譜から手軽に定跡を作る場合は `book_builder_from_game_record()` を使用できます。

```python
import rsshogi as rs

record = rs.record.Record.from_kif_file("game.kif")
memory_book = rs.book.book_builder_from_game_record(record, default_score=0, default_depth=1)
static_book = rs.book.StaticBook.from_memory_book(memory_book)
static_book.write_file("opening.bin")
```

## 定跡の使い方

### 定跡ファイルの読み込みと検索

```python
import rsshogi as rs

# 定跡を読み込む
book = rs.book.StaticBook.from_file("opening.bin")

# 盤面を初期化
board = rs.core.Board()

# 数手進める
board.apply_usi("7g7f")
board.apply_usi("3c3d")

# 現在局面の定跡を検索
key = rs.book.book_key_from_position(board)
entry = book.get(key)

if entry:
    print(f"定跡に{len(entry.moves)}手の候補があります")
    for book_move in entry.moves:
        move_usi = book_move.mv.to_usi()
        print(f"  {move_usi}: 評価値={book_move.score}, 深さ={book_move.depth}")
else:
    print("定跡に登録されていません")
```

### 棋譜から定跡を作成

```python
import rsshogi as rs

# 棋譜を読み込む
record = rs.record.Record.from_kif_file("professional_game.kif")

# 定跡を構築
memory_book = rs.book.book_builder_from_game_record(
    record,
    default_score=0,
    default_depth=1
)

# 静的定跡に変換して保存
static_book = rs.book.StaticBook.from_memory_book(memory_book)
static_book.write_file("my_opening.bin")
print(f"定跡を作成しました: {len(static_book)}局面")
```

### 手動で定跡を構築

```python
import rsshogi as rs

# 空の定跡を作成
book = rs.book.MemoryBook()
board = rs.core.Board()

# 初期局面の候補手を追加
key = rs.book.book_key_from_position(board)

# 7六歩を追加
move1 = rs.core.Move.from_usi("7g7f")
book_move1 = rs.book.BookMove(mv=move1, score=50, depth=10)
book.insert_move(key, book_move1)

# 2六歩を追加
move2 = rs.core.Move.from_usi("2g2f")
book_move2 = rs.book.BookMove(mv=move2, score=45, depth=10)
book.insert_move(key, book_move2)

# 確認
entry = book.get(key)
print(f"{len(entry.moves)}手の候補を登録しました")
```

## 外部定跡

DB2016 や SBK などのエンジン向け外部定跡ファイルを読み込む場合は、
`YaneuraOuBook` / `SbkBook` クラスを使用します。詳細は [外部定跡](external-books.md) を参照してください。

```python
import rsshogi as rs

# DB2016
book = rs.book.YaneuraOuBook.open("book.db")

# SBK
book = rs.book.SbkBook.open("book.sbk")
```

## 関連項目

- [定跡バイナリ形式](../reference/formats/book.md) - `StaticBook` / `MemoryBook` が使うバイナリ定跡のフォーマット仕様
- [外部定跡（DB2016 / SBK）](external-books.md) - 外部定跡ファイルの読み込み
- [Board](board.md) - 盤面クラス
- [Move](move.md) - 16bit 指し手型
- [Record](record.md) - 棋譜レコード
