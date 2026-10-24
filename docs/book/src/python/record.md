# 棋譜管理（`Record` / `GameResult` / 棋譜入出力）

`rsshogi.record` は棋譜の読み込み、書き出し、手順ツリーの走査と編集、対局メタデータやエンジン解析情報の管理、およびバイナリ棋譜フォーマット（`pack` / `sbinpack` / `PackedSfenValue`）の変換を提供するモジュールである。

```python
from rsshogi.record import (
    EngineInfo,
    GameResult,
    GameResultInfo,
    MoveEntry,
    Record,
    RecordBuilder,
    RecordEditor,
    RecordEntry,
    RecordMetadata,
    RecordMetadataBuilder,
    RecordMetadataKey,
    SpecialMoveEntry,
    TimeControl,
    decode_pack,
    decode_pack_file,
    decode_sbinpack,
    decode_sbinpack_file,
    write_pack,
    write_pack_file,
    write_sbinpack,
    write_sbinpack_file,
)
```

---

## `Record`

```python
Record(
    init_position_sfen: str,
    moves: Sequence[MoveEntry | Move32 | Move | int | str] | None = None,
    metadata: RecordMetadata | Mapping[str, Any] | None = None,
    terminal: SpecialMoveEntry | GameResult | int | str | None = None,
    initial_comment: str | None = None,
) -> Record
```

1 局分の棋譜全体（初期局面、開始局面コメント、本手順と変化手順のツリー、対局ヘッダ情報、終局結果）を保持するクラスである。
直接コンストラクタで生成できるほか、KIF、KI2、CSA、JKF、USI `position`、辞書、各種バイナリ形式から生成できる。

### プロパティ

| プロパティ | 型 | 書込 | 説明 |
|---|---|:---:|---|
| `init_position_sfen` | `str` | 不可 | 初期局面の SFEN 文字列。 |
| `initial_comment` | `str \| None` | 可 | 開始局面（手数 0）に対応するコメント。 |
| `moves` | `list[MoveEntry]` | 不可 | 本手順の指し手リスト。 |
| `move_count` | `int` | 不可 | 本手順の指し手数（`len(record.moves)` と同値）。 |
| `metadata` | `RecordMetadata` | 可 | 対局ヘッダ情報（`RecordMetadata` または辞書を代入可能）。 |
| `result` | `GameResult` | 不可 | 本手順の終局結果。終局特殊手がない棋譜では `GameResult.INVALID` を返す。 |
| `result_info` | `GameResultInfo` | 不可 | 本手順の終局情報のビュー（取得時点のスナップショット）。 |
| `main_terminal` | `SpecialMoveEntry \| None` | 可 | 本手順末尾の終局特殊手（`SpecialMoveEntry` / `GameResult` / `int` / `str` を代入可能。`None` 代入による消去は不可）。 |
| `game_name` | `str \| None` | 不可 | `metadata.game_name` のショートカット。 |
| `game_type` | `str \| None` | 不可 | `metadata.game_type` のショートカット。 |
| `updated_date` | `str \| None` | 不可 | `metadata.updated_date` のショートカット。 |
| `black_time_control` | `TimeControl \| None` | 不可 | `metadata.black_time_control` のショートカット。 |
| `white_time_control` | `TimeControl \| None` | 不可 | `metadata.white_time_control` のショートカット。 |
| `end_time_ms` | `int \| None` | 不可 | 本手順の終局特殊手に記録された消費時間（ミリ秒）。 |
| `end_comment` | `str \| None` | 不可 | 本手順の終局特殊手に記録されたコメント。 |

### 基本操作と編集メソッド

| メソッド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `extend_main_line(moves)` | `moves: Sequence[MoveEntry \| Move32 \| Move \| int \| str]` | `list[int]` | 本手順の末尾に指し手列を追加し、追加された各ノード ID のリストを返す。各手は局面上で合法性を検証する。 |
| `set_main_terminal(terminal)` | `terminal: SpecialMoveEntry \| GameResult \| int \| str` | `int` | 本手順の末尾に終局特殊手を設定し、そのノード ID を返す。 |
| `update_metadata(patch, *, strict=False)` | `patch: RecordMetadata \| Mapping[str, Any]`, `strict: bool = False` | `None` | メタデータを部分更新する。辞書で値に `None` を指定したキーはクリアされる。 |
| `set_metadata_attribute(key, value)` | `key: str`, `value: str` | `None` | `metadata.attributes[key]` を追加または更新する。 |
| `remove_metadata_attribute(key)` | `key: str` | `str \| None` | `metadata.attributes` から指定キーを削除し、以前の値を返す。 |
| `clear_metadata_attributes()` | なし | `None` | `metadata.attributes` をすべて消去する。 |
| `to_dict()` | なし | `RecordDict` | 棋譜を正規キーから成る Python の辞書形式へ変換する。 |

### 読み込み（クラスメソッド）

ファイル読み込みメソッドの `encoding` 引数は位置引数とキーワード引数のどちらでも指定できる。

