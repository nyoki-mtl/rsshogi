# Python API リファレンス

Python から rsshogi を使用するための API リファレンスです。

## このリファレンスの読み方

各ページは共通の記法ルールで書かれています。

- **シグネチャ**は Python のコードフェンスで示します（`board.to_sfen() -> str` など）。
- **型・列挙・定数が中心のページ**（[補助型](types.md) / [GameResult](game_result.md) /
  [Record 系](record.md) / [定跡](book.md) / [NumPy 連携](numpy.md)）は、
  プロパティ・メソッドを一覧表で示します。
- **主要な操作クラス**（[Board](board.md) / [Move](move.md) / [Move32](move32.md)）は、
  メソッドごとに引数・戻り値・例外・使用例を個別に説明します。
- **例外ポリシー**は全 API 共通です。
  - 型が不正: `TypeError`
  - 値は型として正しいが内容が不正（不正な文字列・違法手など）: `ValueError`
  - 履歴が空などの「未存在」: `None` を返す（例: `last_move()`, `pop()`）
- 各ページの末尾には関連ページへのリンク（**関連項目**）があります。

## インストール

```bash
pip install rsshogi
```

AVX2 対応 x86_64 CPU 向けの高速版も利用できます（通常版とは同時にインストールできません）。迷った場合は通常版を使ってください。

```bash
pip install rsshogi-avx2
```

## モジュール構成

rsshogi はサブモジュールで構造化されています。

| モジュール | 主な型 | 説明 |
|------------|--------|------|
| [`rsshogi.core`](board.md) | `Board`, `Move32`, `Move`, `AperyMove`, `AperyMove32`, `PositionState`, `ValidationReport`, `ValidationIssue`, `to_move()`, `parse_usi_position()`, `parse_usi_position_parts()`, `normalize_usi_position()` | 盤面、raw-state、指し手 |
| [`rsshogi.types`](types.md) | `Color`, `PieceType`, `Piece`, `Square`, `Bitboard`, `Hand`, `MoveType`, `RepetitionState` | 補助型と定数 |
| [`rsshogi.record`](record.md) | `Record`, `RecordMetadata`, `GameResultInfo`, `MoveEntry`, `EngineInfo`, `SpecialMoveEntry`, `TimeControl`, `GameResult` | 棋譜の読み書き |
| [`rsshogi.book`](book.md) | `StaticBook`, `MemoryBook`, `BookBuilder`, `BookKey`, `BookMove`, `BookEntry`, `YaneuraOuBook`, `SbkBook` | 定跡データベース（内部形式・外部形式） |
| [`rsshogi.usi`](usi.md) | `UsiInfo`, `UsiBestMove`, `UsiGoCommand`, `UsiScore`, `UsiBound`, `move_from_usi()`, `parse_info()`, `parse_bestmove()` | USI プロトコルの値オブジェクトとパーサ |
| [`rsshogi.policy`](policy.md) | `move_label()`, `compact_move_label()`, 変換テーブル | policy 学習向けラベル変換 |
| [`rsshogi.initial_positions`](initial_positions.md) | `InitialPosition` | 初期局面の SFEN 定数 |
| [`rsshogi.svg`](svg.md) | `Svg` | 盤面の SVG 描画 |
| [`rsshogi.numpy`](numpy.md) | `PackedSfen`, `PackedSfenValue`, `HuffmanCodedPos`, `HuffmanCodedPosAndEval` | NumPy dtype 定義（要 NumPy） |

### インポート例

```python
from rsshogi.core import (
    Board,
    Move32,
    Move,
    AperyMove,
    AperyMove32,
    PositionState,
    ValidationIssue,
    ValidationReport,
    to_move,
    parse_usi_position,
    normalize_usi_position,
)
from rsshogi.types import Color, PieceType, Piece, Square, Bitboard, Hand, MoveType, RepetitionState
from rsshogi.record import (
    Record,
    RecordMetadata,
    MoveEntry,
    EngineInfo,
    SpecialMoveEntry,
    TimeControl,
    GameResult,
)
from rsshogi.book import StaticBook, MemoryBook, BookBuilder, BookKey, BookMove, BookEntry
from rsshogi.book import book_key_from_position, book_key_after, book_builder_from_game_record
from rsshogi.book import YaneuraOuBook, SbkBook
from rsshogi.usi import UsiInfo, UsiBestMove, UsiGoCommand, UsiScore, UsiBound
from rsshogi.policy import (
    MOVE_LABEL_COUNT,
    COMPACT_MOVE_LABEL_COUNT,
    move_label,
    compact_move_label,
)
from rsshogi.initial_positions import InitialPosition
from rsshogi.svg import Svg
from rsshogi.numpy import (
    PackedSfen,
    PackedSfenValue,
    HuffmanCodedPos,
    HuffmanCodedPosAndEval,
)  # NumPy が必要
```

## クラス一覧

### 盤面・指し手

