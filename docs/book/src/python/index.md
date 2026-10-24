# Python API リファレンス

`rsshogi` は Rust 製将棋ライブラリの Python バインディングである。
高速な盤面操作、合法手生成、複数形式の棋譜読み書き、定跡検索、機械学習向けバイナリ変換を型安全な API として提供する。

## このリファレンスの読み方

各ページは共通の記法規則で記述されている。

- **シグネチャ**：Python の型注釈付きコードブロックで示す（例：`board.to_sfen() -> str`）。
- **一覧表と詳細解説**：各クラスのメソッドやプロパティを機能別の表で整理し、引数や戻り値の詳細が重要なメソッドは個別の節とコード例で解説する。
- **例外方針**：多くの API は共通の例外規則に従う。
  - **型が不正**：`TypeError` を送出する。
  - **値は型として正しいが内容が不正**（不正な SFEN や USI 文字列、非合法手、範囲外の整数など）：`ValueError` を送出する。
  - **履歴や検索結果が空**：`None` を返す（例：`board.last_move()`、`board.pop()`、`book.get(key)`）。

## インストール

通常の環境では標準パッケージをインストールする。

```bash
pip install rsshogi
```

AVX2 対応の x86_64 CPU 環境では、SIMD 命令を有効化した高速版パッケージを選択できる。
通常版と高速版は同じ `rsshogi` モジュール名を提供するため、環境ごとにどちらか一方のみをインストールする。

```bash
pip install rsshogi-avx2
```

## モジュール構成

`rsshogi` は機能ごとのサブモジュールで構成されている。
なお `rsshogi.core` は利便性のため `rsshogi.types` の主要型（`Color`、`Piece`、`PieceType`、`Square`）も再エクスポートする。

| モジュール | 主な型と関数 | 説明 |
|---|---|---|
| [`rsshogi.core`](board.md) | `Board`, `PositionState`, `ValidationReport`, `ValidationIssue`, `Move`, `Move32`, `AperyMove`, `AperyMove32`, `UsiPositionParts`, `to_move()`, `parse_usi_position()`, `parse_usi_position_parts()`, `normalize_usi_position()` | 盤面管理、局面検証、指し手表現、USI 局面文字列の解析を行う。 |
| [`rsshogi.types`](types.md) | `Color`, `PieceType`, `Piece`, `Square`, `Bitboard`, `Hand`, `MoveType`, `RepetitionState` | 手番、駒、マス目、ビットボード、持ち駒などの基本型を提供する。 |
| [`rsshogi.record`](record.md) | `Record`, `RecordEditor`, `RecordMetadata`, `RecordMetadataKey`, `MoveEntry`, `SpecialMoveEntry`, `RecordEntry`, `RecordNodeId`, `EngineInfo`, `TimeControl`, `GameResult`, `GameResultInfo`, `RecordBuilder`, `RecordMetadataBuilder`, `decode_pack()`, `write_pack()`, `decode_sbinpack()`, `write_sbinpack()` | 棋譜の解析、編集、書き出し、および学習用バイナリ変換を行う。 |
| [`rsshogi.book`](book.md) | `NativeBook`, `NativeBookEditor`, `NativeBookBuilder`, `NativeScore`, `YaneuraOuBook`, `SbkBook` | 定跡の部分参照、編集、構築と外部形式の読込。 |
| [`rsshogi.usi`](usi.md) | `UsiInfo`, `UsiBestMove`, `UsiGoCommand`, `UsiScore`, `UsiBound`, `move_from_usi()`, `parse_info()`, `parse_bestmove()`, `parse_score()` | USI プロトコルの文字列解析とコマンド構築を行う。 |
| [`rsshogi.policy`](policy.md) | `move_label()`, `compact_move_label()`, `move_label_to_compact()`, `compact_move_label_to_move_label()`, `is_structurally_valid_move_label()`, `MOVE_LABEL_COUNT`, `COMPACT_MOVE_LABEL_COUNT` | 方策ネットワーク（Policy Network）向けの指し手ラベル変換を行う。 |
| [`rsshogi.sazpack`](sazpack.md) | `SazGame`, `SazPosition`, `SazPolicyEntry`, `SazWdl`, `write_sazpack()`, `write_sazpack_file()`, `decode_sazpack()`, `decode_sazpack_file()` | SAZ2 形式の自己対局教師データを読み書きする。 |
| [`rsshogi.initial_positions`](initial_positions.md) | `InitialPosition`, `HANDICAP_TO_SFEN`, `SFEN_TO_HANDICAP`, `handicap_to_sfen()`, `sfen_to_handicap()` | 平手および駒落ち初期局面の SFEN 定数と手合割変換を提供する。 |
| [`rsshogi.svg`](svg.md) | `Svg` | 盤面の SVG 画像出力と Jupyter Notebook 表示を行う。 |
| [`rsshogi.numpy`](numpy.md) | `PackedSfen`, `PackedSfenValue`, `HuffmanCodedPos`, `HuffmanCodedPosAndEval` | 学習データのバイナリ構造に対応する NumPy `dtype` を定義する。 |