| クラスメソッド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `from_kif_str(text)` | `text: str` | `Record` | KIF 文字列から棋譜を生成する。 |
| `from_kif_file(path, encoding=None)` | `path: str \| PathLike`, `encoding: str \| None = None` | `Record` | KIF ファイルから棋譜を読み込む。`encoding=None` の場合、拡張子 `.kifu` は `"utf-8"`、それ以外は `"shift_jis"` として読み込む。 |
| `from_ki2_str(text)` | `text: str` | `Record` | KI2 文字列から棋譜を生成する。 |
| `from_ki2_file(path, encoding="shift_jis")` | `path: str \| PathLike`, `encoding: str = "shift_jis"` | `Record` | KI2 ファイルから棋譜を読み込む。 |
| `from_csa_str(text)` | `text: str` | `Record` | CSA 文字列から最初の 1 局を生成する。 |
| `from_csa_file(path, encoding="shift_jis")` | `path: str \| PathLike`, `encoding: str = "shift_jis"` | `Record` | CSA ファイルから最初の 1 局を読み込む。 |
| `from_csa_games_str(text)` | `text: str` | `list[Record]` | `/` 区切りの CSA 文字列から全対局をリストとして生成する。 |
| `from_csa_games_file(path, encoding="shift_jis")` | `path: str \| PathLike`, `encoding: str = "shift_jis"` | `list[Record]` | `/` 区切りの CSA ファイルから全対局をリストとして読み込む。 |
| `from_jkf_str(text)` | `text: str` | `Record` | JKF（JSON 棋譜フォーマット）文字列から棋譜を生成する。 |
| `from_jkf_file(path, encoding="utf-8")` | `path: str \| PathLike`, `encoding: str = "utf-8"` | `Record` | JKF ファイルから棋譜を読み込む。 |
| `from_usi_position(position, *, allow_special_tokens=False)` | `position: str`, `allow_special_tokens: bool = False` | `Record` | USI `position` コマンド文字列から棋譜を生成する。`allow_special_tokens=True` の場合は末尾の `resign` や `win` などの終局トークンも受理する。 |
| `from_pack(data)` | `data: bytes \| bytearray` | `Record` | `pack` バイナリ（単一局）から棋譜を復元する。 |
| `from_sbinpack(data)` | `data: bytes \| bytearray` | `Record` | `sbinpack` バイナリから棋譜を復元する。 |
| `from_sbinpack_with_metadata(data)` | `data: bytes \| bytearray` | `tuple[Record, bytes]` | `sbinpack` バイナリから棋譜と付属メタデータ（`bytes`）のタプルを復元する。 |
| `from_main_line(init_position_sfen, moves, terminal=None, metadata=None, initial_comment=None)` | 初期局面、本手順、終局情報、メタデータ | `Record` | 本手順の指し手列から合法性を検証しながら棋譜を生成する。 |
| `from_usi_main_line(...)` | 初期局面、USI 手順、各種テレメトリ配列 | `Record` | USI 文字列リストと評価値や消費時間などの配列から一括で棋譜を生成する（詳細は後述）。 |
| `from_dict(data, *, strict=False)` | `data: Mapping[str, Any]`, `strict: bool = False` | `Record` | 辞書から棋譜を生成する。 |

#### `Record.from_usi_main_line` のシグネチャ

```python
Record.from_usi_main_line(
    init_position_sfen: str,
    usi_moves: Sequence[str],
    result: SpecialMoveEntry | GameResult | int | str | None = None,
    move_times_ms: Sequence[int | None] | None = None,
    evals: Sequence[int | None] | None = None,
    nodes: Sequence[int | None] | None = None,
    depths: Sequence[int | None] | None = None,
    seldepths: Sequence[int | None] | None = None,
    wall_times_ms: Sequence[int | None] | None = None,
    latency_deltas_ms: Sequence[int | None] | None = None,
    metadata: RecordMetadata | Mapping[str, Any] | None = None,
    initial_comment: str | None = None,
) -> Record
```

`init_position_sfen` には SFEN 文字列または `"startpos"` を指定できる。
`evals[i]` は `usi_moves[i]` を指す前の局面における手番側視点のセンチポーン評価値である。
各テレメトリ配列を指定する場合は `usi_moves` と同じ長さで渡す。

### 書き出し（インスタンスメソッド）

| メソッド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `to_kif()` | なし | `str` | KIF 形式の文字列へ変換する。 |
| `write_kif(path, encoding=None)` | `path: str \| PathLike`, `encoding: str \| None = None` | `None` | KIF ファイルへ書き出す（拡張子 `.kifu` は `"utf-8"`、それ以外は `"shift_jis"` が既定）。 |
| `to_ki2()` | なし | `str` | KI2 形式の文字列へ変換する。 |
| `write_ki2(path, encoding="shift_jis")` | `path: str \| PathLike`, `encoding: str = "shift_jis"` | `None` | KI2 ファイルへ書き出す。 |
| `to_csa(*, version="2.2")` | `version: str = "2.2"` | `str` | CSA 形式の文字列へ変換する。`version` は `"2.2"` または `"3.0"` を指定する。 |
| `write_csa(path, encoding="shift_jis", *, version="2.2")` | `path: str \| PathLike`, `encoding: str = "shift_jis"`, `version: str = "2.2"` | `None` | CSA ファイルへ書き出す。 |
| `to_jkf()` | なし | `str` | JKF（JSON）形式の文字列へ変換する。 |
| `write_jkf(path, encoding="utf-8")` | `path: str \| PathLike`, `encoding: str = "utf-8"` | `None` | JKF ファイルへ書き出す。 |
| `to_usi_position(*, include_special_tokens=False)` | `include_special_tokens: bool = False` | `str` | USI `position` コマンド文字列へ変換する。`include_special_tokens=True` の場合は終局特殊手トークンを末尾に付加する。 |
| `to_pack()` | なし | `bytes` | 本手順を `pack` バイナリへ変換する。全手に `engine_info.eval` が設定されている必要がある。 |
| `to_sbinpack(stem_score=0, include_main=True, include_variations=False, metadata=None)` | `stem_score: int = 0`, `include_main: bool = True`, `include_variations: bool = False`, `metadata: bytes \| None = None` | `bytes` | `sbinpack` v2 バイナリへ変換する。出力対象の全手に `engine_info.eval` が必要。 |
| `to_psv(*, include_main=True, include_variations=False)` | `include_main: bool = True`, `include_variations: bool = False` | `list[bytes]` | 各局面を 40 バイトの `PackedSfenValue` のリストとして出力する。出力対象の全手に `engine_info.eval` が必要。 |

### 棋譜フォーマット入出力の仕様と互換性

