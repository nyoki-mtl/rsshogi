# Python API

## インストール

Python 3.10 以降が必要です。
通常は portable build をインストールします。

```console
python -m pip install "rsshogi==1.2.0"
```

AVX2 対応 x86_64 CPU だけを対象にする環境では、同じ API を持つ最適化 build を選べます。

```console
python -m pip install "rsshogi-avx2==1.2.0"
```

両 package は同じ `rsshogi` module を提供するため、同じ Python environment へ同時にインストールしないでください。
CPU 対応を確実に判定できない場合や arm64 では通常版を使います。

```python
import rsshogi

assert rsshogi.__version__ == "1.2.0"
```

## 局面を操作する

公開値は機能別 submodule から import します。

```python
from rsshogi.core import Board, Move

board = Board()
move = Move.from_usi("7g7f")
assert move in board.legal_moves()

board.apply_move(move)
assert board.turn.name == "WHITE"
board.undo_move(move)
```

`Board` は SFEN 入出力、局面更新・巻き戻し、合法手と pseudo-legal 手、千日手、詰み、駒・持ち駒、局面検証、packed position 変換を提供します。
`Board.legal_moves()` と `legal_moves_move32()` は完全な合法手集合を返しますが、順序は未規定です。
test や永続出力で順序が必要なら、たとえば `move.value` や `move.to_usi()` を key に呼び出し側で sort します。

`Move`、`Move32`、`AperyMove`、`AperyMove32` は別の型です。
PACK や Apery 形式の raw 値を通常の `Move` raw 値として解釈せず、公開された変換 method を使います。

```python
from rsshogi.core import Move

null = Move.from_usi("0000")
assert null.to_usi() == "null"
```

USI `0000` は null move の入力表記として受理され、正規出力は `null` です。

## 棋譜

`rsshogi.record.Record` は KIF、KI2、CSA、JKF、USI-position、PACK、SBINPACK の共通表現です。

```python
from rsshogi.record import Record

record = Record.from_usi_main_line("startpos", ["7g7f", "3c3d"])
assert record.to_usi_position() == "position startpos moves 7g7f 3c3d"

kif = record.to_kif()
round_tripped = Record.from_kif_str(kif)
```

file API では encoding を明示できます。
KIF と KI2 の既定は UTF-8 です。
CSA は `Record.to_csa(version="2.2")` と `version="3.0"` を選べます。
`Record.to_dict()` / `Record.from_dict(strict=True)` は typed な構造化 interchange に使います。

PACK は 1 game ずつなら `Record.from_pack()` / `Record.to_pack()`、連結データなら `decode_pack()` / `decode_pack_file()` と `write_pack()` / `write_pack_file()` を使います。
駒打ちと成りは AperyMove の 16-bit layout で保持されます。
`Record.to_pack()` には各指し手の評価値と terminal が必要で、評価値は `-32000..=32000` へ clamp されます。
切り詰め、無効な手、非合法手、勝敗と終了理由の矛盾は例外になります。

## 定跡

`rsshogi.book` は次の用途を分けています。

- `MemoryBook` / `StaticBook`: rsshogi native book の構築と高速 lookup。
- `YaneuraOuBook`: DB2016 text book の lookup と検証。
- `YbbBook`: YBB binary book の lookup。
- `SbkBook`: SBK の lookup と graph traversal。

```python
from rsshogi.book import YaneuraOuBook
from rsshogi.core import Board

book = YaneuraOuBook.open("book.db")
book.validate_full()
entry = book.lookup_position(Board())
if entry is not None:
    for candidate in entry.moves:
        print(candidate.mv.to_usi(), candidate.score, candidate.count)
```

DB2016 の既定 open は先頭部分の並びを検査します。
大規模 book に binary lookup する前は `validate_full()` を一度実行し、未整列の book は `lookup_sfen_by_scan()` を使います。
Rust reader は entry iterator を持ちますが、Python API は lookup を公開します。
reader は book 全体を Python object graph へ展開しません。

## 学習データと補助 module

| Module | 用途 |
| --- | --- |
| `rsshogi.policy` | `Move`、`Move32`、raw 値、USI 文字列と full/compact policy label の相互変換。 |
| `rsshogi.sazpack` | typed SAZ2 self-play game の読み書き。 |
| `rsshogi.numpy` | PackedSfen、HCP、HCPE、PackedSfenValue などの NumPy dtype。 |
| `rsshogi.svg` | notebook 表示にも使える盤面 SVG。 |
| `rsshogi.usi` | `info` / `bestmove` の解析と `go` command の整形。engine process controller ではない。 |
| `rsshogi.initial_positions` | 名前付きの平手・駒落ち初期局面。 |

## 例外と入力境界

無効な USI、raw 値、SFEN、wire data は `ValueError` などの Python 例外になります。
外部ファイルを trust せず、変換単位で例外を処理してください。
NumPy の view を渡す API では dtype と byte length も契約の一部です。
