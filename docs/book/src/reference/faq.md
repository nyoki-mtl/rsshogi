# よくある質問（FAQ）

## 1. 導入とビルド設定

### `rsshogi` とは何か？

Rust で実装された高速な将棋コアライブラリ（`rsshogi` クレート）と、その機能を Python から利用するための薄い PyO3 バインディング（`rsshogi` パッケージ）である。
盤面管理、ビットボードによる合法手生成、千日手および入玉宣言判定、棋譜（KIF / KI2 / CSA / JKF）の読み書き、定跡検索、ならびに機械学習用バイナリ（HCP / HCPE / PackedSfen / PSV / PACK / SBINPACK / SAZPACK）の入出力を提供する。

### Python パッケージは `rsshogi` と `rsshogi-avx2` のどちらを入れるべきか？

- **通常環境（ARM64 や汎用 x86_64）**：`python -m pip install rsshogi` を使用する。
- **AVX2 / BMI2 対応の x86_64 CPU 環境**：`python -m pip install rsshogi-avx2` を使用すると、飛び駒の利き計算やビットボード演算が高速化される。

両者は同じ `rsshogi` モジュール名を提供するため、同一の仮想環境にはどちらか一方のみをインストールする。
切り替え手順の詳細は [インストール](../getting-started/installation.md) を参照。

### Rust クレート（`rsshogi`）の Cargo feature はどう構成されているか？

Rust クレート `rsshogi` は **デフォルトで最小限のエンジンコアのみを有効にする設計（`default = []`）** となっている。
棋譜の読み書き、定跡、局面シリアライズ、Policy ラベル、SVG 出力などの機能を使用する場合は、`Cargo.toml` の `features` で明示的に有効化する。

| Cargo feature 名 | 有効化される機能と依存関係 |
| :--- | :--- |
| （指定なし：`default = []`） | 盤面（`Board` / `Position`）、ビットボード、合法手生成、Zobrist ハッシュ、千日手および入玉宣言判定の最小コア。 |
| `records` | `Record`、KIF / KI2 / CSA / JKF / USI / SFEN の読み書き、および `pack` / `sbinpack` / `sazpack`（`position-serialization`, `initial-positions`, `policy-labels` を含む）。 |
| `book` | `NativeBook`、`NativeBookEditor`、`YaneuraOuBook`、`YbbBook`、`SbkBook`、`BookDatabase`。 |
| `position-serialization` | `HuffmanCodedPos` (HCP)、`HuffmanCodedPosAndEval` (HCPE)、`PackedSfen`、`PackedSfenValue` (PSV)。 |
| `policy-labels` | `rsshogi::labels::policy`（2187 クラス `MoveLabel` および 1496 クラス `CompactMoveLabel`）。 |
| `initial-positions` | 平手および駒落ち定型初期局面の SFEN / `Handicap` サポート。 |
| `svg` | 盤面の SVG 描画出力。 |
| `validation` | 局面の詳細な整合性検査ユーティリティ。 |
| `hash-128` | `ZobristKey` を 64-bit から 128-bit に拡張する。 |
| `python-data` | Python バインディング相当の全データ機能（`book`, `records`, `position-serialization`, `svg`, `validation`）を一括で有効化する。 |

```toml
[dependencies]
rsshogi = { version = "0.1", features = ["records", "book"] }
```

## 2. 盤面と指し手 API

### 手番（`Board.turn`）の型は何か？

`Board.turn` プロパティは `Color` 列挙型（`Color.BLACK` または `Color.WHITE`）を返す。
整数比較が必要な場合は `int(board.turn)` で先手が `0`、後手が `1` に変換される。

### 指し手を進めてから 1 手戻すにはどうするか？

- **`board.push_move(move)`**：16-bit の `Move` を適用して局面を進め、適用した手を `Move32` で返す。
- **`board.push_move32(move32)`**：32-bit の `Move32` を適用して局面を進め、適用した手を `Move32` で返す。
- **`board.push_usi("7g7f")`**：USI 文字列の指し手を適用して局面を進め、適用した手を `Move32` で返す。
- **`board.pop()`**：直前の 1 手を取り消して局面を戻し、取り消した手を `Move32`（履歴が空なら `None`）で返す。
- **`board.last_move()`**：局面を戻さずに直前の手を `Move32`（履歴が空なら `None`）で参照する。