- **KIF / KI2 の変化手順とコメント**：
  - 変化手順と終局要約行が両方存在する場合、`to_kif()` および `to_ki2()` は変化ブロックを先に出力し、本手順の `まで...` 要約行を最後に出力する。
  - 連続する `*` コメント行は `\n` で連結され、直前の指し手の `MoveEntry.comment`（初手より前の場合は `Record.initial_comment`）に格納される。
- **CSA 2.2 / 3.0 の仕様**：
  - `from_csa_str()` / `from_csa_file()` は CSA 仕様に従い `'*` で始まる行を指し手コメントとして読み込み、単一の `'` で始まる行は注釈として読み飛ばす。
  - `'** <評価値> <読み筋> #<ノード数>` 形式のエンジン評価行は、`MoveEntry.engine_info` の `eval`、`nodes`、および `extras["csa_pv"]` に格納され、`to_csa()` で再出力される。
  - `to_csa(version="3.0")` は先頭に `'CSA encoding=UTF-8` 宣言を出力し、ミリ秒単位の端数がある消費時間を小数表記（例：`T15.123`）で出力する。`write_csa(..., version="3.0")` では指定した `encoding`（UTF-8 または Shift_JIS）に合わせてエンコーディング宣言を出力し、CSA が宣言できない文字コードが指定された場合は `ValueError` を送出する。

```python
from rsshogi.core import Board
from rsshogi.record import Record

record = Record.from_usi_main_line(
    "startpos",
    ["7g7f", "3c3d", "2g2f"],
    evals=[30, -25, 40],
    move_times_ms=[1000, 1200, 800],
    result="WHITE_WIN",
)

print(record.move_count)  # => 3
print(record.result.name)  # => "WHITE_WIN"

# 盤面上で本手順を再生する
board = Board(record.init_position_sfen)
for entry in record.moves:
    board.apply_move(entry.move)
```

---

## 分岐（変化手順）の走査と `RecordEditor`

KIF、KI2、JKF、および `sbinpack` の変化手順を含む棋譜は、内部で木構造として保持される。
各ノードは整数型のノード ID（`RecordNodeId = int`）で識別される。

### `Record` の木構造参照メソッド

| メソッド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `root_node_id()` | なし | `int` | 開始局面を表すルートノードの ID（常に `0`）を返す。 |
| `main_line_ids()` | なし | `list[int]` | 本手順に属する通常手ノードの ID リストを返す。 |
| `children(node_id)` | `node_id: int` | `list[int]` | 指定ノードの子ノード ID リストを返す。先頭要素（`children[0]`）が本手順側の枝を表す。 |
| `node_parent(node_id)` | `node_id: int` | `int \| None` | 親ノードの ID を返す。ルートノードの場合は `None` を返す。 |
| `node_entry(node_id)` | `node_id: int` | `RecordEntry \| None` | 指定ノードが保持する `RecordEntry` を返す。ルートノードの場合は `None` を返す。 |
| `node_move(node_id)` | `node_id: int` | `MoveEntry \| None` | 指定ノードが通常手であれば `MoveEntry` を返し、ルートまたは終局特殊手ノードであれば `None` を返す。 |
| `node_terminal(node_id)` | `node_id: int` | `SpecialMoveEntry \| None` | 指定ノードが終局特殊手であれば `SpecialMoveEntry` を返し、それ以外は `None` を返す。 |
| `into_editor()` | なし | `RecordEditor` | 現在局面を追跡しながら分岐を追加および移動できる `RecordEditor` を生成する。 |

### `RecordEntry`

`Record.node_entry(node_id)` が返すノード要素のラッパークラスである。

| プロパティ / メソッド | 型 | 説明 |
|---|---|---|
| `kind` | `str` | 通常手なら `"MOVE"`、終局特殊手なら `"SPECIAL"`。 |
| `move` | `MoveEntry \| None` | 通常手ノードの場合は `MoveEntry`、終局特殊手ノードの場合は `None`。 |
| `terminal` | `SpecialMoveEntry \| None` | 終局特殊手ノードの場合は `SpecialMoveEntry`、通常手ノードの場合は `None`。 |
| `is_move()` | `bool` | 通常手ノードであれば `True` を返す。 |
| `is_terminal()` | `bool` | 終局特殊手ノードであれば `True` を返す。 |

### `RecordEditor`

`record.into_editor()` で生成するカーソル付きの棋譜エディタである。
カーソル位置の盤面状態を内部で管理するため、追加する指し手の合法性を検証しながら分岐を構築できる。
編集完了後は `into_record()` で `Record` を取り出す（`into_record()` 呼び出し後のエディタに対する操作は `ValueError` を送出する）。