| クラス | 説明 |
|--------|------|
| [Board](board.md) | 盤面を管理するメインクラス |
| [PositionState](board.md#positionstate) | Board の raw-state を保持する編集用オブジェクト |
| [ValidationReport](board.md#validationreport) | `validate_all()` の全件レポート |
| [ValidationIssue](board.md#validationissue) | 個別の validation 問題 |
| [Move32](move32.md) | 32bit 指し手型（駒情報を含む完全な表現） |
| [Move](move.md) | 16bit 指し手型（軽量、通常はこちらを使用） |
| [Apery / hcpe 互換](apery.md) | `AperyMove` / `AperyMove32` と HCP/HCPE の互換 API |

### 棋譜

| クラス | 説明 |
|--------|------|
| [Record](record.md) | 棋譜レコード（初期局面・開始局面コメント・指し手列・メタデータ・結果） |
| [RecordMetadata](record.md#gamerecordmetadata) | 対局ヘッダ（棋戦名・対局者・日時など） |
| [MoveEntry](record.md#moverecord) | 各指し手のデータ（コメント・消費時間・エンジン解析情報） |
| [EngineInfo](record.md#moveengineinfo) | エンジン解析情報（評価値・深度・ノード数・拡張属性） |
| [SpecialMoveEntry](record.md#specialmoverecord) | 終局特殊手（種別・終局コメント・終局時間・raw） |
| [TimeControl](record.md#timecontrol) | 持ち時間の設定 |
| [GameResult](game_result.md) | 終局結果（18 種類の終局状態） |
| [GameResultInfo](record.md#gameresultinfo) | 終局情報ビュー（`ply_count` / `reason` / `end_time_ms` / `end_comment`） |

### 定跡

| クラス | 説明 |
|--------|------|
| [MemoryBook](book.md#memorybook) | メモリ上で編集可能な定跡 |
| [StaticBook](book.md#staticbook) | 読み取り専用の定跡ファイル |
| [BookBuilder](book.md#bookbuilder) | 棋譜から定跡を構築 |
| [BookKey](book.md#bookkey) | 局面の定跡検索キー |
| [BookMove](book.md#bookmove) | 定跡の候補手データ |
| [BookEntry](book.md#bookentry) | 定跡の1局面分のエントリー |
| [YaneuraOuBook](external-books.md#db2016) | DB2016 `.db` ファイルの外部定跡リーダ |
| [SbkBook](external-books.md#sbk) | SBK `.sbk` ファイルの外部定跡リーダ |

### 補助型

| クラス | 説明 |
|--------|------|
| [Color](types.md#color) | 手番（BLACK / WHITE） |
| [PieceType](types.md#piecetype) | 駒種（PAWN, LANCE, ... DRAGON） |
| [Piece](types.md#piece) | 駒（色付き駒種） |
| [Square](types.md#square) | マス目（81 マス + NONE） |
| [Bitboard](types.md#bitboard) | ビットボード（81 ビット盤面表現） |
| [Hand](types.md#hand) | 持ち駒 |
| [MoveType](types.md#movetype) | 指し手の種類（通常 / 成り / 駒打ち） |
| [RepetitionState](types.md#repetitionstate) | 千日手の状態 |

### ユーティリティ

| クラス | 説明 |
|--------|------|
| [InitialPosition](initial_positions.md) | 初期局面の SFEN 定数（平手・駒落ち） |
| [Svg](svg.md) | 盤面の SVG 描画 |
| [NumPy 連携](numpy.md) | PackedSfen / PackedSfenValue / HCP / HCPE dtype |

## スタンドアロン関数

| 関数 | モジュール | 説明 |
|------|------------|------|
| [`to_move(mv)`](move.md) | `rsshogi.core` | `Move` / `Move32` / `int` を `Move` に変換（`int` は下位16bitを使用） |
| `parse_usi_position(position)` | `rsshogi.core` | `position ...` / `startpos` / `sfen ... moves ...` を `Board` に変換 |
| `parse_usi_position_parts(position)` | `rsshogi.core` | USI position 文字列を `UsiPositionParts`（初期局面 SFEN・指し手列・最終局面 SFEN）に分解 |
| `normalize_usi_position(position)` | `rsshogi.core` | USI position 文字列を正規化（初期局面は `"startpos"`） |
| [`book_key_from_position(board)`](book.md#book_key_from_position) | `rsshogi.book` | 局面から定跡キーを計算 |
| [`book_key_after(board, mv)`](book.md#book_key_after) | `rsshogi.book` | 1手後の定跡キーを計算（盤面を変更せずに） |
| [`book_builder_from_game_record(record, ...)`](book.md#book_builder_from_game_record) | `rsshogi.book` | 棋譜から定跡を一括構築 |
| [`move_label(move, turn)`](policy.md) | `rsshogi.policy` | `Move` / `Move32` / USI を 2187 クラスへ変換 |
| [`compact_move_label(move, turn)`](policy.md) | `rsshogi.policy` | 1496 クラスへ圧縮（構造的に無効なら `None`） |

## 対応フォーマット

| フォーマット | パース | エクスポート | 説明 |
|-------------|:------:|:----------:|------|
| KIF | o | o | 柿木形式 |
| KI2 | o | o | KIF の指し手のみ形式 |
| CSA | o | o | CSA 標準形式 |
| JKF | - | o | JSON 棋譜形式（エクスポートのみ） |
| sbinpack | o | o | バイナリ学習データ形式 |
| 定跡バイナリ | o | o | 互換定跡形式（`StaticBook` / `MemoryBook`） |
| DB2016 | o（参照） | - | `.db` 外部定跡（`YaneuraOuBook`） |
| SBK | o（参照） | - | SBK `.sbk` 外部定跡（`SbkBook`） |
| HCP / HCPE | o | o | Apery / cshogi 互換の固定長学習データ |

## 関連ドキュメント

- [クイックスタート](../getting-started/quickstart.md) - 基本的な使い方
- [例とパターン](../getting-started/examples.md) - 実践的なコード例
- [Policy ラベル](policy.md) - policy 学習向けラベル変換
