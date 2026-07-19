# 棋譜のパースと書き出し

棋譜のパース・書き出しはすべて [`Record`](record.md) クラスのメソッドとして提供されています。

## 対応フォーマット一覧

| フォーマット | パース | エクスポート | 備考 |
|-------------|:------:|:----------:|------|
| KIF | o | o | 柿木形式。分岐（変化）対応 |
| KI2 | o | o | 指し手のみ形式（盤面情報から駒種を復元） |
| CSA | o | o | CSA 標準棋譜形式 |
| JKF | o | o | JSON Kifu Format。分岐（forks）対応 |
| sbinpack | o | o | バイナリ学習データ形式 |
| dict | o | o | Python 辞書形式（`to_dict()` / `from_dict()`） |

## パース

### 文字列からのパース

| メソッド | 説明 |
|----------|------|
| `Record.from_kif_str(text)` | KIF 文字列から生成 |
| `Record.from_ki2_str(text)` | KI2 文字列から生成 |
| `Record.from_csa_str(text)` | CSA 文字列から生成 |
| `Record.from_jkf_str(text)` | JKF (JSON) 文字列から生成 |
| `Record.from_usi_position(position, *, allow_special_tokens=False)` | USI position command から生成 |
| `Record.from_sbinpack(data)` | sbinpack バイナリ（`bytes`）から生成 |
| `Record.from_main_line(init_position_sfen, moves, terminal=None, metadata=None, initial_comment=None)` | 指し手列から直接生成 |
| `Record.from_dict(data, *, strict=False)` | Python 辞書（`dict`）から生成 |

### 終局結果と付加情報

- `Record.result` は `GameResult` を直接返します
- `Record.result_info` で `ply_count` / `reason` / `end_time_ms` / `end_comment` を typed access できます
  （取得時点のスナップショット）
- `Record.main_terminal` で終局特殊手（`SpecialMoveEntry`）へアクセスできます
- 終局特殊手が存在しない棋譜では `Record.main_terminal is None`、
  `Record.result == GameResult.INVALID` になります
- `Record.from_dict()` の終局入力は `result` ペイロードのみです（`main_terminal` キーは非対応）
- 分岐探索中のノードに対しては `Record.node_terminal(node_id)` で終局特殊手を取得できます
- KIF/KI2 の要約行（`まで...手で最大手数` / `まで...手で持将棋`）は、それぞれ
  `DRAW_BY_MAX_PLIES` / `DRAW_BY_IMPASSE` として解釈されます
- KIF の指し手行末尾にある `(<elapsed>/<total>)` 形式の消費時間も `MoveEntry.time_ms`
  に取り込まれます
- CSA の終局 `%...` 行に続く `T...` とコメント行（`'...` / `'*...`）は内部的に保持され、
  `to_csa()` 時にも維持されます
- 先後別時間設定や上限手数は `record.metadata.black_time_control` /
  `record.metadata.white_time_control` / `record.metadata.max_moves` / `record.metadata.impasse_rule`
  から参照できます

### ファイルからの読み込み

| メソッド | デフォルトエンコーディング | 説明 |
|----------|--------------------------|------|
| `Record.from_kif_file(path, *, encoding=None)` | 拡張子で判定（後述） | KIF ファイルから生成 |
| `Record.from_ki2_file(path, *, encoding="shift_jis")` | Shift_JIS | KI2 ファイルから生成 |
| `Record.from_csa_file(path, *, encoding="shift_jis")` | Shift_JIS | CSA ファイルから生成 |
| `Record.from_jkf_file(path, *, encoding="utf-8")` | UTF-8 | JKF ファイルから生成 |

#### KIF ファイルのエンコーディング

`from_kif_file` は `encoding` 引数を省略した場合、ファイルの拡張子に応じてエンコーディングを自動判定します。

| 拡張子 | エンコーディング |
|--------|----------------|
| `.kif` | Shift_JIS |
| `.kifu` | UTF-8 |

明示的に指定する場合:

```python
# UTF-8 の .kif ファイルを読み込む場合
record = Record.from_kif_file("game.kif", encoding="utf-8")
```

## 書き出し

### 文字列への変換

| メソッド | 説明 |
|----------|------|
| `record.to_kif()` | KIF 文字列へ変換 |
| `record.to_ki2()` | KI2 文字列へ変換 |
| `record.to_csa()` | CSA 文字列へ変換 |
| `record.to_jkf()` | JKF (JSON) 文字列へ変換 |
| `record.to_usi_position()` | USI position command 文字列へ変換 |
| `record.to_sbinpack(...)` | sbinpack バイナリへ変換 |
| `record.to_dict()` | Python 辞書（`dict`）へ変換 |

### ファイルへの書き出し

| メソッド | デフォルトエンコーディング |
|----------|--------------------------|
| `record.write_kif(path, *, encoding=None)` | 拡張子で判定（`.kif` → Shift_JIS, `.kifu` → UTF-8） |
| `record.write_csa(path, *, encoding="shift_jis")` | Shift_JIS |
| `record.write_ki2(path, *, encoding="shift_jis")` | Shift_JIS |
| `record.write_jkf(path, *, encoding="utf-8")` | UTF-8 |

## 実務互換メモ

- `Record.to_kif()` / `to_ki2()` は Rust core の exporter をそのまま使い、
  ShogiHome/tsshogi 系の実務互換と `shogi-validator` に近い整形を意識しています。
- 変化手順と終局要約 (`まで...`) が同居する KIF/KI2 では、
  `変化：...` が先、`まで...` が最後です。`main_terminal` を落とさず
  round-trip しやすくするためです。
- `Record.from_csa_str()` / `from_csa_file()` は CSA 3.0 に合わせて `'*comment`
  だけをコメントとして受理し、plain な `'comment` は読み飛ばします。
  `Record.to_csa()` は `'*comment` を出力します。
- 互換性のため、手番行より前の `'*comment` は `initial_comment` に正規化して受理します。
- `RecordMetadata.comment` は KIF の `備考` / CSA の `$NOTE` に対応し、
  匿名ヘッダコメントを受けるためのフィールドではありません。
- `Record.main_terminal is None` の場合、`to_kif()` / `to_ki2()` / `to_csa()` は
  終局行を省略して本手順のみを書き出します。
- `write_kif()` / `write_ki2()` / `write_csa()` は末尾改行つきで書き出します。

## 使用例

### フォーマット変換

```python
import rsshogi as rs

# KIF → CSA に変換
record = rs.record.Record.from_kif_file("game.kif")
record.write_csa("game.csa")

# KIF → JKF (JSON) に変換
record.write_jkf("game.json")
```

### 文字列での処理

```python
import rsshogi as rs

kif_text = """
先手：先手太郎
後手：後手花子
手数----指手---------消費時間--
   1 ７六歩(77)
   2 ３四歩(33)
"""

record = rs.record.Record.from_kif_str(kif_text)
print(record.to_csa())
```

## 関連項目

- [`Record`](record.md) - 棋譜レコードの詳細
- [棋譜フォーマット一覧](../reference/formats/index.md) - 各フォーマットの仕様
