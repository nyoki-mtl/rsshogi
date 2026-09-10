# rsshogi Python バインディング

`rsshogi` Python パッケージは Rust core の薄いバインディングです。
盤面操作、棋譜、定跡、USI の値オブジェクト、policy ラベル、学習データ形式を Python から扱えます。
Python 3.10 以降が必要です。

```console
python -m pip install rsshogi
```

AVX2 対応 x86_64 CPU では `rsshogi-avx2` も選択できます。
どちらも `rsshogi` として読み込むため、環境ごとに一方をインストールします。

```python
from rsshogi.core import Board, Move

board = Board()
board.apply_move(Move.from_usi("7g7f"))
assert board.turn.name == "WHITE"
```

## モジュール

- `core`: `Board`、`Move`、`Move32`、`AperyMove`、`AperyMove32`、局面検証、USI-position の解析・正規化。
- `types`: `Color`、`Square`、`PieceType`、`Piece`、`Bitboard`、`Hand`、`MoveType`、`RepetitionState`。
- `record`: 型付き棋譜、棋譜編集、KIF/KI2/CSA/JKF/USI position、PACK、SBINPACK。
- `book`: メモリ定跡、静的定跡、DB2016、SBK の参照 API。
- `policy`: 通常ラベルと compact ラベルの相互変換。
- `numpy`: Packed SFEN、HCP、HCPE の NumPy dtype。
- `sazpack`: SAZ2 自己対局データ。
- `svg`: 盤面 SVG 描画。
- `usi`: ステートレスな USI プロトコルの値オブジェクト。
- `initial_positions`: 平手と駒落ちの初期局面。

解析できない入力、型として受理できない値、形式に違反するバイト列では Python 例外を送出します。
`Board.legal_moves()` は合法手だけを含み、順序は契約ではありません。
`Move32` metadata は受け手 API が必要とする場合だけ保持してください。

HCP、Packed SFEN、PACK、HCPE、SBK、Zobrist key には、それぞれの形式に対応した API を使用します。
PACK は Apery の 16bit 指し手表現を使います。
単一局は `Record.from_pack()` / `to_pack()`、複数局は `decode_pack()` / `decode_pack_file()` で扱えます。