### インポート例

```python
from rsshogi.core import (
    AperyMove,
    AperyMove32,
    Board,
    Move,
    Move32,
    PositionState,
    UsiPositionParts,
    ValidationIssue,
    ValidationReport,
    normalize_usi_position,
    parse_usi_position,
    parse_usi_position_parts,
    to_move,
)
from rsshogi.types import (
    Bitboard,
    Color,
    Hand,
    MoveType,
    Piece,
    PieceType,
    RepetitionState,
    Square,
)
from rsshogi.record import (
    EngineInfo,
    GameResult,
    GameResultInfo,
    MoveEntry,
    Record,
    RecordBuilder,
    RecordEditor,
    RecordMetadata,
    RecordMetadataBuilder,
    RecordMetadataKey,
    SpecialMoveEntry,
    TimeControl,
)
from rsshogi.book import (
    NativeBook, NativeBookEditor, NativeBookBuilder,
    NativeBookEntry, NativeBookPosition, NativeBookMetadata,
    NativeScore, NativeBound, NativeCandidate,
    SbkBook,
    YaneuraOuBook,
)
from rsshogi.usi import UsiBestMove, UsiBound, UsiGoCommand, UsiInfo, UsiScore
from rsshogi.policy import (
    COMPACT_MOVE_LABEL_COUNT,
    MOVE_LABEL_COUNT,
    compact_move_label,
    move_label,
)
from rsshogi.sazpack import (
    SazGame,
    SazPolicyEntry,
    SazPosition,
    SazWdl,
    decode_sazpack,
    write_sazpack,
)
from rsshogi.initial_positions import InitialPosition
from rsshogi.svg import Svg
from rsshogi.numpy import (
    HuffmanCodedPos,
    HuffmanCodedPosAndEval,
    PackedSfen,
    PackedSfenValue,
)
```

## クラスと型の一覧

### 盤面と局面検証

