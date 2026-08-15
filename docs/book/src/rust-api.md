# Rust API

## インストール

Rust 1.95 以降で core crate を追加します。
既定 feature は空なので、利用するデータ形式だけを明示します。

```toml
[dependencies]
rsshogi = { version = "1.2.0", features = ["records", "book", "position-serialization"] }
```

| Feature | 有効になる機能 |
| --- | --- |
| `records` | KIF、KI2、CSA、JKF、USI-position、PACK、SBINPACK などの棋譜 API。 |
| `book` | memory/static book、DB2016、YBB、SBK。`initial-positions` と `position-serialization` も有効になる。 |
| `position-serialization` | HCP、PackedSfen と関連する局面変換。 |
| `policy-labels` | policy label の相互変換。 |
| `svg` | 盤面の SVG 出力。 |
| `validation` | 局面検証 API。 |
| `initial-positions` | 平手・駒落ちの名前付き初期局面。 |
| `hash-128` | `ZobristKey` と `BookKey` を 128 bit にする。低位 64 bit は通常 build と一致する。 |

`python-data` は Python binding の build 用に上記のデータ機能をまとめた feature です。
通常の Rust application は必要な feature を個別に選びます。

## 局面と合法手

```rust,ignore
use rsshogi::board::{MoveList, hirate_position};
use rsshogi::movegen::{LegalAll, generate_moves};

let position = hirate_position();
let mut moves = MoveList::new();
generate_moves::<LegalAll>(&position, &mut moves);

assert_eq!(moves.len(), 30);
assert!(moves.iter().any(|mv| mv.to_usi() == "7g7f"));
```

`Position` は盤面、持ち駒、手番、手数と、合法性判定に必要な状態を所有します。
SFEN には `Position::from_sfen` または `board::position_from_sfen` を使い、局面更新には `apply_move` / `undo_move` を使います。
外部入力から作った手は、適用前に `Position::is_legal_move` で確認します。

`MoveList` と `Move32List` は生成手向けの固定容量コンテナで、通常の生成では heap allocation を行いません。
`Move` と `Move32` の生成集合は、`Move32` の駒 metadata を除けば一致します。
custom sink を使う場合は `MoveSink` または `Move32Sink` を実装します。

## move-generation mode

完全な合法手集合が必要な場合は `LegalAll` を使います。
`Legal` は探索向けに、選択しても局面結果を変えない一部の不成を省略します。
たとえば敵陣 3 段目へ進む香の不成は `Legal` にも含まれますが、歩と大駒の一部の任意不成まで列挙するには `LegalAll` が必要です。

| Mode | 契約 |
| --- | --- |
| `Legal` / `LegalAll` | 王手中なら回避手へ切り替え、自玉の安全まで確認する。 |
| `Captures` / `CapturesAll` | 取る手を生成する。単側の探索用 generator であり、完全な合法手 API ではない。 |
| `CapturePlusPro` / `CapturePlusProAll` | 取る手に、敵陣へ進む歩の成りを加える。 |
| `Quiets` / `QuietsAll` | 駒を取らない手を生成する。 |
| `QuietsProMinus` / `QuietsProMinusAll` | 静かな手から歩の成りを除く。 |
| `Checks` / `ChecksAll` | 取る手と静かな手を含む王手を生成する。 |
| `QuietChecks` / `QuietChecksAll` | 駒を取らない王手を生成し、駒打ちの王手も含む。 |
| `Evasions` / `EvasionsAll` | 王手局面用の pseudo-legal 回避手を生成する。 |
| `NonEvasions` / `NonEvasionsAll` | 非王手局面用の pseudo-legal 手を生成する。 |
| `Recaptures` / `RecapturesAll` | 指定升目への取り返し候補を生成する。 |

`*All` は、対応する mode に歩、香、大駒の任意不成を加えます。
`Legal` 系以外は探索を分割するための generator であり、呼び出し側が局面の前提と後段の合法性 filter を管理します。
王手局面で `Captures` や `Quiets` を呼んでも、自動的に王手回避だけへ絞られません。

生成順はすべての mode で未規定です。
再現可能な順序が必要なら raw 値、探索 score、または application 固有の key で並べ替えます。

## 手の型と raw 値

`Move` / `Move32` と `AperyMove` / `AperyMove32` は別の nominal type です。
`Move` は bit 14 を駒打ち、bit 15 を成りに使います。
`AperyMove` は source field 81–87 を駒打ち、bit 14 を成りに使います。
raw 値を cast せず、`Move::to_apery`、`AperyMove::to_move`、`Move32::to_apery(position)` を使います。

```rust,ignore
use rsshogi::types::{AperyMove, Move};

let promoted = Move::from_usi("8h2b+").expect("valid USI");
let packed: AperyMove = promoted.to_apery();
assert_eq!(packed.to_move(), promoted);

let null = Move::from_usi("0000").expect("USI null move");
assert_eq!(null, Move::MOVE_NULL);
```

`0000` と `null` はどちらも null move として解析されます。
`Move::to_usi()` の正規出力は `null` です。
`none`、`resign`、`win`、`end` もそれぞれ対応する特殊値へ変換されます。

## 基本値

`Color`、`File`、`Rank`、`Square`、`PieceType`、`Piece`、`HandPiece`、`Hand`、`Move`、`Move32`、`Eval`、`GameResult` の文書化された有効 raw 値は互換契約です。
外部の整数には、値を検証する constructor または `is_valid` を使います。

- `Square::iter()` は `file_index * 9 + rank_index` の file-major 順です。
- `Eval::Cp` は `-32000..=32000`、それ以外の `i16` は `Eval::Special` です。比較順は variant ではなく signed raw 値の数値順です。
- `HandPiece::from_piece_type` は未成の歩、香、桂、銀、角、飛、金だけを受け付けます。成駒を自動で生駒へ戻しません。
- `Hand::add` / `Hand::sub` は field の上限超過・不足で panic します。検証可能な入力には `checked_add` / `checked_sub` を使います。
- `Bitboard::is_aligned(from, to, king)` は、`from` と `to` が `king` から見て同じ向きの ray 上にある場合だけ `true` です。玉を挟んで反対側にある二升は `false` です。

## 一手詰め

`mate::solve_mate_in_one(&Position) -> Option<Move32>` は、合法な王手で相手に合法応手を残さない手だけを返します。
複数の一手詰めがある場合、どの詰み手を返すかは未規定です。
返却順や特定の詰み手ではなく、返された手の合法性と詰みを検証してください。

## 棋譜と定跡

`records` feature は共通の `Record` tree と typed entry を中心に、各形式の parser / writer を提供します。
形式をまたぐ変換では parser の内部構造ではなく `Record` を中間表現にします。

`book` feature は `MemoryBook`、`StaticBook`、`BookDatabase`、`YaneuraOuBook`、`YbbBook`、`SbkBook` を提供します。
DB2016 の大規模ファイルは `YaneuraOuBook` が path を保持し、lookup と `iter_entries()` で必要な範囲を読みます。
詳細な wire contract と DB2016 writer option は [形式と互換性](formats.md) を参照してください。
