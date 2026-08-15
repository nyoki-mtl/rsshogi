# rsshogi Python バインディング

`rsshogi` Python パッケージは Rust core の薄いバインディングで、棋譜、定跡、protocol text、policy labels、学習データ向けの Python value objects と helpers を加えます。
Python 3.10 以降が必要です。

```console
python -m pip install rsshogi
```

AVX2 対応 x86_64 CPU では `rsshogi-avx2` も選択できます。
通常版とAVX2版を同じ環境へインストールしないでください。

```python
from rsshogi.core import Board, Move

board = Board()
board.apply_move(Move.from_usi("7g7f"))
assert board.turn.name == "WHITE"
```

## モジュール

- `core`: `Board`、`Move`、`Move32`、`AperyMove`、`AperyMove32`、局面検証、USI-position の解析・正規化。
- `types`: `Color`、`Square`、`PieceType`、`Piece`、`Bitboard`、`Hand`、`MoveType`、`RepetitionState`。
- `record`: typed game records、棋譜編集、KIF/KI2/CSA/JKF/USI-position conversion、PACK、SBINPACK。
- `book`: memory/static books と DB2016、YBB-backed、SBK-facing book APIs。
- `policy`: full/compact move-label conversion、`numpy`: packed-position dtypes、`sazpack`: SAZ2 self-play records、`svg`: board rendering、`usi`: stateless USI protocol value objects、`initial_positions`: named starts。

解析できない入力、value type として無効な値、wire format に違反する入力は Python 例外を発生させます。
`Board.legal_moves()` は合法手だけを含み、順序は契約ではありません。
`Move32` metadata は受け手 API が必要とする場合だけ保持してください。

HCP、PackedSfen、PACK、HCPE、YBB、SBK、Zobrist key は 1.1.1 の表現を維持します。
PACK は駒打ち・成りを含む AperyMove layout を使用し、`Record.from_pack()` / `to_pack()` と `decode_pack_file()` から利用できます。
1.1.1 から更新する場合は [移行ガイド](https://nyoki-mtl.github.io/rsshogi/migration.html) を確認してください。
