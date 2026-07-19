# cshogi との API 対応表

rsshogi と cshogi の API 対応をまとめたリファレンスです。

## 実装状況

| カテゴリ | 状況 | 主な API |
|----------|------|----------|
| 盤面管理 | ✅ | `Board` |
| 合法手生成 | ✅ | `legal_moves`, `pseudo_legal_moves` |
| 状態判定 | ✅ | `is_in_check`, `is_mated`, `is_repetition`, `can_declare_win` |
| SFEN/USI | ✅ | `to_sfen`, `set_sfen`, `apply_usi` |
| PackedSfen | ✅ | `to_packed_sfen(out=None)`, `set_packed_sfen` |
| HCP / HCPE | ✅ | `to_hcp`, `set_hcp`, `to_hcpe`, `AperyMove`, `AperyMove32` |
| 局面キー | ✅ | `zobrist_hash` |
| Move32 復元 | ✅ | `move32_from_move`, `move_from_csa` |
| policy ラベル変換 | ✅ | `rsshogi.policy`, `rsshogi::labels::policy` |
| 棋譜 I/O | ✅ | `from_kif_str`, `from_ki2_str`, `from_csa_str`, `to_kif`, `to_ki2`, `to_csa` |
| USI エンジン連携 | ⚠️ | `rsshogi.usi`（USI 文字列の値オブジェクト変換のみ。エンジンプロセス制御は対象外）|

## API 差分（主要）

### メソッド設計

```python
# cshogi
board.turn          # property
board.legal_moves   # property-like iterable

# rsshogi
board.turn          # property
board.legal_moves() # method
```

### 手番型

```python
board.turn  # Color
```

手番は `Color` に統一されています。

### push/pop 系

```python
# rsshogi
board.push_move(move)      # Move を受け取り Move32 を返す
board.push_usi("7g7f")     # USI 文字列を受け取り Move32 を返す
board.pop()                # 最後の手を取り消して Move32 を返す（履歴なければ None）
board.last_move()          # 最後の手を Move32 で返す（なければ None）
```

### 指し手の操作

```python
# Move32 のメソッド例
mv32 = board.push_move(move)   # push_move は Move32 を返す
to_sq = mv32.to_sq
from_sq = mv32.from_sq
is_drop = mv32.is_drop()
is_promo = mv32.is_promotion()
```

### cshogi / Apery 互換 API

```python
amv = move.to_apery()
mv = amv.to_move()

amv32 = board.apery_move32_from_move(move)
amv32 = move32.to_apery(board)
move32 = amv32.to_move32(board)
```

`AperyMove` は `cshogi.move16(...)` と同じ 16bit 形式です。
`AperyMove32` は Apery 系 32bit move に対応しますが、`rsshogi.Move32` とは保持情報が異なります。

### policy ラベル

cshogi/dlshogi 系でよく使われる 27x81 の policy ラベルは、
Python では `rsshogi.policy`、Rust では `labels::policy` で提供しています。

| 用途 | cshogi 系 | rsshogi |
|------|-----------|--------|
| 2187 クラスの move label | `make_move_label(...)` 相当 | `rsshogi.policy.move_label(mv, turn)` |
| 1496 クラスへの圧縮 | ライブラリごとに別実装 | `rsshogi.policy.compact_move_label(mv, turn)` |
| 2187 → 1496 変換 | - | `rsshogi.policy.move_label_to_compact(label)` |
| 1496 → 2187 変換 | - | `rsshogi.policy.compact_move_label_to_move_label(label)` |

`MoveLabel` は `class * 81 + to_sq` の 2187 クラスです。
後手番の手は 180 度回転してからラベル化するため、常に先手視点で比較できます。

`CompactMoveLabel` は「局面で合法」ではありません。
空盤での移動・成り・駒打ち制約から見て、構造的に現れうる 1496 クラスだけを残した
gapless なラベルです。

Python では `Move`, `Move32`, `int`, `USI` 文字列をそのまま `move` 引数に渡せます。

### 指し手エンコーディング

rsshogi と cshogi は異なる指し手エンコーディングを採用しています。
`rsshogi` では `AperyMove` / `AperyMove32` を追加し、整数値を保ったまま相互変換できます。
通常の手生成や盤面操作では `Move` / `Move32` を使い、外部データとの境界でのみ
`AperyMove` 系を使うのを推奨します。

| 項目 | rsshogi (互換) | cshogi (Apery 型) |
|------|----------------------|-------------------|
| 駒打ち判定 | bit 14 フラグ | `from >= 81` |
| 成りフラグ位置 | bit 15 | bit 14 |
| 駒情報 | 移動後の Piece (5bit) | PieceType (4bit) × 2 |

### `AperyMove32 -> Move32` が局面依存な理由

`cshogi` の内部 32bit move は `pieceTypeFrom` と `capturedPieceType` を保持します。
一方、`rsshogi.Move32` は「移動後の色付き駒 (`Piece`)」を保持します。

そのため `AperyMove32` から `Move32` を復元するには、

- 現在の手番
- 移動元の盤上駒

が必要です。`rsshogi` ではこれを `AperyMove32.to_move32(board)` として公開しています。

### 指し手型の命名

rsshogi と cshogi/参照実装では型名が異なります。

| サイズ | rsshogi | YaneuraOu | cshogi |
|--------|--------|-----------|--------|
| 16bit | `Move` | `Move16` | `Move16` |
| 32bit | `Move32` | `Move` | `Move` |

rsshogi は 16bit 指し手を基本型と位置付け `Move` と命名しています。
YaneuraOu/cshogi では 32bit が `Move` で、16bit が `Move16` です。

**重要:** rsshogi の `Move` は 参照実装の `Move16` と同一のビットレイアウトですが、
cshogi (Apery 系) の `Move16` とはビットレイアウトが異なります。
整数値を保持した変換が必要な場合は `Move.to_apery()` / `AperyMove.to_move()` を使ってください。

### rsshogi.usi（USI 文字列パーサ）

`rsshogi.usi` モジュールは USI プロトコルの値オブジェクト変換を提供します。
エンジンプロセスの起動・通信は対象外です。

| クラス / 関数 | 説明 |
|---------------|------|
| `UsiInfo` / `parse_info(line)` | `info depth ... score ...` 行のパース |
| `UsiBestMove` / `parse_bestmove(line)` | `bestmove ... ponder ...` 行のパース |
| `UsiScore` | cp / mate 距離のラッパー |
| `UsiBound` | `lowerbound` / `upperbound` / `exact` |
| `UsiGoCommand` | `go` コマンドのシリアライズ |
| `move_from_usi(usi)` | `resign` / `win` / `0000` / `none` を含む USI 指し手変換 |

詳細は [rsshogi.usi](../python/usi.md) を参照してください。
