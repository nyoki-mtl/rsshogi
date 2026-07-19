# rsshogi

`rsshogi` は、将棋エンジンや解析ツールで再利用しやすい Rust 製の将棋ライブラリです。
盤面表現、指し手、合法手生成、局面の適用と取り消し、各種棋譜フォーマットを、Rust の型と
エラー処理に沿った API として提供します。

> English: `rsshogi` provides reusable Rust shogi primitives for board state,
> moves, legality, move generation, record parsing, and training-data formats.

[YaneuraOu](https://github.com/yaneurao/YaneuraOu) の設計と検証対象を参考にしつつ、
Rust から使いやすく、安全で明示的なプリミティブとして整理することを重視しています。
Python バインディングは別パッケージ
([rsshogi](https://pypi.org/project/rsshogi/)) として提供しています。

## 設計思想

### 1. 正確性

- 参照実装の合法手生成と照合するテストで、基本ルールと特殊ルールの互換性を検証
- Perft テストによる手生成の網羅的な確認
- 千日手、入玉宣言勝ち、反則手判定など、将棋固有のルールを明示的に扱う

### 2. 再利用しやすい Rust API

- 盤面、駒、指し手、持ち駒、棋譜、ラベル変換を crate 内の独立したプリミティブとして提供
- エンジンや評価関数とは分離し、局面管理、合法性、入出力の基盤に専念
- Python バインディングは薄い別レイヤーに置き、core crate の API は Rust 利用を主軸に保つ

### 3. 安全性

- パフォーマンス上必要な箇所に限り `unsafe` を使用し、SAFETY コメントとテストで妥当性を担保
- コンパイル時に検出可能なエラーは型システムで表現
- 実行時エラーは `Result` 型で明示的に扱う

### 4. パフォーマンス

- ビットボードによる高速な盤面表現
- Magic Bitboard / PEXT による効率的なスライダー攻撃計算
- 差分更新による Zobrist ハッシュ計算
- SIMD 最適化（ビルド時に `target-feature=+avx2` が有効な x86_64 では AVX2 実装を使用）

## アーキテクチャ

```text
rsshogi
├── types     - 基本型（Color, Square, Move, Bitboard, Hand, ...）
├── labels    - 学習や推論で利用するラベル変換（policy-labels feature）
├── board     - 盤面管理と合法手生成
│   ├── Position      - 盤面状態の管理
│   ├── movegen       - 合法手生成
│   ├── attack_tables - 利きテーブル
│   └── ...
├── records   - 棋譜フォーマット（KIF, CSA, JKF, SFEN, sbinpack, sazpack, ...）
├── book      - 定跡管理（MemoryBook, StaticBook, BookDatabase, 外部定跡）
└── mate      - 詰み判定（1手詰め）
```

`labels` / `records` / `book` / `svg` / `validation` は opt-in feature です。
エンジン向けの軽量利用では既定 feature のまま、棋譜 I/O や定跡 I/O が必要な場合だけ
対応する feature を有効にしてください。

## クイックスタート

### 盤面操作

```rust
use rsshogi::board::{self, Position};
use rsshogi::types::Move;

// 任意: 初回アクセス前にテーブルを warm-up する
board::init();

// 平手初期局面
let mut pos = board::hirate_position();

// SFEN から局面を構築
let mut pos = board::position_from_sfen(
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"
).unwrap();

// 指し手の適用
let mv16 = Move::from_usi("7g7f").unwrap();
let m = pos.move32_from_move(mv16);
pos.apply_move32(m);

// 指し手の取り消し
pos.undo_move32(m);

// SFEN 出力
println!("{}", pos.to_sfen(None));
```

### SVG 描画（`svg` feature）

`svg` feature を有効にすると `Position::to_svg` で盤面 SVG を生成できます。

```text
use rsshogi::board;
use rsshogi::types::Move;

board::init();

let mut pos = board::hirate_position();
let mv = pos.move32_from_move(Move::from_usi("7g7f").unwrap());
pos.apply_move32(mv);

let svg = pos.to_svg(Some(mv), 1.0);
assert!(svg.starts_with("<svg"));
```

### raw-state import/export

```rust
use rsshogi::board::{self, position_from_position_state};

board::init();

let pos = board::hirate_position();
let mut state = board::generate_position_state(&pos);
state.ply = 42;

let restored = position_from_position_state(&state);
assert_eq!(board::generate_position_state(&restored), state);
```

`PositionState` は SFEN 文字列を経由しない raw-state 境界です。
外部で編集したデータを import した後は、`validation` feature を有効にすると
`validate()` / `validate_all()` で明示的に検証できます。

### 合法手生成

```rust
use rsshogi::board::{self, MoveList};
use rsshogi::movegen::generate_legal_all;

board::init();
let pos = board::hirate_position();

// 通常省略される不成も含む全合法手を生成
let mut moves = MoveList::new();
generate_legal_all(&pos, &mut moves);

for m in moves.iter() {
    println!("{}", m.to_usi());
}

// 合法手の数
println!("合法手数: {}", moves.len());
```

### policy ラベル変換（`policy-labels` feature）

`policy-labels` feature を有効にすると `labels::policy` を利用できます。

```text
use rsshogi::labels::policy::{CompactMoveLabel, MoveLabel, MoveLabelClass};
use rsshogi::types::{Color, Move, Square};

let mv = Move::from_usi("7g7f").unwrap();
let label = MoveLabel::from_move(mv, Color::BLACK).unwrap();

assert_eq!(label.class(), MoveLabelClass::Up);
assert_eq!(label.to_sq(), Square::from_usi("7f").unwrap());

let compact = CompactMoveLabel::from_move(mv, Color::BLACK).unwrap();
assert_eq!(compact.expand(), label);
```

### 棋譜の読み込み（`records` feature）

`records` feature を有効にすると棋譜 I/O（KIF/CSA/HCPE/JKF/SFEN/sbinpack 等）を利用できます。

```text
use rsshogi::records::formats::kif;
use rsshogi::types::GameResult;

let kif_text = r#"
手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 ３四歩(33)
まで2手で中断
"#;

let record = kif::parse_kif_str(kif_text).unwrap();
println!("手数: {}", record.move_count());
println!("結果: {:?}", record.result());
assert_eq!(record.result(), GameResult::Paused);
assert!(record.main_terminal().is_some());
```

### 棋譜のエンコードと走査（`records` feature）

```text
use rsshogi::records::formats::{kif, traversal, TextEncoding};
use rsshogi::records::record::RecordEntry;

let kif_text = "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\nまで1手で中断\n";
let record = kif::parse_kif_str(kif_text).unwrap();

let encoded = kif::export_kif_bytes(&record, TextEncoding::Utf8).unwrap();
assert!(!encoded.has_unmappable_chars());

traversal::traverse_with_position(&record, |node| {
    if let RecordEntry::Move(mv_record) = node.entry {
        println!("{}: {}", node.ply, mv_record.mv().to_usi());
    }
    true
})
.unwrap();
```

### 定跡の利用（`book` / `records` feature）

`book` feature を有効にすると定跡 DB と外部定跡リーダを利用できます。
`Record` から定跡を構築する `BookBuilder` は `records` feature も必要です。

```text
use rsshogi::board::{self, Position};
use rsshogi::book::{Book, MemoryBook, BookBuilder, book_key_from_position};

board::init();
let pos = board::hirate_position();

// 定跡を構築（Record から）
let mut builder = BookBuilder::default();
// builder.add_record(&record);
let book = builder.into_memory();

// 定跡を検索
let key = book_key_from_position(&pos);
if let Some(entry) = book.get(key) {
    for mv in entry.moves() {
        println!("{}: {}", mv.mv().to_usi(), mv.score());
    }
}
```

## モジュール詳細

### `types` - 基本型

将棋で使用する基本的な型を定義します。

| 型 | 説明 |
|---|---|
| `Color` | 手番（先手 `BLACK` / 後手 `WHITE`） |
| `Square` | マス（0-80、9x9 盤面） |
| `File` | 筋（1-9） |
| `Rank` | 段（一-九） |
| `PieceType` | 駒種（歩、香、桂、銀、金、角、飛、玉、と、... ） |
| `Piece` | 駒（駒種 + 手番） |
| `Hand` | 持ち駒（各駒種の枚数をビットパック） |
| `Move32` | 32bit 指し手（移動元、移動先、成り、駒打ちなど） |
| `Move` | 16bit 指し手（コンパクト版） |
| `Bitboard` | 81bit ビットボード |
| `GameResult` | 対局結果（勝敗、千日手、入玉宣言など） |

### `labels` - 学習向けラベル

局面非依存で使える policy ラベル変換を提供します。

- `policy::MoveLabel`: 手番を先手視点に正規化した 2187 クラス（27x81）
- `policy::CompactMoveLabel`: 構造的に現れうる 1496 クラスに圧縮したラベル
- `policy::MoveLabelClass`: 通常移動 20 種 + 駒打ち 7 種のラベルクラス
- `policy::MOVE_LABEL_TO_COMPACT` / `policy::COMPACT_TO_MOVE_LABEL`: 変換テーブル

`CompactMoveLabel` は「局面で合法」ではなく、空盤面の移動、成り、駒打ちの制約から見て
構造的に現れうるラベルだけを残したものです。

### `board` - 盤面管理

`Position` 構造体で盤面状態を管理します。

SFEN 文字列を経由しない raw-state の import/export には
`PositionState`, `generate_position_state()`, `generate_sfen_from_position_state()`,
`position_from_position_state()`, `Position::set_position_state()` を使用します。
外部入力をそのまま反映した場合は `validate()` / `validate_all()` で
ルール妥当性を明示的に確認できます。

**主要メソッド:**

| メソッド | 説明 |
|---|---|
| `apply_move32(m)` | 指し手を適用（Move32） |
| `undo_move32(m)` | 指し手を取り消し（Move32） |
| `is_legal_move32(m)` | 合法手か判定（Move32） |
| `checkers()` | 王手している駒のビットボード |
| `check_square(pt)` | 指定駒種で王手可能な候補マス（キャッシュ） |
| `blockers_for_king(color)` | 玉を守るブロッカー（pin候補を含む） |
| `is_in_check()` | 王手されているか |
| `is_mated()` | 詰んでいるか |
| `validate()` / `validate_all()` | 盤面の妥当性を検証 |
| `to_sfen(ply)` | SFEN 文字列を生成 |
| `board_key()` | 盤上配置 + 手番の Zobrist キー |
| `key()` | Zobrist ハッシュキー |

### 探索ホットパス向け cache view

王手判定やピン判定で複数の tactical cache をまとめて読みたい場合は、
`Position::current_state_cache()` から `StateCacheView` を取得できます。
`check_square(piece_type)` は指定した駒種の王手候補マスだけを返すため、探索ホットパスで
`checkers` / `check_square` / `blockers_for_king` / `pinners` を低コストでまとめて参照できます。

```rust
use rsshogi::board;
use rsshogi::types::{Color, PieceType};

board::init();

let pos = board::hirate_position();
let cache = pos.current_state_cache();

let gives_rook_check = cache.check_square(PieceType::ROOK);
let blockers = cache.blockers_for_king(Color::BLACK);
let pinners = cache.pinners(Color::WHITE);

assert_eq!(cache.checkers(), pos.checkers());
assert_eq!(cache.check_square(PieceType::ROOK), gives_rook_check);
assert_eq!(blockers, pos.blockers_for_king(Color::BLACK));
assert_eq!(pinners, pos.pinners(Color::WHITE));
```

`StateCacheView` は取得時点の current state を表す共有 borrow です。
`apply_move32()` / `apply_null_move()` / `undo_*()` を挟んだ後は再取得してください。

### `board::movegen` - 指し手生成

ジェネリクスによる型安全な手生成 API を提供します。

```rust
use rsshogi::board::{self, MoveList};
use rsshogi::movegen::{Captures, Evasions, generate_legal_all, generate_moves};

board::init();
let pos = board::hirate_position();
let mut moves = MoveList::new();

// 通常省略される不成も含む全合法手
generate_legal_all(&pos, &mut moves);

// 駒を取る手のみ
generate_moves::<Captures>(&pos, &mut moves);

// 王手回避の pseudo-legal 手（王手されている場合）
generate_moves::<Evasions>(&pos, &mut moves);
```

`generate_moves::<Evasions>()` / `generate_moves::<EvasionsAll>()` は legal-only ではなく、
split legality 後段で落ちる回避手を含み得ます。王手回避を legal-only で直接取りたい場合は
`generate_legal_evasions()` または `generate_legal_evasions_all()` を使います
（`Move32` リストが必要な場合は `generate_legal_evasions_move32()` /
`generate_legal_evasions_all_move32()`）。

`generate_moves::<Legal>()` は探索向けに通常省略される不成を省く legal-only 生成です。
全合法手集合を外部へ返す用途では `generate_legal_all()` を優先します。

`Move32`（駒情報付き 32bit 指し手）のリストを直接生成することもできます。
また、独自コンテナへ直接流し込む `Move32Sink` も使えます。

```rust
use rsshogi::board::{self, Move32List};
use rsshogi::movegen::{generate_legal_all_move32, generate_legal_evasions_all_move32};

board::init();
let pos = board::hirate_position();

let mut moves = Move32List::new();
generate_legal_all_move32(&pos, &mut moves);

let mut legal_evasions = Move32List::new();
if !pos.checkers().is_empty() {
    generate_legal_evasions_all_move32(&pos, &mut legal_evasions);
}
```

**手生成タイプ:**

| タイプ | 説明 |
|---|---|
| `Legal` | 全合法手 |
| `Captures` | 駒を取る手 |
| `Quiets` | 駒を取らない手 |
| `Evasions` | 王手回避の pseudo-legal 手 |
| `Checks` | 王手をかける手 |
| `Recaptures` | 取り返し手 |

### `board::attack_tables` - 利き/王手候補テーブル

低レベル API として、王手候補マスの取得関数を提供します。

```rust
use rsshogi::board::{self, check_candidate_bb};
use rsshogi::types::{Color, PieceType, Square};

board::init();
let king_sq = Square::from_usi("5a").unwrap();
let candidates = check_candidate_bb(Color::BLACK, PieceType::ROOK, king_sq);
assert!(candidates.count() > 0);
```

### `records` - 棋譜フォーマット

複数の棋譜フォーマットの読み書きをサポートします。

| フォーマット | パース | 出力 |
|---|:---:|:---:|
| KIF | ○ | ○ |
| KI2 | ○ | ○ |
| CSA | ○ | ○ |
| JKF | ○ | ○ |
| SFEN | ○ | ○ |
| SBINPACK | ○ | ○ |

`Record` の終局情報は `result() -> GameResult` で取得します。\
終局行そのものは本手順末尾の特殊手ノードとして保持されます。
投了、千日手、最大手数などの種別と raw text は
`main_terminal() -> Option<&SpecialMoveEntry>`、終局コメントや終局消費時間は
`main_terminal_node()` で得たノードの annotation に保持されます。
終局特殊手が存在しない棋譜も保持でき、その場合 `result()` は `GameResult::Invalid`、
`main_terminal()` は `None` になります。

KIF/KI2/CSA の細かな互換挙動（コメントの扱い、変化手順と終局要約の出力順、
指し手行末尾の消費時間、`initial_comment` や `RecordMetadata::comment` の対応関係など）は
[mdBook のリファレンス](https://nyoki-mtl.github.io/rsshogi/reference/records.html)にまとめています。

各指し手のエンジン解析情報は `EngineInfo` 構造体で管理されます。
評価値、探索深度、ノード数のほか、`extras` フィールドで任意のキー/値ペア
（`EngineExtraValue`: String / Int / Float / Bool）を保持できます。

Rust core では追加で以下の補助 API を提供します。

- `TextEncoding`, `ExportOptions`, `EncodedText`: KIF / KI2 / CSA を UTF-8 または Shift_JIS のバイト列として出力
- `traversal::traverse_with_position()`: `Record` を DFS で走査し、各ノードの局面を計算
- `traversal::position_at()`: 任意ノード時点の局面を復元

### `book` - 定跡

Zobrist ハッシュによる高速な定跡検索を提供します。

- `MemoryBook`: メモリ内で定跡を管理
- `StaticBook`: 静的にコンパイルされた定跡
- `BookDatabase`: 外部定跡の編集と変換に使う IR
- `YaneuraOuBook` / `YbbBook` / `SbkBook`: 外部定跡の参照
- `BookBuilder`: `Record` から定跡を構築（`book` + `records` feature）

### `mate` - 詰み判定

テーブル駆動の 1 手詰め判定ルーチンを提供します。

- `solve_mate_in_one()`: 1手詰め判定

## 互換性

### 互換

- ビットボード表現、Zobrist ハッシュ、指し手エンコーディングは参照実装と互換
- `Move`（16bit）は参照実装の `Move16` と同一のビットレイアウト
- Perft 結果を参照値と照合済み

### policy ラベル互換

- `labels::policy::MoveLabel` は、cshogi/dlshogi 系で広く使われる 27x81 の policy ラベル配置と互換
- 後手番の手は 180 度回転してからラベル化し、常に先手視点で比較できる
- `CompactMoveLabel` は 2187 クラスから構造的に無効な 691 クラスを除いた 1496 クラス

## ベンチマーク（perft）

80 局面で perft（depth=4）を計測した参考値です。

### 実行方法

```bash
RUSTFLAGS="-C target-feature=+avx2" \
  cargo bench -p rsshogi --bench movegen
```

AVX2 実装は runtime dispatch ではなくビルド時の target feature で選択されます。
AVX2 非対応環境で実行する配布物には `target-feature=+avx2` を付けないでください。

### 参考結果

x86_64 / AVX2 環境、PGO なし、5 回平均の結果です。

この環境での rsshogi AVX2 の avg NPS は約 248M でした。

結果はハードウェアや OS によって変動します。

## ライセンス

MIT License