| メソッド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `current_node_id()` | なし | `int` | 現在のカーソルが指すノード ID を返す。 |
| `current_sfen()` | なし | `str` | カーソル位置の局面の SFEN 文字列を返す。 |
| `current_board()` | なし | `Board` | カーソル位置の盤面のコピーを `Board` として返す。 |
| `at_root()` | なし | `bool` | カーソルがルートノードにあれば `True` を返す。 |
| `at_end()` | なし | `bool` | カーソル位置のノードに子ノードが存在しなければ `True` を返す。 |
| `current_move()` | なし | `MoveEntry \| None` | カーソル位置の `MoveEntry`（ルートまたは終局ノードでは `None`）を返す。 |
| `current_terminal()` | なし | `SpecialMoveEntry \| None` | カーソル位置の `SpecialMoveEntry`（通常手またはルートでは `None`）を返す。 |
| `variations()` | なし | `list[int]` | カーソル位置のノードが持つ子ノード ID のリストを返す。 |
| `go_to(node_id)` | `node_id: int` | `None` | 指定したノード ID へカーソルを移動し、盤面を再構築する。 |
| `go_to_start()` | なし | `None` | ルートノードへカーソルを移動する。 |
| `go_to_main_end()` | なし | `None` | 本手順の末尾ノードへカーソルを移動する。 |
| `go_back()` | なし | `bool` | 親ノードへ 1 手戻る。移動できた場合は `True`、すでにルートの場合は `False` を返す。 |
| `go_forward(child_index=0)` | `child_index: int = 0` | `bool` | 指定インデックスの子ノードへ 1 手進む。子が存在しない場合は `False` を返す。 |
| `branch_to(node_id)` | `node_id: int` | `None` | 現在のノードの子ノードのうち、`node_id` と一致する枝へ 1 手進む。 |
| `append_move(move, *, time_ms=None, comment=None, engine_info=None)` | `move: MoveEntry \| Move32 \| Move \| int \| str` ほか | `int` | 現在位置の子として通常手を追加し、カーソルを新しいノードへ進めてそのノード ID を返す。 |
| `append_special(terminal)` | `terminal: SpecialMoveEntry \| GameResult \| int \| str` | `int` | 現在位置の子として終局特殊手を追加し、カーソルを新しいノードへ進めてそのノード ID を返す。 |
| `into_record()` | なし | `Record` | 編集結果の `Record` を取り出し、エディタを終了する。 |

```python
from rsshogi.record import Record

record = Record.from_usi_position("position startpos moves 7g7f 3c3d")
editor = record.into_editor()

# 1 手目（7g7f）の直後に戻って変化手順（8c8d）を追加する
editor.go_to_main_end()
editor.go_back()
variation_node_id = editor.append_move("8c8d", comment="飛車先を突く変化")
editor.append_move("2g2f")

edited_record = editor.into_record()
print(len(edited_record.children(edited_record.main_line_ids()[0])))  # => 2
```

---

## 辞書変換（`from_dict` / `to_dict`）と `TypedDict` 型

`Record.from_dict(data, *, strict=False)` と `record.to_dict()` は、JSON シリアライズや外部スキーマとの変換に適した辞書構造を扱う。
型ヒント用に `RecordDict`、`RecordMetadataDict`、`MoveEntryDict`、`EngineInfoDict`、`TimeControlDict`、`GameResultInfoDict` の 6 つの `TypedDict` が `rsshogi.record` から提供されている。

### `from_dict` の入力スキーマ

- **`init_position_sfen`**（`str`、必須）：初期局面の SFEN 文字列。
- **`initial_comment`**（`str | None`、任意）：開始局面コメント。
- **`moves`**（`Sequence[Mapping[str, Any]]`、任意）：本手順の指し手リスト。各要素は以下のキーを持つ。
  - `move`（必須）：`MoveEntry`、`Move32`、`Move`、`int`（`0..=65535` の 16 ビット `Move` 値、範囲外は `ValueError`）、または USI 文字列。各手は盤面上で合法性を検証する。
  - `time_ms`（`int | None`、任意）：消費時間（ミリ秒）。
  - `comment`（`str | None`、任意）：指し手コメント。
  - `engine_info`（`Mapping[str, Any] | None`、任意）：`eval`、`depth`、`nodes`、`seldepth`、`extras`（`dict[str, str | int | float | bool]`）を保持する辞書。
- **`result`**（`Mapping[str, Any] | GameResult | int | str | None`、任意）：終局情報。辞書を渡す場合は `result` キー（`GameResult`、数値コード、または名前文字列）が必須であり、任意で `ply_count`、`reason`、`end_time_ms`、`end_comment` を指定できる。
- **`metadata`**（`Mapping[str, Any] | None`、任意）：後述する `RecordMetadata` の各フィールドに対応する辞書。

### `strict` モードと `permissive` モードの違い

- **`strict=False`（既定）**：`metadata` 内の未知キー（値が `str` または `None` のもの）を自動的に `metadata.attributes` に格納する。
- **`strict=True`**：トップレベル、`moves[*]`、`result`、`metadata` のいずれかに未知のキーが含まれている場合に `ValueError` を送出する。

---

## `RecordMetadata` / `RecordMetadataKey` / `TimeControl`

### `RecordMetadata`

```python
RecordMetadata(
    event: str | None = None,
    site: str | None = None,
    black_player: str | None = None,
    white_player: str | None = None,
    game_name: str | None = None,
    game_type: str | None = None,
    time_control: TimeControl | str | None = None,
    black_time_control: TimeControl | str | None = None,
    white_time_control: TimeControl | str | None = None,
    max_moves: int | None = None,
    impasse_rule: str | None = None,
    start_date: str | None = None,
    end_date: str | None = None,
    updated_date: str | None = None,
    comment: str | None = None,
    attributes: Mapping[str, str] | None = None,
) -> RecordMetadata
```

KIF、KI2、CSA、JKF の対局ヘッダ情報を保持するクラスである。すべてのプロパティは読み書き可能であり、未設定時は `None`（`attributes` は空辞書 `{}`）となる。