| クラス | モジュール | 説明 |
|---|---|---|
| [`Board`](board.md#局面の初期化と入出力) | `rsshogi.core` | 盤面状態、合法手生成、指し手の適用と取り消し、各種判定を管理する中核クラスである。 |
| [`PositionState`](board.md#positionstate) | `rsshogi.core` | 81 マスの駒配置、持ち駒、手番、手数を直接編集できる構造化オブジェクトである。 |
| [`ValidationReport`](board.md#validationreport) | `rsshogi.core` | `validate_all()` が返す局面検証レポートである。 |
| [`ValidationIssue`](board.md#validationissue) | `rsshogi.core` | 二歩や玉枚数異常などの個別検証エラーを表すオブジェクトである。 |

### 指し手表現

| クラス | モジュール | 説明 |
|---|---|---|
| [`Move`](move.md#move) | `rsshogi.core` | 移動元、移動先、成り、駒打ちを表す標準の 16 ビット指し手型である。 |
| [`Move32`](move.md#move32) | `rsshogi.core` | 16 ビット指し手に移動後の色付き駒情報を加えた 32 ビット指し手型である。 |
| [`AperyMove`](move.md#aperymove) | `rsshogi.core` | `cshogi` および Apery 互換ビットレイアウトの 16 ビット指し手型である。 |
| [`AperyMove32`](move.md#aperymove32) | `rsshogi.core` | 移動前駒種と取得駒種を含む `cshogi` および Apery 互換の 32 ビット指し手型である。 |

### 基本型

| クラス | モジュール | 説明 |
|---|---|---|
| [`Color`](types.md#color) | `rsshogi.types` | 先手（`BLACK`）と後手（`WHITE`）を表す手番型である。 |
| [`PieceType`](types.md#piecetype) | `rsshogi.types` | 歩から龍までの駒種を表す型である。 |
| [`Piece`](types.md#piece) | `rsshogi.types` | 手番（所有者）と駒種を組み合わせた色付き駒型である。 |
| [`Square`](types.md#square) | `rsshogi.types` | 盤上の 81 マスおよび `Square.NONE` を表す座標型である。 |
| [`Bitboard`](types.md#bitboard) | `rsshogi.types` | 81 マスの集合をビット演算で扱うビットボード型である。 |
| [`Hand`](types.md#hand) | `rsshogi.types` | 片方の手番の持ち駒枚数を管理する型である。 |
| [`MoveType`](types.md#movetype) | `rsshogi.types` | 通常移動、成り、駒打ちの区分を表す型である。 |
| [`RepetitionState`](types.md#repetitionstate) | `rsshogi.types` | 千日手や連続王手の千日手、優等局面、劣等局面の判定結果を表す型である。 |

### 棋譜と分岐編集

| クラス | モジュール | 説明 |
|---|---|---|
| [`Record`](record.md#record) | `rsshogi.record` | 初期局面、指し手ツリー、対局メタデータ、終局結果を保持する棋譜クラスである。 |
| [`RecordEditor`](record.md#recordeditor) | `rsshogi.record` | 局面カーソルを動かしながら本手順や分岐手順を編集するクラスである。 |
| [`RecordMetadata`](record.md#recordmetadata) | `rsshogi.record` | 対局者名、棋戦名、持ち時間、日時などのヘッダ情報を保持するクラスである。 |
| [`RecordMetadataKey`](record.md#recordmetadatakey) | `rsshogi.record` | 正規化されたメタデータキー名の文字列定数群である。 |
| [`MoveEntry`](record.md#moveentry) | `rsshogi.record` | 1 手分の指し手、消費時間、コメント、エンジン解析情報を保持するクラスである。 |
| [`SpecialMoveEntry`](record.md#specialmoveentry) | `rsshogi.record` | 投了、千日手、入玉宣言勝ちなどの終局特殊手を表すクラスである。 |
| [`RecordEntry`](record.md#recordentry) | `rsshogi.record` | ツリーノードが保持する通常手または終局特殊手の共用体ラッパである。 |
| [`RecordNodeId`](record.md#分岐変化手順の走査と-recordeditor) | `rsshogi.record` | 棋譜ツリー内のノード識別子を表す整数型エイリアス（`int`）である。 |
| [`EngineInfo`](record.md#engineinfo) | `rsshogi.record` | 評価値、探索深さ、ノード数、拡張テレメトリを保持するクラスである。 |
| [`TimeControl`](record.md#timecontrol) | `rsshogi.record` | 持ち時間、秒読み、フィッシャー加算秒を表すクラスである。 |
| [`GameResult`](record.md#gameresult) | `rsshogi.record` | 18 種類の対局結果を表す列挙型クラスである。 |
| [`GameResultInfo`](record.md#gameresultinfo) | `rsshogi.record` | 終局結果、手数、終局理由、終局消費時間、終局コメントを束ねた読み取り専用ビューである。 |
| [`RecordBuilder`](record.md#recordbuilder) | `rsshogi.record` | 本手順から `Record` を組み立てるビルダークラスである。 |
| [`RecordMetadataBuilder`](record.md#recordmetadatabuilder) | `rsshogi.record` | `RecordMetadata` を組み立てるビルダークラスである。 |

### 定跡データベース

`NativeScore`、`NativeBound`、`NativeCandidate`、`NativeBookEntry`、`NativeBookPosition`、`NativeBookMetadata` は評価、候補手、局面 snapshot と文書属性を保持する。

| クラス | モジュール | 説明 |
|---|---|---|
| [`NativeBook`](book.md#native-book-v4) | `rsshogi.book` | v4 の圧縮ブロック部分参照 reader。編集は `NativeBookEditor`、初期構築は `NativeBookBuilder` を使う。 |
| [`YaneuraOuBook`](book.md#やねうら王形式定跡yaneuraoubook) | `rsshogi.book` | やねうら王形式（DB2016 `.db`）の外部定跡ファイルを検索するリーダである。 |
| [`YaneuraOuBookEntry`](book.md#やねうら王形式定跡yaneuraoubook) | `rsshogi.book` | DB2016 定跡の 1 局面分のエントリである。 |
| [`YaneuraOuBookMove`](book.md#やねうら王形式定跡yaneuraoubook) | `rsshogi.book` | DB2016 定跡の 1 候補手データである。 |
| [`YaneuraOuBookDiagnostics`](book.md#やねうら王形式定跡yaneuraoubook) | `rsshogi.book` | DB2016 定跡ファイルのソート状態や検証結果を保持する診断型である。 |
| [`SbkBook`](book.md#sbk-形式定跡sbkbook) | `rsshogi.book` | SBK（`.sbk`）形式の外部定跡ファイルを参照するリーダである。 |
| [`SbkEntry`](book.md#sbk-形式定跡sbkbook) | `rsshogi.book` | SBK 定跡の 1 局面分の状態情報と候補手リストを保持する型である。 |
| [`SbkMove`](book.md#sbk-形式定跡sbkbook) | `rsshogi.book` | SBK 定跡の 1 候補手と遷移先情報である。 |
| [`SbkEval`](book.md#sbk-形式定跡sbkbook) | `rsshogi.book` | SBK 定跡に記録されたエンジン評価情報である。 |
| [`SbkDiagnostics`](book.md#sbk-形式定跡sbkbook) | `rsshogi.book` | SBK 定跡の重複局面数（`duplicate_positions`）と未解決遷移先数（`unresolved_next_states`）を報告する診断型である。 |
| [`SbkOpenProgress`](book.md#sbk-形式定跡sbkbook) | `rsshogi.book` | `SbkBook.open_with_control()` のコールバックに渡される進捗オブジェクトである。 |

### USI プロトコルとユーティリティ

| クラス / 定数 | モジュール | 説明 |
|---|---|---|
| [`UsiPositionParts`](usi.md#usipositionparts) | `rsshogi.core` | USI `position` 文字列を初期局面 SFEN、手順、最終局面 SFEN に分解した結果オブジェクトである。 |
| [`UsiInfo`](usi.md#usiinfo) | `rsshogi.usi` | USI `info` 行の解析結果を保持するデータクラスである。 |
| [`UsiBestMove`](usi.md#usibestmove) | `rsshogi.usi` | USI `bestmove` 行の指し手と予想応手を保持するデータクラスである。 |
| [`UsiGoCommand`](usi.md#usigocommand) | `rsshogi.usi` | USI `go` コマンド文字列を組み立てるデータクラスである。 |
| [`UsiScore`](usi.md#usiscore) | `rsshogi.usi` | センチポーン評価値と詰み手数エンコーディングを扱うイミュータブルなデータクラスである。 |
| [`UsiBound`](usi.md#usibound) | `rsshogi.usi` | 評価値の上界、下界、正確値を表す列挙型である。 |
| [`SazGame`](sazpack.md#sazgame) | `rsshogi.sazpack` | SAZ2 形式の 1 局分の自己対局データである。 |
| [`SazPosition`](sazpack.md#sazposition) | `rsshogi.sazpack` | SAZ2 形式の 1 局面分の探索統計と着手データである。 |
| [`SazPolicyEntry`](sazpack.md#sazpolicyentry) | `rsshogi.sazpack` | SAZ2 形式の候補手ごとの事前確率、訪問数、勝敗上下限である。 |
| [`SazWdl`](sazpack.md#sazwdl) | `rsshogi.sazpack` | 量子化された勝敗引分（Win / Draw / Loss）分布である。 |
| [`InitialPosition`](initial_positions.md#initialposition) | `rsshogi.initial_positions` | 平手や駒落ちの初期局面 SFEN を提供する列挙型である。 |
| [`Svg`](svg.md#svg) | `rsshogi.svg` | `Board.to_svg()` が返す SVG 画像ラッパである。 |
| [`PackedSfen`](numpy.md#packedsfen) | `rsshogi.numpy` | 32 バイトの圧縮局面を表す NumPy `dtype` である。 |
| [`PackedSfenValue`](numpy.md#packedsfenvalue) | `rsshogi.numpy` | 40 バイトの PSV 学習データを表す NumPy `dtype` である。 |
| [`HuffmanCodedPos`](numpy.md#huffmancodedpos) | `rsshogi.numpy` | 32 バイトの HCP 局面を表す NumPy `dtype` である。 |
| [`HuffmanCodedPosAndEval`](numpy.md#huffmancodedposandeval) | `rsshogi.numpy` | 38 バイトの HCPE 学習データを表す NumPy `dtype` である。 |

## スタンドアロン関数一覧

| 関数 | モジュール | 説明 |
|---|---|---|
| [`to_move(mv)`](move.md#to_move) | `rsshogi.core` | `Move`、`Move32`、または `0..=65535` の `int` を `Move` に変換する（範囲外の整数は `ValueError`）。 |
| [`parse_usi_position(position)`](usi.md#usi-position-文字列の解析と正規化rsshogicore) | `rsshogi.core` | USI `position` 文字列を解析し、`(初期局面の SFEN, 合法手の Move リスト)` のタプルを返す。 |
| [`parse_usi_position_parts(position)`](usi.md#usi-position-文字列の解析と正規化rsshogicore) | `rsshogi.core` | USI `position` 文字列を初期局面 SFEN、指し手列、最終局面 SFEN に分解する。 |
| [`normalize_usi_position(position)`](usi.md#usi-position-文字列の解析と正規化rsshogicore) | `rsshogi.core` | USI `position` 文字列を検証し、正規化された `"position startpos ..."` または `"position sfen ..."` 文字列へ変換する。 |
| [`decode_pack(data)`](record.md#pack-形式の一括読み書き) | `rsshogi.record` | `pack` バイト列から全対局を `list[Record]` として復号する。 |
| [`decode_pack_file(path)`](record.md#pack-形式の一括読み書き) | `rsshogi.record` | `pack` ファイルから全対局を `list[Record]` として読み込む。 |
| [`write_pack(records)`](record.md#pack-形式の一括読み書き) | `rsshogi.record` | 複数の `Record` を `pack` バイト列へシリアライズする。 |
| [`write_pack_file(path, records)`](record.md#pack-形式の一括読み書き) | `rsshogi.record` | 複数の `Record` を `pack` ファイルへ書き出す。 |
| [`decode_sbinpack(data)`](record.md#sbinpack-形式の一括読み書き) | `rsshogi.record` | `sbinpack` バイト列から `(Record, bytes)` のリストを復号する。 |
| [`decode_sbinpack_file(path)`](record.md#sbinpack-形式の一括読み書き) | `rsshogi.record` | `sbinpack` ファイルから `(Record, bytes)` のリストを読み込む。 |
| [`write_sbinpack(records, ...)`](record.md#sbinpack-形式の一括読み書き) | `rsshogi.record` | 複数の `Record` を `sbinpack` バイト列へシリアライズする。 |
| [`write_sbinpack_file(path, records, ...)`](record.md#sbinpack-形式の一括読み書き) | `rsshogi.record` | 複数の `Record` を `sbinpack` ファイルへ書き出す。 |
| [`move_from_usi(usi)`](usi.md#定数と指し手変換rsshogiusi) | `rsshogi.usi` | 通常手に加えて `"resign"`、`"win"`、`"0000"`、`"none"` も `Move` に変換する。 |
| [`parse_info(line)`](usi.md#定数と指し手変換rsshogiusi) | `rsshogi.usi` | USI `info` 行を解析して `UsiInfo` を返す。 |
| [`parse_bestmove(line)`](usi.md#定数と指し手変換rsshogiusi) | `rsshogi.usi` | USI `bestmove` 行を解析して `UsiBestMove` を返す。 |
| [`parse_score(text)`](usi.md#定数と指し手変換rsshogiusi) | `rsshogi.usi` | `"cp 120"` や `"mate +5"` などの USI `score` 文字列を解析する。 |
| [`move_label(move, turn)`](policy.md#関数一覧) | `rsshogi.policy` | 指し手と手番から 2187 クラスの policy ラベルを計算する。 |
| [`compact_move_label(move, turn)`](policy.md#関数一覧) | `rsshogi.policy` | 指し手と手番から 1496 クラスの圧縮ラベルを計算する。 |
| [`move_label_to_compact(label)`](policy.md#関数一覧) | `rsshogi.policy` | 2187 クラスのラベルを 1496 クラスの圧縮ラベルへ変換する。 |
| [`compact_move_label_to_move_label(label)`](policy.md#関数一覧) | `rsshogi.policy` | 1496 クラスの圧縮ラベルを 2187 クラスのラベルへ展開する。 |
| [`is_structurally_valid_move_label(label)`](policy.md#関数一覧) | `rsshogi.policy` | 2187 クラスのラベルが盤面構造上出現しうる手かどうかを判定する。 |
| [`write_sazpack(games)`](sazpack.md#シリアライズとデシリアライズ関数) | `rsshogi.sazpack` | `SazGame` の列を SAZ2 バイナリに変換する。 |
| [`write_sazpack_file(path, games)`](sazpack.md#シリアライズとデシリアライズ関数) | `rsshogi.sazpack` | `SazGame` の列を SAZ2 ファイルへ書き出す。 |
| [`decode_sazpack(data)`](sazpack.md#シリアライズとデシリアライズ関数) | `rsshogi.sazpack` | SAZ2 バイナリから `list[SazGame]` を復元する。 |
| [`decode_sazpack_file(path)`](sazpack.md#シリアライズとデシリアライズ関数) | `rsshogi.sazpack` | SAZ2 ファイルから `list[SazGame]` を読み込む。 |
| [`handicap_to_sfen(handicap)`](initial_positions.md#手合割名と-sfen-の相互変換) | `rsshogi.initial_positions` | 日本語の手合割名（`"平手"`、`"二枚落ち"` など）から SFEN 文字列を返す。 |
| [`sfen_to_handicap(sfen)`](initial_positions.md#手合割名と-sfen-の相互変換) | `rsshogi.initial_positions` | SFEN 文字列に対応する日本語の手合割名を返す。 |

## 対応データフォーマット

| フォーマット | 読み込み | 書き出し | 主な対応 API | 説明 |
|---|---|---|---|---|
| KIF / KIFU | 対応 | 対応 | [`Record.from_kif_str()`](record.md#読み込みクラスメソッド), [`Record.to_kif()`](record.md#書き出しインスタンスメソッド) | 柿木形式の棋譜であり、変化手順や消費時間、コメントに対応する。 |
| KI2 / KI2U | 対応 | 対応 | [`Record.from_ki2_str()`](record.md#読み込みクラスメソッド), [`Record.to_ki2()`](record.md#書き出しインスタンスメソッド) | 移動元座標を省略した柿木形式の棋譜である。 |
| CSA | 対応 | 対応 | [`Record.from_csa_str()`](record.md#読み込みクラスメソッド), [`Record.to_csa()`](record.md#書き出しインスタンスメソッド) | CSA 標準棋譜形式（V2.2 / V3.0、複数局 `/` 区切り）に対応する。 |
| JKF | 対応 | 対応 | [`Record.from_jkf_str()`](record.md#読み込みクラスメソッド), [`Record.to_jkf()`](record.md#書き出しインスタンスメソッド) | JSON Kifu Format であり、分岐手順にも対応する。 |
| USI position | 対応 | 対応 | [`parse_usi_position()`](usi.md#usi-position-文字列の解析と正規化rsshogicore), [`Record.from_usi_position()`](record.md#読み込みクラスメソッド) | `position startpos moves ...` や `sfen ...` 形式の手順文字列である。 |
| pack | 対応 | 対応 | [`Record.from_pack()`](record.md#読み込みクラスメソッド), [`decode_pack()`](record.md#pack-形式の一括読み書き), [`write_pack()`](record.md#pack-形式の一括読み書き) | ScriptCollection 互換の可変長バイナリ棋譜形式である。 |
| sbinpack | 対応 | 対応 | [`Record.from_sbinpack()`](record.md#読み込みクラスメソッド), [`decode_sbinpack()`](record.md#sbinpack-形式の一括読み書き), [`write_sbinpack()`](record.md#sbinpack-形式の一括読み書き) | 評価値とチェーン単位メタデータを持つ差分圧縮バイナリ形式（v2）である。 |
| PSV (`PackedSfenValue`) | 対応（局面） | 対応 | [`Board.to_psv()`](board.md#バイナリシリアライズと-apery-互換変換), [`Record.to_psv()`](record.md#書き出しインスタンスメソッド), [`PackedSfenValue`](numpy.md#packedsfenvalue) | やねうら王互換の 40 バイト固定長教師データ形式である。 |
| HCP / HCPE | 対応（HCP） | 対応 | [`Board.to_hcp()`](board.md#バイナリシリアライズと-apery-互換変換), [`Board.set_hcp()`](board.md#バイナリシリアライズと-apery-互換変換), [`Board.to_hcpe()`](board.md#バイナリシリアライズと-apery-互換変換), [`HuffmanCodedPosAndEval`](numpy.md#huffmancodedposandeval) | Apery および `cshogi` 互換の 32 バイト局面 / 38 バイト教師データ形式である。 |
| SAZ2 (`sazpack`) | 対応 | 対応 | [`decode_sazpack()`](sazpack.md#シリアライズとデシリアライズ関数), [`write_sazpack()`](sazpack.md#シリアライズとデシリアライズ関数) | AlphaZero 系自己対局の WDL、policy、探索統計を保持するバイナリ形式である。 |
| Native Book v4 (`.book`) | 対応 | 対応 | [`NativeBook`](book.md#native-book-v4), `NativeBookEditor`, `NativeBookBuilder` | 部分参照、差分保存、初期構築に対応する。 |
| DB2016 (`.db`) | 対応（参照） | 非対応 | [`YaneuraOuBook`](book.md#やねうら王形式定跡yaneuraoubook) | やねうら王のテキスト定跡形式を読み取り専用で検索する。 |
| SBK (`.sbk`) | 対応（参照） | 非対応 | [`SbkBook`](book.md#sbk-形式定跡sbkbook) | ShogiHome 系のバイナリ定跡形式を読み取り専用で検索する。 |

## 関連項目

- [クイックスタート](../getting-started/quickstart.md)
- [例とパターン](../getting-started/examples.md)
- [フォーマット仕様一覧](../reference/formats/index.md)
