# よくある質問（FAQ）

## 基本

### rsshogi とは？

Rust 実装の将棋ライブラリです。
Python パッケージの `rsshogi` からも、盤面管理、指し手生成、棋譜処理を利用できます。

### どのパッケージを入れるべき？

- 通常版：`python -m pip install rsshogi`
- AVX2 対応の x86_64 CPU 用：`python -m pip install rsshogi-avx2`

同じ環境には一方だけをインストールします。
切り替える手順は [インストール](../getting-started/installation.md) を参照してください。

## API

### 手番の型は？

`Board.turn` の戻り値は `Color` です。

### 指し手を進めてから戻すには？

- `push_move(move)`：`Move` を受け取り指し手を進める。戻り値は `Move32`。
- `push_move32(move32)`：`Move32` を受け取り指し手を進める。戻り値は `Move32`。
- `push_usi(usi)`：USI 文字列で指し手を進める。戻り値は `Move32`。
- `pop()`：最後の手を取り消す。戻り値は `Move32`（手がなければ `None`）。
- `last_move()`：最後の手を `Move32` で返す（なければ `None`）。

### 入玉宣言判定はできる？

`board.can_declare_win()` が使えます。

### 局面キー（Zobrist hash）は取得できる？

`board.zobrist_hash()` で取得できます。


### `Move` から `Move32` を復元したい

`board.move32_from_move(mv)` を使います。
現在局面から移動する駒の情報を補うため、指し手を適用する前に呼び出してください。
CSA 形式の文字列から作る場合は `board.move_from_csa(csa)` を使います。

### `AperyMove32` から `Move32` を復元したい

`AperyMove32.to_move32(board)` を使ってください。

`AperyMove32` は `PieceType` しか持たず、`rsshogi.Move32` は色付きの `Piece` を持つため、
復元には現在局面 `board` が必要です。

### PackedSfen を既存のバッファに書き出すには？

`board.to_packed_sfen(out=buffer)` を使います。
`out=None` なら `bytes` を返し、`out` 指定時はバッファに書き込んで `None` を返します。
`set_packed_sfen()` は `bytes` だけでなく `numpy.ndarray`（`uint8[32]` / `PackedSfen` 構造体）も受け取れます。

### HCP や HCPE を既存のバッファに書き出すには？

`board.to_hcp(out=buffer)` または `board.to_hcpe(game_result=result, out=buffer)` を使います。
`to_hcpe()` の第 1 引数は `best_move` なので、出力先は `out=` で指定してください。
`out=None` なら `bytes` を返し、`out` 指定時はバッファに書き込んで `None` を返します。

- `to_hcp()`: `uint8[32]` または `HuffmanCodedPos`
- `to_hcpe()`: `uint8[38]` または `HuffmanCodedPosAndEval`

`to_hcpe()` は `game_result` の指定が必須です。

### `Record` を USI 指し手列から作りたい

`Record.from_usi_main_line(init_position_sfen, usi_moves, result=..., evals=..., ...)` が使えます。
USI 文字列の指し手列を渡すと合法性を検証しながら `Record` を構築します。

### `SpecialMoveEntry` を結果から簡単に作りたい

`SpecialMoveEntry.from_result(result, time_ms=..., comment=...)` が使えます。
`GameResult` を渡すと種別 (`RESIGN` / `WIN` など) を自動判定して `SpecialMoveEntry` を構築します。

## 例外

### `TypeError` と `ValueError` の使い分けは？

- 型不一致: `TypeError`
- 値の不正（不正手・不正文字列）: `ValueError`
- 履歴が空など「値がない」ケース: `None` 返却

## 棋譜・バイナリ形式

### KI2 は読み込める？

`Record.from_ki2_str()` で読み込みできます。`to_ki2()` で書き出しも可能です。

## トラブルシューティング

### `ValueError: illegal move` が出る

指し手が現在局面で合法ではありません。`board.is_legal_move(move)` で事前確認してください。

### `ModuleNotFoundError: No module named 'rsshogi'`

実行中の Python 環境からパッケージを見つけられていません。
その環境で `python -m pip install rsshogi` を実行してください。
インストール済みの場合は、仮想環境や Notebook のカーネルが実行環境と一致しているか確認してください。