| プロパティ | 型 | 説明 | 対応する主なヘッダ項目 |
|---|---|---|---|
| `event` | `str \| None` | 棋戦名。 | KIF `棋戦：` / CSA `$EVENT:` |
| `site` | `str \| None` | 対局場所。 | KIF `場所：` / CSA `$SITE:` |
| `black_player` | `str \| None` | 先手（下手）の対局者名。 | KIF `先手：` / CSA `N+` |
| `white_player` | `str \| None` | 後手（上手）の対局者名。 | KIF `後手：` / CSA `N-` |
| `game_name` | `str \| None` | 対局名。 | JKF / カスタムヘッダ `game_name` |
| `game_type` | `str \| None` | 対局種別。 | JKF / カスタムヘッダ `game_type` |
| `time_control` | `TimeControl \| None` | 共通の持ち時間設定（`TimeControl` または spec 文字列を代入可能）。 | KIF `持ち時間：` / CSA `$TIME_LIMIT:` |
| `black_time_control` | `TimeControl \| None` | 先手個別の持ち時間設定。 | KIF `先手持ち時間：` / CSA `$TIME+:` |
| `white_time_control` | `TimeControl \| None` | 後手個別の持ち時間設定。 | KIF `後手持ち時間：` / CSA `$TIME-:` |
| `max_moves` | `int \| None` | 最大手数規定。 | KIF `最大手数：` / CSA `$MAX_MOVES:` |
| `impasse_rule` | `str \| None` | 持将棋および入玉規定。 | KIF `持将棋：` / CSA `$JISHOGI:` |
| `start_date` | `str \| None` | 対局開始日時。 | KIF `開始日時：` / CSA `$START_TIME:` |
| `end_date` | `str \| None` | 対局終了日時。 | KIF `終了日時：` / CSA `$END_TIME:` |
| `updated_date` | `str \| None` | 更新日時。 | カスタムヘッダ `updated_date` |
| `comment` | `str \| None` | ヘッダの備考欄。 | KIF `備考：` / CSA `$NOTE:` |
| `attributes` | `dict[str, str]` | 第一級フィールド以外の任意の文字列属性。 | 任意の拡張ヘッダ |

`attributes` を個別に操作するメソッドとして `set_attribute(key, value)`、`remove_attribute(key)`、`clear_attributes()` を備える。

### `RecordMetadataKey`

メタデータの正規キー文字列を定数としてまとめたクラスである。

| 定数 | 文字列値 |
|---|---|
| `RecordMetadataKey.EVENT` | `"event"` |
| `RecordMetadataKey.SITE` | `"site"` |
| `RecordMetadataKey.BLACK_PLAYER` | `"black_player"` |
| `RecordMetadataKey.WHITE_PLAYER` | `"white_player"` |
| `RecordMetadataKey.GAME_NAME` | `"game_name"` |
| `RecordMetadataKey.GAME_TYPE` | `"game_type"` |
| `RecordMetadataKey.START_DATE` | `"start_date"` |
| `RecordMetadataKey.END_DATE` | `"end_date"` |
| `RecordMetadataKey.UPDATED_DATE` | `"updated_date"` |
| `RecordMetadataKey.COMMENT` | `"comment"` |

### `TimeControl`

```python
TimeControl(base_seconds: int, byoyomi_seconds: int, increment_seconds: int) -> TimeControl
```

持ち時間、秒読み、1 手ごとの加算時間（フィッシャールール）を秒単位で保持するイミュータブルなクラスである。

| プロパティ / メソッド | 型 | 説明 |
|---|---|---|
| `base_seconds` | `int` | 基本持ち時間（秒）。 |
| `byoyomi_seconds` | `int` | 秒読み（秒）。 |
| `increment_seconds` | `int` | 1 手ごとの加算時間（秒）。 |
| `to_spec()` / `str(tc)` | `str` | `"{base}+{byoyomi}+{increment}"` 形式（例：`"600+30+0"`）の文字列を返す。 |
| `TimeControl.from_spec(spec)` | `TimeControl` | `"600+30+0"` 形式の文字列から `TimeControl` を生成する（クラスメソッド）。 |
| `TimeControl.normalize_spec(spec)` | `str` | `"300.0+10+0"` のような表記を `"300+10+0"` に正規化する（静的メソッド）。 |

---

## `MoveEntry` / `EngineInfo` / `SpecialMoveEntry`

### `MoveEntry`

```python
MoveEntry(
    move: Move | Move32 | int | str,
    *,
    time_ms: int | None = None,
    comment: str | None = None,
    engine_info: EngineInfo | None = None,
) -> MoveEntry
```

棋譜内の 1 手分の情報（指し手、消費時間、コメント、エンジン解析情報）を保持するクラスである。
`move` には `Move`、`Move32`、`int`（`0..=65535`）、または USI 文字列を指定でき、内部では 16 ビットの `Move` として保持される。

| プロパティ | 型 | 書込 | 説明 |
|---|---|:---:|---|
| `move` | `Move` | 不可 | 16 ビットの指し手オブジェクト。 |
| `time_ms` | `int \| None` | 可 | 消費時間（ミリ秒）。 |
| `comment` | `str \| None` | 可 | 指し手に対するコメント。 |
| `engine_info` | `EngineInfo \| None` | 可 | エンジン解析情報。 |

### `EngineInfo`

```python
EngineInfo(
    eval: int | None = None,
    depth: int | None = None,
    nodes: int | None = None,
    seldepth: int | None = None,
    wall_time_ms: int | None = None,
    latency_delta_ms: int | None = None,
    extras: Mapping[str, str | int | float | bool] | None = None,
) -> EngineInfo
```

各指し手に紐づく将棋エンジンの探索情報を保持するクラスである。

| プロパティ / メソッド | 型 | 書込 | 説明 |
|---|---|:---:|---|
| `eval` | `int \| None` | 可 | 着手前局面の手番側視点での評価値（センチポーン単位）。 |
| `depth` | `int \| None` | 可 | 探索深さ。 |
| `nodes` | `int \| None` | 可 | 探索ノード数。 |
| `seldepth` | `int \| None` | 可 | 選択探索深さ。 |
| `wall_time_ms` | `int \| None` | 可 | 実経過時間（ミリ秒）。`extras["wall_time_ms"]` または `extras["arena.wall_time_ms"]` に対応する型付きアクセサ。 |
| `latency_delta_ms` | `int \| None` | 可 | レイテンシ差分（ミリ秒）。`extras["latency_delta_ms"]` または `extras["arena.latency_delta_ms"]` に対応する型付きアクセサ。 |
| `extras` | `dict[str, str \| int \| float \| bool]` | 可 | 任意の拡張テレメトリ辞書。 |
| `set_extra(key, value)` | `None` | - | `extras[key]` を追加または更新する。 |
| `remove_extra(key)` | `str \| int \| float \| bool \| None` | - | `extras` から指定キーを削除し、以前の値を返す。 |
| `clear_extras()` | `None` | - | `extras` をすべて消去する。 |

