# Record 系クラス

棋譜を扱うためのクラス群です。`Record` は棋譜全体、`RecordMetadata` はヘッダ情報、
`MoveEntry` は各指し手を表します。終局結果は `Record.result` から `GameResult` として取得します。

## Record

```python
rsshogi.record.Record
```

棋譜全体を表すクラスです。初期局面、開始局面コメント、指し手列、メタデータ、終局結果を保持します。
終局特殊手がない棋譜では `result` は `GameResult.INVALID` になります。

`Record(init_position_sfen, moves=None, metadata=None, terminal=None, initial_comment=None)` で Python から直接生成できます。

### 基本的な使い方

```python
import rsshogi as rs

# KIF から読み込み
record = rs.record.Record.from_kif_str(kif_text)

# 情報を表示
print(f"先手: {record.metadata.black_player}")
print(f"総手数: {record.move_count()}")

# 指し手を確認
for move_rec in record.moves:
    print(move_rec.move.to_usi())

# 結果
print(f"結果: {record.result.name}")
```

### プロパティ

| プロパティ | 型 | 書込 | 説明 |
|------------|-----|:----:|------|
| `init_position_sfen` | `str` | - | 初期局面の SFEN 文字列 |
| `initial_comment` | `str \| None` | o | 開始局面（手数 0）に対応するコメント |
| `moves` | `list[MoveEntry]` | - | 指し手の一覧（本手順のみ） |
| `metadata` | `RecordMetadata` | o | 対局ヘッダ情報 |
| `result` | `GameResult` | - | 終局結果（[`GameResult`](game_result.md) 参照） |
| `result_info` | `GameResultInfo` | - | 終局情報の typed view（`ply_count` / `reason` / `end_time_ms` / `end_comment`） |
| `main_terminal` | `SpecialMoveEntry \| None` | o | 本手順末尾の終局特殊手（`None` 代入でのクリアは不可） |

### メソッド

#### 基本操作

| メソッド | 戻り値 | 説明 |
|----------|--------|------|
| `move_count()` | `int` | 指し手数（本手順） |
| `update_metadata(patch, *, strict=False)` | `None` | メタデータを部分更新（`None` は指定キーをクリア） |
| `set_metadata_attribute(key, value)` | `None` | metadata.attributes の追加/更新 |
| `remove_metadata_attribute(key)` | `str \| None` | metadata.attributes から削除 |
| `clear_metadata_attributes()` | `None` | metadata.attributes を全削除 |
| `extend_main_line(moves)` | `list[int]` | 本手順末尾に指し手を追加 |
| `set_main_terminal(terminal)` | `int` | 終局特殊手を設定 |
| `to_dict()` | `dict` | 辞書形式へ変換 |

#### パース（クラスメソッド）

| メソッド | 説明 |
|----------|------|
| `from_kif_str(text)` | KIF 文字列から生成 |
| `from_ki2_str(text)` | KI2 文字列から生成 |
| `from_csa_str(text)` | CSA 文字列から生成 |
| `from_jkf_str(text)` | JKF (JSON) 文字列から生成 |
| `from_usi_position(position, *, allow_special_tokens=False)` | USI position command から生成 |
| `from_pack(data)` | pack バイナリから単一棋譜を生成 |
| `from_sbinpack(data)` | sbinpack バイナリから生成 |
| `from_main_line(init_position_sfen, moves, terminal=None, metadata=None, initial_comment=None)` | 本手順から生成 |
| `from_usi_main_line(init_position_sfen, usi_moves, result=None, move_times_ms=None, evals=None, nodes=None, depths=None, seldepths=None, wall_times_ms=None, latency_deltas_ms=None, metadata=None, initial_comment=None)` | USI 手順・評価値・消費時間などを受け取り、合法性を検証しながら `Record` を生成 |
| `from_dict(data, *, strict=False)` | 辞書形式から生成 |
| `from_kif_file(path, *, encoding=None)` | KIF ファイルから生成 |
| `from_ki2_file(path, *, encoding="shift_jis")` | KI2 ファイルから生成 |
| `from_csa_file(path, *, encoding="shift_jis")` | CSA ファイルから生成 |
| `from_jkf_file(path, *, encoding="utf-8")` | JKF ファイルから生成 |