### 16-bit の `Move` から 32-bit の `Move32` を復元するには？

指し手を適用する前の局面 `board` に対して `board.move32_from_move(mv)` を呼び出す。
`Move32` の移動後の色付き駒（`Piece`）を補うため、現在局面の盤上駒情報を参照する必要がある。
捕獲駒は `Move32` 自体には含めず、局面の状態スタックが保持する。
CSA 文字列（例：`"+7776FU"`）から直接変換する場合は `board.move_from_csa("+7776FU")` または `Move32.from_csa("+7776FU")` を使用する。

### `AperyMove32` から `Move32` を復元するには？

指し手を適用する前の局面 `board` を渡して `apery_move32.to_move32(board)` を呼び出す。
`AperyMove32` は先後を持たない `PieceType` のみを保持し、`Move32` は色付きの `Piece` を保持するため、手番と盤上駒の参照に `board` が必要となる。

## 3. 棋譜と学習用バイナリ形式

### `PackedSfen`、`HCP`、`HCPE`、`PSV` を既存の NumPy 配列やバッファへ直接書き出すには？

各メソッドのキーワード引数 `out` に書き込み可能なバッファ（`bytearray`、`memoryview`、または対応するサイズの `numpy.ndarray`）を指定する。
`out=None`（既定）のときは新しい `bytes` オブジェクトを返し、`out` を指定したときはバッファへ直接書き込んで `None` を返す。

- **`board.to_packed_sfen(out=buf)`**：32 バイト（`uint8[32]` または `rsshogi.numpy.PackedSfen`）。
- **`board.to_psv(score, move, game_ply=None, game_result=0, out=buf)`**：40 バイト（`uint8[40]` または `rsshogi.numpy.PackedSfenValue`）。
- **`board.to_hcp(out=buf)`**：32 バイト（`uint8[32]` または `rsshogi.numpy.HuffmanCodedPos`）。
- **`board.to_hcpe(best_move, score, game_result, out=buf)`**：38 バイト（`uint8[38]` または `rsshogi.numpy.HuffmanCodedPosAndEval`）。

### USI 指し手リストから簡単に `Record` を作るには？

`Record.from_usi_main_line(init_position_sfen, usi_moves, result=..., evals=...)` を使用する。
初期局面 SFEN と USI 指し手文字列のリストを渡すと、各手の合法性を検証しながら `Record` を一括構築できる。

### `GameResult` から終局特殊手（`SpecialMoveEntry`）を自動生成するには？

`SpecialMoveEntry.from_result(result, time_ms=None, comment=None)` を使用する。
`GameResult.BLACK_WIN` や `GameResult.DRAW_BY_REPETITION` などを渡すと、対応する `SpecialMove`（投了、千日手、入玉宣言勝ちなど）を自動選択して `SpecialMoveEntry` を生成する。

## 4. 例外とトラブルシューティング

### Python API における `TypeError`、`ValueError`、`None` の使い分けは？

- **`TypeError`**：引数の型が一致しない場合に送出される。
- **`ValueError`**：SFEN や棋譜の構文エラー、非合法手の適用、バイナリのフォーマット違反など、値の内容が不正な場合に送出される。
- **`None` の返却**：`board.pop()` や `board.last_move()` で履歴が空の場合、あるいは定跡検索（`lookup` / `get`）で該当局面が存在しない場合など、正常系として要素が存在しないことを表す。

### `ValueError: illegal move` が発生する原因は？

現在の局面において非合法な指し手（二歩、打ち歩詰め、王手放置、移動先に自駒がある手、または手番違いの手）を適用しようとした場合に発生する。
外部データを適用する前に検証したい場合は `board.is_legal_move(move)` を使用する。

## 関連項目

- [cshogi との API 対応表](compat.md)
- [フォーマット仕様一覧](formats/index.md)
- [Record と棋譜データモデル](records.md)