### `SpecialMoveEntry`

```python
SpecialMoveEntry(
    kind: str,
    result: GameResult | int | str,
    unknown_name: str | None = None,
    comment: str | None = None,
    time_ms: int | None = None,
    raw: str | None = None,
) -> SpecialMoveEntry

SpecialMoveEntry.from_result(
    result: GameResult | int | str,
    time_ms: int | None = None,
    comment: str | None = None,
    raw: str | None = None,
) -> SpecialMoveEntry
```

投了、千日手、持将棋、入玉宣言勝ちなどの終局特殊手ノードを表すクラスである。
`SpecialMoveEntry.from_result(...)` を使うと、`GameResult` から対応する `kind` を自動導出して生成できる（`GameResult.INVALID` は終局状態ではないため `ValueError` を送出する）。

| プロパティ | 型 | 書込 | 説明 |
|---|---|:---:|---|
| `kind` | `str` | 不可 | 終局種別文字列（下表の 16 種類のいずれか）。 |
| `result` | `GameResult` | 不可 | 対応する終局結果。 |
| `unknown_name` | `str \| None` | 不可 | `kind == "UNKNOWN"` の場合に保持される元の終局文字列。 |
| `comment` | `str \| None` | 可 | 終局ノードに付与されたコメント。 |
| `time_ms` | `int \| None` | 可 | 終局時の消費時間（ミリ秒）。 |
| `raw` | `str \| None` | 可 | 棋譜ファイル内の元の終局行テキスト。 |

#### `SpecialMoveEntry.kind` の値一覧

| `kind` の値 | 意味 | 対応する主な棋譜表記 |
|---|---|---|
| `"INTERRUPT"` | 中断 | KIF `中断` / CSA `%CHUDAN` |
| `"RESIGN"` | 投了 | KIF `投了` / CSA `%TORYO` |
| `"MAX_MOVES"` | 最大手数到達 | KIF `最大手数` |
| `"IMPASSE"` | 持将棋 | KIF `持将棋` / CSA `%JISHOGI` |
| `"DRAW"` | 引き分け | 結果が裁定なら KIF / KI2 `裁定引分`（独自拡張）、持将棋なら `持将棋` / CSA `%HIKIWAKE` |
| `"REPETITION_DRAW"` | 千日手 | KIF `千日手` / CSA `%SENNICHITE` |
| `"MATE"` | 詰み | KIF `詰み` / CSA `%TSUMI` |
| `"NO_MATE"` | 不詰 | KIF `不詰` / CSA `%FUZUMI` |
| `"TIMEOUT"` | 切れ負け | KIF `切れ負け` / CSA `%TIME_UP` |
| `"WIN_BY_ILLEGAL_MOVE"` | 反則勝ち | KIF `反則勝ち` / CSA `%ILLEGAL_MOVE`, `%+ILLEGAL_ACTION`, `%-ILLEGAL_ACTION` |
| `"LOSE_BY_ILLEGAL_MOVE"` | 反則負け | KIF `反則負け` |
| `"WIN_BY_DECLARATION"` | 入玉宣言勝ち | KIF `入玉勝ち` / CSA `%KACHI` |
| `"WIN_BY_DEFAULT"` | 不戦勝 | KIF `不戦勝` |
| `"LOSE_BY_DEFAULT"` | 不戦敗 | KIF `不戦敗` |
| `"TRY"` | トライルール勝利 | KIF `トライ` |
| `"UNKNOWN"` | 未定義の終局表記 | `unknown_name` に元文字列を保持 |

---

## `GameResult` と `GameResultInfo`

### `GameResult`

```python
GameResult(value: int) -> GameResult
GameResult.from_str(name: str) -> GameResult
```

対局の勝敗結果と決着理由を表す列挙ライクなイミュータブルクラスである。

| 定数 | 値 (`value`) | 名前 (`name`) | 意味 |
|---|---:|---|---|
| `GameResult.BLACK_WIN` | `0` | `"BLACK_WIN"` | 先手勝ち（一般、投了、詰みなど）。 |
| `GameResult.WHITE_WIN` | `1` | `"WHITE_WIN"` | 後手勝ち（一般、投了、詰みなど）。 |
| `GameResult.DRAW_BY_REPETITION` | `2` | `"DRAW_BY_REPETITION"` | 千日手による引き分け。 |
| `GameResult.ERROR` | `3` | `"ERROR"` | エラーによる終了。 |
| `GameResult.BLACK_WIN_BY_DECLARATION` | `4` | `"BLACK_WIN_BY_DECLARATION"` | 入玉宣言による先手勝ち。 |
| `GameResult.WHITE_WIN_BY_DECLARATION` | `5` | `"WHITE_WIN_BY_DECLARATION"` | 入玉宣言による後手勝ち。 |
| `GameResult.DRAW_BY_MAX_PLIES` | `6` | `"DRAW_BY_MAX_PLIES"` | 最大手数到達による引き分け。 |
| `GameResult.INVALID` | `7` | `"INVALID"` | 未決着または結果なし。 |
| `GameResult.BLACK_WIN_BY_FORFEIT` | `8` | `"BLACK_WIN_BY_FORFEIT"` | 不戦勝による先手勝ち。 |
| `GameResult.WHITE_WIN_BY_FORFEIT` | `9` | `"WHITE_WIN_BY_FORFEIT"` | 不戦勝による後手勝ち。 |
| `GameResult.DRAW_BY_IMPASSE` | `10` | `"DRAW_BY_IMPASSE"` | 持将棋による引き分け。 |
| `GameResult.PAUSED` | `11` | `"PAUSED"` | 中断。 |
| `GameResult.BLACK_WIN_BY_ILLEGAL_MOVE` | `12` | `"BLACK_WIN_BY_ILLEGAL_MOVE"` | 後手の反則手による先手勝ち。 |
| `GameResult.WHITE_WIN_BY_ILLEGAL_MOVE` | `13` | `"WHITE_WIN_BY_ILLEGAL_MOVE"` | 先手の反則手による後手勝ち。 |
| `GameResult.DRAW_BY_ADJUDICATION` | `14` | `"DRAW_BY_ADJUDICATION"` | 裁定による引き分け。規則上の引き分けが成立していない局面で、外部の判断により引き分けとしたもの。 |
| `GameResult.BLACK_WIN_BY_TIMEOUT` | `16` | `"BLACK_WIN_BY_TIMEOUT"` | 後手の時間切れによる先手勝ち。 |
| `GameResult.WHITE_WIN_BY_TIMEOUT` | `17` | `"WHITE_WIN_BY_TIMEOUT"` | 先手の時間切れによる後手勝ち。 |
| `GameResult.BLACK_WIN_BY_TRY_RULE` | `20` | `"BLACK_WIN_BY_TRY_RULE"` | トライルールによる先手勝ち。 |
| `GameResult.WHITE_WIN_BY_TRY_RULE` | `21` | `"WHITE_WIN_BY_TRY_RULE"` | トライルールによる後手勝ち。 |