#### 書き出し（インスタンスメソッド）

| メソッド | 説明 |
|----------|------|
| `to_kif()` | KIF 文字列へ変換 |
| `to_csa()` | CSA 文字列へ変換 |
| `to_ki2()` | KI2 文字列へ変換 |
| `to_jkf()` | JKF 文字列へ変換 |
| `to_usi_position(*, include_special_tokens=False)` | USI position command 文字列へ変換 |
| `to_pack()` | pack バイナリへ変換 |
| `to_psv(*, include_main=True, include_variations=False)` | `PackedSfenValue` 配列（`list[bytes]`）へ変換（引数はすべてキーワード専用） |
| `write_kif(path, *, encoding=None)` | KIF ファイルへ書き出し |
| `write_csa(path, *, encoding="shift_jis")` | CSA ファイルへ書き出し |
| `write_ki2(path, *, encoding="shift_jis")` | KI2 ファイルへ書き出し |
| `write_jkf(path, *, encoding="utf-8")` | JKF ファイルへ書き出し |

#### 棋譜 I/O の互換メモ

- `to_kif()` / `to_ki2()` は、変化手順と終局要約が両方ある場合に、
  変化ブロックを先に、`まで...` を最後に出力します。
- KIF/KI2 の出力整形は ShogiHome/tsshogi 系との実務互換と
  `shogi-validator` 寄りの見た目を意識しています。
- `from_csa_str()` / `from_csa_file()` は CSA 3.0 に合わせて `'*comment` だけを
  コメントとして受理し、plain な `'comment` は読み飛ばします。`to_csa()` は `'*comment`
  として出力します。
- 互換性のため、手番行より前の `'*comment` も開始局面コメントとして受理し、
  `to_csa()` では手番行直後の正規形へ寄せて出力します。
- `from_kif_str()` / `from_ki2_str()` は連続するコメント行を `\n` で連結し、
  同じ指し手の `MoveEntry.comment` として保持します。初手前コメントは `Record.initial_comment` に入ります。
- `RecordMetadata.comment` はヘッダ自由コメントではなく、KIF の `備考` /
  CSA の `$NOTE` / JKF header の `備考` に対応します。
- `main_terminal is None` の棋譜も `to_kif()` / `to_ki2()` / `to_csa()` で出力でき、
  その場合は終局行を省略します。
- `write_kif()` / `write_ki2()` / `write_csa()` は末尾改行つきで書き出します。
- `to_pack()` は本手順のみを変換し、各手に `MoveEntry.engine_info.eval` が必要です。
- `from_pack()` は単一局のみ対応します。複数局は `rsshogi.record.decode_pack()` を使います。

#### 分岐の操作

KIF/KI2/JKF の分岐（変化）を扱うためのメソッドです。

| メソッド | 戻り値 | 説明 |
|----------|--------|------|
| `root_node_id()` | `int` | ルートノード ID |
| `main_line_ids()` | `list[int]` | 本手順のノード ID 一覧 |
| `children(node_id)` | `list[int]` | 子ノード ID 一覧（先頭が本手順） |
| `node_parent(node_id)` | `int \| None` | 親ノード ID（ルートは `None`） |
| `node_entry(node_id)` | `RecordEntry \| None` | ノードの payload（通常手または終局特殊手） |
| `node_move(node_id)` | `MoveEntry \| None` | ノードの指し手（ルートは `None`） |
| `node_terminal(node_id)` | `SpecialMoveEntry \| None` | ノードの終局特殊手（通常手ノードは `None`） |
| `into_editor()` | `RecordEditor` | 一時カーソル付き editor を生成 |

`RecordEditor` は `Record` と現在ノード、現在局面を所有する一時 editor です。
`append_move()` / `append_special()` / `go_to()` / `go_back()` / `go_forward()` /
`branch_to()` で編集し、`into_record()` で `Record` に戻します。`into_record()` 後の
editor 操作は `ValueError` になります。

### 使用例

#### 棋譜の読み込みと再生

