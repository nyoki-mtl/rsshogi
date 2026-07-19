# よくある質問（FAQ）

## 基本

### rsshogi とは？

Rust 実装の将棋ライブラリです。Python バインディング（`rsshogi`）から盤面管理・指し手生成・棋譜処理を利用できます。

### どのパッケージを入れるべき？

- 推奨: `pip install rsshogi`
- x86_64 で AVX2 を使う場合: `pip install rsshogi-avx2`

## API

### 手番の型は？

`Board.turn` の戻り値は `Color` です。

### `push` / `pop` はある？

あります。以下を利用できます。

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

- `board.move32_from_move(mv)`
- `board.move_from_csa(csa)`

を利用してください（線形探索不要）。

### `AperyMove32` から `Move32` を復元したい

`AperyMove32.to_move32(board)` を使ってください。

`AperyMove32` は `PieceType` しか持たず、`rsshogi.Move32` は色付きの `Piece` を持つため、
復元には現在局面 `board` が必要です。

### `Board.to_packed_sfen(out)` は使える？

使えます。`out=None` なら `bytes` を返し、`out` 指定時はバッファへ書き込みます。
`set_packed_sfen()` は `bytes` だけでなく `numpy.ndarray`（`uint8[32]` / `PackedSfen` 構造体）も受け取れます。

### `Board.to_hcp(out)` / `Board.to_hcpe(out)` は使える？

使えます。`out=None` なら `bytes` を返し、`out` 指定時は既存バッファへ書き込みます。

- `to_hcp()`: `uint8[32]` または `HuffmanCodedPos`
- `to_hcpe()`: `uint8[38]` または `HuffmanCodedPosAndEval`

`to_hcpe()` は `game_result` の指定が必須です。

### `Record` を USI 指し手列から作りたい

v0.10.3 以降、`Record.from_usi_main_line(init_position_sfen, usi_moves, result=..., evals=..., ...)` が使えます。
USI 文字列の指し手列を渡すと合法性を検証しながら `Record` を構築します。

### `SpecialMoveEntry` を結果から簡単に作りたい

v0.10.3 以降、`SpecialMoveEntry.from_result(result, time_ms=..., comment=...)` が使えます。
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

パッケージ未導入です。`pip install rsshogi` を実行してください。