> [!NOTE]
> 数値コードは `14` の次が `16`、`17` の次が `20` と飛び番になっている。全定数の辞書は `GameResult.__members__` から参照できる。

| プロパティ / メソッド | 型 | 説明 |
|---|---|---|
| `value` / `int(r)` | `int` | 数値コード。 |
| `name` / `str(r)` | `str` | 定数名（例：`"BLACK_WIN"`）。 |
| `GameResult.from_str(name)` | `GameResult` | 名前文字列（大文字小文字不問）から `GameResult` を返す。 |
| `is_black_win()` | `bool` | 先手勝ちの種別であれば `True` を返す。 |
| `is_white_win()` | `bool` | 後手勝ちの種別であれば `True` を返す。 |
| `is_draw()` | `bool` | 引き分けの種別（`2`, `6`, `10`, `14`）であれば `True` を返す。 |
| `is_win()` | `bool` | 先手または後手の勝利が決まっていれば `True` を返す。 |
| `is_win_by_declaration()` | `bool` | 入玉宣言勝ち（`4`, `5`）であれば `True` を返す。 |
| `is_win_by_try_rule()` | `bool` | トライルール勝ち（`20`, `21`）であれば `True` を返す。 |

### 裁定引き分けの保存

`GameResult.DRAW_BY_ADJUDICATION` は数値 `14` の裁定引き分けである。
`SpecialMoveEntry.from_result(...)` はこれを `kind == "DRAW"` の終局特殊手に変換する。
保存形式によって、読み戻した結果が次のように変わる。

| 保存形式 | 読み戻した結果 | 保持される情報 |
|---|---|---|
| SAZ2 v4 / SBINPACK | `DRAW_BY_ADJUDICATION` | 裁定引き分けという結果。 |
| HCPE | HCPE の `Draw`（数値 `0`） | 引き分け。HCPE は独自の結果コードを使い、理由を保持しない。 |
| PACK / USI 特殊トークン | `DRAW_BY_REPETITION` | 引き分け。裁定という理由は失われる。 |
| CSA / JKF | `DRAW_BY_IMPASSE` | 引き分け。`HIKIWAKE` は持将棋の結果として読み込まれる。 |
| KIF / KI2 | `DRAW_BY_ADJUDICATION` | 独自拡張の終局表記 `裁定引分` により、裁定による引き分けを保持する。 |