```python
import rsshogi as rs

record = rs.record.Record.from_kif_file("game.kif")

# 盤面を作成して指し手を再生
board = rs.core.Board(sfen=record.init_position_sfen)

for move_rec in record.moves:
    board.apply_move(move_rec.move)

print(f"最終局面: {board.to_sfen()}")
```

#### 棋譜の変換

```python
import rsshogi as rs

# KIF を読み込んで CSA に変換
record = rs.record.Record.from_kif_file("game.kif")
record.write_csa("game.csa")
```

---

### `from_dict` 入力形式（主要キー）

- `init_position_sfen: str`（必須）
- `initial_comment: str | None`（任意）
- `moves: list[dict]`（省略時は空配列）
  - `move: MoveEntry | Move32 | Move | int | str`
  - `int` は `<= 0xFFFF` を Move、`> 0xFFFF` を Move32 として解釈
  - 各手は局面経由で正規化され、不正手は `ValueError`
  - `time_ms`, `comment` は任意
  - `engine_info: dict | None` は任意
    - `eval`, `depth`, `nodes`, `seldepth` は任意
    - `extras: dict[str, str|int|float|bool]` は任意
- `result: dict | int | str`（任意）
  - `result: int | str`（`GameResult` の数値コードまたは名前）※`dict` の場合は必須
  - `ply_count`, `reason`, `end_time_ms`, `end_comment` は任意
- `metadata: dict`（任意）
  - 正規キー（strict でも受理）:
    - `event`, `site`, `black_player`, `white_player`
    - `game_name`, `game_type`, `updated_date`
    - `time_control`, `black_time_control`, `white_time_control`
    - `max_moves`, `impasse_rule`, `start_date`, `end_date`, `comment`, `attributes`

`from_dict` は `main_terminal` キーを受け付けません。終局情報は `result` で指定してください。
`initial_comment` は開始局面コメントとして保持され、KIF/KI2 では 1 手目の前へ、
CSA では手番行の直後へ出力されます。

### strict / permissive

- `strict=False`（既定, permissive）
  - `metadata` の未知キーは `attributes` に退避（値は `str | None` のみ）
  - ただし削除済み互換キー
    （`black_player_name`, `white_player_name`, `time_control_black`,
    `time_control_white`, `time_specs`）は `ValueError`
- `strict=True`
  - 未知キー・削除済み互換キーを `ValueError` にする
    （top-level / `moves[*]` / `result` / `metadata`）
  - 型不正は `TypeError`

`to_dict()` は正規キーのみを出力します。

`RecordMetadataKey` は正規 metadata key 名の定数 namespace です。
`RecordMetadataKey.BLACK_PLAYER == "black_player"` のように、`attributes` や辞書 API
で同じ key 名を使いたい場合に利用できます。

### metadata の保持保証

- 第一級フィールド:
  - `event`, `site`, `black_player`, `white_player`
  - `game_name`, `game_type`, `updated_date`
  - `time_control`, `black_time_control`, `white_time_control`
  - `max_moves`, `impasse_rule`, `start_date`, `end_date`, `comment`
- `attributes`:
  - 上記第一級に含まれない文字列メタデータを保持
- 未設定値:
  - 第一級フィールドはすべて属性として常に存在し、未設定時は `None` を返す

## GameResultInfo

```python
rsshogi.record.GameResultInfo
```

`Record.result_info` から取得できる終局情報ビューです。
`result_info` は取得時点のスナップショットで、`Record` を更新しても既存の
`GameResultInfo` インスタンスは自動更新されません。

| プロパティ | 型 | 説明 |
|------------|-----|------|
| `result` | `GameResult` | 終局結果 |
| `ply_count` | `int` | 手数（本手順） |
| `reason` | `str \| None` | 終局理由（終局行 raw） |
| `end_time_ms` | `int \| None` | 終局消費時間（ms） |
| `end_comment` | `str \| None` | 終局コメント |

## RecordMetadata

```python
rsshogi.record.RecordMetadata
```

KIF/CSA ヘッダ由来のメタ情報を保持します。未指定の項目は `None` になります。
`RecordMetadata(...)` で Python から直接生成できます。

### プロパティ

| プロパティ | 型 | 書込 | 説明 | KIF ヘッダ例 |
|------------|-----|:----:|------|-------------|
| `event` | `str \| None` | o | 棋戦名 | `棋戦：` |
| `site` | `str \| None` | o | 対局場所 | `場所：` |
| `black_player` | `str \| None` | o | 先手対局者名 | `先手：` |
| `white_player` | `str \| None` | o | 後手対局者名 | `後手：` |
| `time_control` | `TimeControl \| None` | o | 持ち時間 | `持ち時間：` |
| `black_time_control` | `TimeControl \| None` | o | 先手個別の持ち時間 | `先手持ち時間：`, `$TIME+:` |
| `white_time_control` | `TimeControl \| None` | o | 後手個別の持ち時間 | `後手持ち時間：`, `$TIME-:` |
| `game_name` | `str \| None` | o | 対局名（第一級フィールド） | 互換入力: `attributes["game_name"]` |
| `game_type` | `str \| None` | o | 対局種別（第一級フィールド） | 互換入力: `attributes["game_type"]` |
| `max_moves` | `int \| None` | o | 最大手数 | `最大手数：`, `$MAX_MOVES:` |
| `impasse_rule` | `str \| None` | o | 持将棋規定 | `持将棋：`, `$JISHOGI:` |
| `start_date` | `str \| None` | o | 開始日時 | `開始日時：` |
| `end_date` | `str \| None` | o | 終了日時 | `終了日時：` |
| `updated_date` | `str \| None` | o | 更新日時（第一級フィールド） | 互換入力: `attributes["updated_date"]` |
| `comment` | `str \| None` | o | 備考 / NOTE | `備考：`, `$NOTE:` |

### メソッド

| プロパティ | 型 | 説明 |
|------------|-----|------|
| `attributes` | `dict[str, str]` | 未定義ヘッダ項目の辞書（read/write） |

| メソッド | 戻り値 | 説明 |
|----------|--------|------|
| `set_attribute(key, value)` | `None` | 属性を追加/更新 |
| `remove_attribute(key)` | `str \| None` | 属性を削除 |
| `clear_attributes()` | `None` | 属性を全削除 |

### 使用例

```python
import rsshogi as rs

record = rs.record.Record.from_kif_file("game.kif")
meta = record.metadata

print(f"棋戦: {meta.event or '不明'}")
print(f"先手: {meta.black_player or '不明'}")
print(f"後手: {meta.white_player or '不明'}")

# 持ち時間
if meta.time_control:
    tc = meta.time_control
    print(f"持ち時間: {tc.base_seconds}秒 + 秒読み{tc.byoyomi_seconds}秒")

# 先後別の持ち時間（CSA の TIME+ / TIME- など）
if meta.black_time_control:
    print(f"先手持ち時間: {meta.black_time_control.to_spec()}")
if meta.white_time_control:
    print(f"後手持ち時間: {meta.white_time_control.to_spec()}")

if meta.max_moves is not None:
    print(f"最大手数: {meta.max_moves}")
if meta.impasse_rule:
    print(f"持将棋規定: {meta.impasse_rule}")

# カスタム属性
meta.attributes = {"game_name": "arena-match", "black_rate": "2100"}
for key, value in meta.attributes.items():
    print(f"{key}: {value}")
```

---

## TimeControl

```python
rsshogi.record.TimeControl
```

持ち時間の情報を保持する型です。
`TimeControl(base_seconds, byoyomi_seconds, increment_seconds)` で Python から直接生成できます。

### プロパティ

| プロパティ | 型 | 説明 |
|------------|-----|------|
| `base_seconds` | `int` | 持ち時間（秒） |
| `byoyomi_seconds` | `int` | 秒読み（秒） |
| `increment_seconds` | `int` | 加算時間（秒） |

### メソッド

| メソッド | 戻り値 | 説明 |
|----------|--------|------|
| `to_spec()` | `str` | `base+byoyomi+increment` 形式の文字列 |
| `from_spec(spec)` | `TimeControl` | spec 文字列から生成（不正形式は `ValueError`） |
| `normalize_spec(spec)` | `str` | spec を正規化（例: `300.0+10+0` → `300+10+0`） |