裁定理由の保持には SAZ2 v4、SBINPACK、または rsshogi の KIF / KI2 拡張表記を使う。
KIF / KI2 は終局特殊手の `裁定引分` と終局サマリーの `まで○手で裁定引分` を `DRAW_BY_ADJUDICATION` として読み込む。
`裁定引分` は [KIF 形式の公式仕様](https://kakinoki.o.oo7.jp/kif_format.html)に定義された終局表記ではない。
ShogiHome が使用する [tsshogi 2.3.4 の実装](https://github.com/sunfish-shogi/tsshogi/blob/bec83166c011eb662ee963f04ddb3fe2eb584c99/src/kakinoki.ts)は未知の終局理由を文字列として読み書きするが、裁定引分を引き分けとして分類する対応はない。
他ソフトとの受け渡しでは、拡張表記の読み込みと結果の分類を確認する必要がある。

CSA `%HIKIWAKE` と JKF `HIKIWAKE` の読み込み結果は、`kind == "DRAW"`、`result == DRAW_BY_IMPASSE` である。
これを KIF / KI2 に変換する場合は、結果に従って `持将棋` を出力し、読み戻しても `DRAW_BY_IMPASSE` を保持する。
`SpecialMoveEntry("DRAW", GameResult.DRAW_BY_IMPASSE)` を直接指定した場合も同じである。

旧版が出力した `引き分け` は裁定と持将棋を区別する情報を持たないため、裁定引分としては読み込まない。
KIF の終局行は未知の特殊手（`kind == "UNKNOWN"`、結果は `PAUSED`）、終局サマリーだけの場合は結果未設定（`INVALID`）となる。
KI2 の終局サマリーだけの場合は中断（`kind == "INTERRUPT"`、結果は `PAUSED`）となる。
旧表記の結果を復元するには、生成元の対局情報で理由を確認する必要がある。

### `GameResultInfo`

`record.result_info` が返す終局情報のイミュータブルなスナップショットである。

| プロパティ | 型 | 説明 |
|---|---|---|
| `result` | `GameResult` | 終局結果。 |
| `ply_count` | `int` | 本手順の指し手数。 |
| `reason` | `str \| None` | 終局理由の原文文字列。 |
| `end_time_ms` | `int \| None` | 終局特殊手の消費時間（ミリ秒）。 |
| `end_comment` | `str \| None` | 終局特殊手のコメント。 |

---

## ビルダークラス（`RecordBuilder` / `RecordMetadataBuilder`）

メソッドチェーンまたは段階的な設定で `Record` や `RecordMetadata` を組み立てるビルダークラスである。

### `RecordBuilder`

```python
RecordBuilder(init_position_sfen: str | None = None) -> RecordBuilder
```

`init_position_sfen=None` の場合は平手初期局面で初期化される。
`build()` を呼ぶと内部状態から `Record` を生成し、ビルダー自身は空の初期状態にリセットされる。

| メソッド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `set_initial_position(sfen)` | `sfen: str` | `None` | 初期局面の SFEN を設定し、既存の指し手ツリーをリセットする。 |
| `set_initial_comment(comment)` | `comment: str \| None` | `None` | 開始局面コメントを設定する。 |
| `set_metadata(metadata)` | `metadata: RecordMetadata \| Mapping[str, Any]` | `None` | メタデータを設定する。 |
| `update_metadata(patch, *, strict=False)` | `patch: RecordMetadata \| Mapping[str, Any]`, `strict: bool = False` | `None` | メタデータを部分更新する。 |
| `add_move(move, *, time_ms=None, comment=None, engine_info=None)` | `move: MoveEntry \| Move32 \| Move \| int \| str` ほか | `int` | 本手順の末尾に 1 手追加し、ノード ID を返す。 |
| `extend_moves(moves)` | `moves: Sequence[MoveEntry \| Move32 \| Move \| int \| str]` | `list[int]` | 本手順の末尾に複数手を追加し、ノード ID のリストを返す。 |
| `set_terminal(terminal)` | `terminal: SpecialMoveEntry \| GameResult \| int \| str` | `int` | 本手順の末尾に終局特殊手を設定し、ノード ID を返す。 |
| `build()` | なし | `Record` | 構築した `Record` を返す。 |

### `RecordMetadataBuilder`

```python
RecordMetadataBuilder() -> RecordMetadataBuilder
```

`set_event(v)`、`set_site(v)`、`set_black_player(v)`、`set_white_player(v)`、`set_game_name(v)`、`set_game_type(v)`、`set_time_control(v)`、`set_black_time_control(v)`、`set_white_time_control(v)`、`set_max_moves(v)`、`set_impasse_rule(v)`、`set_start_date(v)`、`set_end_date(v)`、`set_updated_date(v)`、`set_comment(v)`、`set_attribute(key, value)`、`remove_attribute(key)`、`clear_attributes()` を提供し、`build() -> RecordMetadata` で `RecordMetadata` を生成する。

---

## 複数棋譜のバイナリ一括入出力（`pack` / `sbinpack`）

複数局の棋譜をまとめて 1 つのバイナリ列またはファイルに読み書きするモジュール関数を提供する。
バイナリ構造の仕様は [棋譜フォーマット一覧](../reference/formats/index.md) を参照。

### `sbinpack` 形式の一括読み書き

| 関数 | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `write_sbinpack(records, *, stem_score=0, include_main=True, include_variations=False, metadatas=None)` | `records: Sequence[Record]` ほか | `bytes` | 複数の `Record` を `sbinpack` v2 形式にシリアライズする。`metadatas` を指定する場合は `records` と同じ長さの `Sequence[bytes \| None]`（各要素 0 から 127 バイト）を渡す。 |
| `write_sbinpack_file(path, records, *, stem_score=0, include_main=True, include_variations=False, metadatas=None)` | `path: str \| PathLike`, `records: Sequence[Record]` ほか | `None` | 複数の `Record` を `sbinpack` ファイルへ書き出す。 |
| `decode_sbinpack(data)` | `data: bytes \| bytearray` | `list[tuple[Record, bytes]]` | `sbinpack` バイナリから `(Record, metadata_bytes)` のリストを復元する。 |
| `decode_sbinpack_file(path)` | `path: str \| PathLike` | `list[tuple[Record, bytes]]` | `sbinpack` ファイルから `(Record, metadata_bytes)` のリストを復元する。 |

### `pack` 形式の一括読み書き

| 関数 | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `write_pack(records)` | `records: Sequence[Record]` | `bytes` | 複数の `Record` の本手順を `pack` 形式にシリアライズする。 |
| `write_pack_file(path, records)` | `path: str \| PathLike`, `records: Sequence[Record]` | `None` | 複数の `Record` を `pack` ファイルへ書き出す。 |
| `decode_pack(data)` | `data: bytes \| bytearray` | `list[Record]` | `pack` バイナリから `list[Record]` を復元する。 |
| `decode_pack_file(path)` | `path: str \| PathLike` | `list[Record]` | `pack` ファイルから `list[Record]` を復元する。 |

```python
from rsshogi.record import Record, decode_sbinpack, write_sbinpack

record = Record.from_usi_main_line(
    "startpos",
    ["7g7f", "3c3d"],
    evals=[20, -15],
    result="BLACK_WIN",
)

blob = write_sbinpack([record], metadatas=[b"game-001"])
restored_list = decode_sbinpack(blob)
restored_record, meta_bytes = restored_list[0]
assert meta_bytes == b"game-001"
assert restored_record.move_count == 2
```

## 関連項目

- [盤面管理（`Board`）](board.md)
- [指し手クラス（`Move` / `Move32`）](move.md)
- [NumPy 連携](numpy.md)
- [USI プロトコル](usi.md)
- [棋譜フォーマット一覧](../reference/formats/index.md)