---

## MoveEntry

```python
rsshogi.record.MoveEntry
```

棋譜内の各指し手のデータを保持します。棋譜としての情報（指し手、時間、コメント）と、
エンジン解析情報（`EngineInfo`）を分離して管理します。
`MoveEntry(move, *, time_ms=None, comment=None, engine_info=None)` で生成できます。

### プロパティ

| プロパティ | 型 | 書込 | 説明 |
|------------|-----|:----:|------|
| `move` | `Move` | - | 指し手（16bit 指し手型） |
| `comment` | `str \| None` | o | 指し手コメント（KIF の `*` 行） |
| `time_ms` | `int \| None` | o | 消費時間（ミリ秒） |
| `engine_info` | `EngineInfo \| None` | o | エンジン解析情報 |

---

## EngineInfo

```python
rsshogi.record.EngineInfo
```

エンジン解析情報を保持します。`MoveEntry.engine_info` から参照します。
`EngineInfo(eval=None, depth=None, nodes=None, seldepth=None, wall_time_ms=None, latency_delta_ms=None, extras=None)` で生成できます。

### プロパティ

| プロパティ | 型 | 書込 | 説明 |
|------------|-----|:----:|------|
| `eval` | `int \| None` | o | 評価値（先手視点、センチポーン単位） |
| `depth` | `int \| None` | o | 探索深度 |
| `nodes` | `int \| None` | o | 探索ノード数 |
| `seldepth` | `int \| None` | o | 選択深度 |
| `extras` | `dict[str, str \| int \| float \| bool]` | o | 拡張キー/値 |
| `wall_time_ms` | `int \| None` | o | 壁時間（ms）。`extras["wall_time_ms"]` または `extras["arena.wall_time_ms"]` の typed アクセサ |
| `latency_delta_ms` | `int \| None` | o | レイテンシ差分（ms）。`extras["latency_delta_ms"]` または `extras["arena.latency_delta_ms"]` の typed アクセサ |

### メソッド

| メソッド | 戻り値 | 説明 |
|----------|--------|------|
| `set_extra(key, value)` | `None` | 拡張属性を追加/更新 |
| `remove_extra(key)` | `str \| int \| float \| bool \| None` | 拡張属性を削除 |
| `clear_extras()` | `None` | 拡張属性を全削除 |

### 使用例

```python
import rsshogi as rs

info = rs.record.EngineInfo(
    eval=120,
    depth=18,
    nodes=145000,
    seldepth=27,
    extras={
        "nps": 180000,
        "arena.wall_time_ms": 1200,
        "arena.latency_delta_ms": 30,
        "book_hit": False,
    },
)
move = rs.record.MoveEntry("7g7f", time_ms=900, comment="opening", engine_info=info)

record = rs.record.Record.from_kif_file("game.kif")

for i, move_rec in enumerate(record.moves, 1):
    usi = move_rec.move.to_usi()
    info = move_rec.engine_info
    eval_str = ""
    if info and info.eval is not None:
        eval_str = f" (評価値: {info.eval})"
    
    # 消費時間
    time_str = ""
    if move_rec.time_ms is not None:
        time_str = f" [{move_rec.time_ms / 1000:.1f}秒]"
    
    print(f"{i}手目: {usi}{eval_str}{time_str}")
```

---

## SpecialMoveEntry

```python
rsshogi.record.SpecialMoveEntry
```

終局特殊手ノードの情報です。
`SpecialMoveEntry(kind, result, unknown_name=None, comment=None, time_ms=None, raw=None)` で Python から直接生成できます。

終局結果から自動的に `kind` を導出するには `SpecialMoveEntry.from_result(result, time_ms=None, comment=None, raw=None)` を使用します。

### プロパティ

| プロパティ | 型 | 書込 | 説明 |
|------------|-----|:----:|------|
| `kind` | `str` | - | 終局種別（下表参照） |
| `unknown_name` | `str \| None` | - | `kind == "UNKNOWN"` のときの元文字列 |
| `result` | `GameResult` | - | 終局結果 |
| `comment` | `str \| None` | o | 終局コメント |
| `time_ms` | `int \| None` | o | 終局消費時間（ミリ秒） |
| `raw` | `str \| None` | o | 元の終局行 |

#### `kind` の値一覧

| 値 | 説明 |
|-----|------|
| `INTERRUPT` | 中断 |
| `RESIGN` | 投了 |
| `MAX_MOVES` | 最大手数到達 |
| `IMPASSE` | 持将棋 |
| `DRAW` | 引き分け |
| `REPETITION_DRAW` | 千日手 |
| `MATE` | 詰み |
| `NO_MATE` | 不詰 |
| `TIMEOUT` | 時間切れ |
| `WIN_BY_ILLEGAL_MOVE` | 反則勝ち |
| `LOSE_BY_ILLEGAL_MOVE` | 反則負け |
| `WIN_BY_DECLARATION` | 入玉宣言勝ち |
| `WIN_BY_DEFAULT` | 不戦勝 |
| `LOSE_BY_DEFAULT` | 不戦敗 |
| `TRY` | トライ |
| `UNKNOWN` | 不明（`unknown_name` に元文字列を保持） |

---

## バイナリ形式

### sbinpack 形式

```python
record.to_sbinpack(
    stem_score: int = 0,
    include_main: bool = True,
    include_variations: bool = False,
    metadata: bytes | None = None,
) -> bytes

Record.from_sbinpack(data: bytes) -> Record
Record.from_sbinpack_with_metadata(data: bytes) -> tuple[Record, bytes]

rs.record.decode_sbinpack(data: bytes) -> list[tuple[Record, bytes]]
rs.record.decode_sbinpack_file(path) -> list[tuple[Record, bytes]]

rs.record.write_sbinpack(
    records: list[Record],
    *,
    stem_score: int = 0,
    include_main: bool = True,
    include_variations: bool = False,
    metadatas: list[bytes | None] | None = None,
) -> bytes
rs.record.write_sbinpack_file(path, records, *, metadatas=None) -> None
```

sbinpack v2 形式のバイナリに変換します。`metadata` を指定すると、
0..=127 bytes の opaque metadata として per-chain に格納します。
一括 writer では `metadatas` を `records` と同じ順序・長さで渡します。`include_variations=True` の場合、各 record の metadata はその record から出る各 variation chain にコピーされます。

**制約:** 本手順の全ての指し手に評価値が必要です。

```python
# 変換
binary = record.to_sbinpack()

# 読み戻し
record2 = rs.record.Record.from_sbinpack(binary)
```

### pack 形式

```python
record.to_pack() -> bytes
Record.from_pack(data: bytes) -> Record

rs.record.write_pack(records: list[Record]) -> bytes
rs.record.decode_pack(data: bytes) -> list[Record]
rs.record.write_pack_file(path, records) -> None
rs.record.decode_pack_file(path) -> list[Record]
```

ScriptCollection 互換の可変長 `pack` 形式です。

```python
payload = rs.record.write_pack([record])
restored = rs.record.decode_pack(payload)
record2 = rs.record.Record.from_pack(record.to_pack())
```

### PSV 配列

```python
record.to_psv(
    *,
    include_main: bool = True,
    include_variations: bool = False,
) -> list[bytes]
```

棋譜から `PackedSfenValue`（40 bytes）を1局面ずつ生成して返します。

- 各要素は `Board.to_psv(...)` と同一レイアウトの 40 bytes
- 既定は本手順のみ（`include_main=True, include_variations=False`）
- 分岐も含める場合は `include_variations=True`
- `include_main=False, include_variations=False` の場合は空配列

**制約:** 出力対象に含めた全ての指し手に評価値（`MoveEntry.engine_info.eval`）が必要です。

---

## 関連項目

- [`GameResult`](game_result.md) - 終局結果の詳細
- [`Board`](board.md) - 盤面管理クラス
- [`Move32`](move32.md) / [`Move`](move.md) - 指し手型（32bit / 16bit）
- [棋譜フォーマット一覧](../reference/formats/index.md) - KIF/CSA/KI2/JKF の仕様
