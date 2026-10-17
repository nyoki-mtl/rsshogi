# KI2 フォーマット仕様

KI2（柿木形式 2）は、KIF フォーマットから手数番号、移動元座標、および消費時間を省き、新聞の観戦記や棋書に近い表記で指し手を記述するテキスト棋譜フォーマットである。

## KIF との主な違い

KI2 はヘッダや初期局面（BOD 盤面図）の仕様を KIF と共有するが、手順セクションの記述規則に以下の違いがある。

| 項目 | KIF | KI2 |
| :--- | :--- | :--- |
| 手番記号 | なし（手数番号で識別） | `▲`（先手）、`△`（後手）、`▽`（後手別表記） |
| 移動元の表現 | `(77)` または `打` で明示 | `右`、`左`、`直`、`上`、`寄`、`引`、`打` の相対動作修飾子で区別 |
| 1 行あたりの指し手数 | 1 行に 1 手固定 | 1 行に複数の指し手を空白や改行なしで連続記述可能 |
| 消費時間 | 秒単位で記録可能 | 記録しない |
| 要約行欠落時の挙動 | `main_terminal = None` / `GameResult::Invalid` | 自動的に `SpecialMove::Interrupt` / `GameResult::Paused` を設定 |

## 1. 指し手表記と相対動作修飾子

KI2 では移動元座標を書かないため、同じ移動先に同種の駒が複数移動できる場合は、現在の盤面における合法手集合と照合して以下の修飾子を付与する。

- **左右の位置関係**：先手および後手それぞれの視点から見た移動元の相対位置を `右`、`左`、または真後ろから直進する `直`（金銀などの前進時）で表す。
- **前後の移動方向**：手番側の視点から前方に進む手を `上`、横に移動する手を `寄`、後方に退く手を `引` で表す。
- **駒打ちの明示**：盤上の同種駒もそのマスへ移動できる場合、持ち駒を打つ手には `打` を付与し、盤上の駒を動かす手には位置または方向修飾子を付与して曖昧さを排除する。
- **成と不成**：成る場合は `成`、成れる移動で成らない場合は `不成` を末尾に付す。

```text
手合割：平手
先手：先手太郎
後手：後手次郎
▲７六歩    △３四歩    ▲２二角成  △同　銀    ▲４五角
まで5手で先手の勝ち
```

KI2 のパーサとエクスポータはいずれも内部で `Board` を進めながら合法手を列挙し、各指し手の曖昧性解消を動的に行う。

## 2. コメントと評価値（`**評価値=`）の扱い

### コメント行の構文規則

- **ヘッダ内**：`#` で始まる行はファイルコメント行として無視される。なお、手順開始後に `#` 行を記述する構文はサポートされておらず、手順セクション内で扱われるコメントは `*`（または `'`）で始まる行のみである。
- **初期局面コメント**：初手（最初の `▲` / `△` / `▽`）より前に現れる `*` 行は、`Record::initial_comment()` に格納される。
- **指し手コメント**：指し手の後に現れる `*` 行は、直前の指し手ノードの `comment` に格納される。コメントを挿入する場合、その直前の指し手で一旦改行される。

### `**評価値=` の入出力仕様

KI2 における `**評価値=` 行の扱いは KIF とは異なる。

- **入力（パース）時**：`**評価値=...` 行は `EngineInfo.eval` へ変換されず、先頭の `*` を除いた通常の指し手コメント（`評価値=...`）として直前の指し手の `comment` に格納される。
- **出力（エクスポート）時**：指し手の `RecordAnnotation` に `EngineInfo.eval` が設定されている場合、エクスポータは符号反転を行わず、`EngineInfo.eval` の生の整数値（手番視点）をそのまま `**評価値=<raw i32>` 行として出力する。

## 3. 終局要約行と欠落時の挙動

KI2 では、手順の末尾に `までN手で...` という要約行を記述して終局状態を表す。

| 要約行の例 | 設定される `SpecialMove` | 設定される `GameResult` |
| :--- | :--- | :--- |
| `まで95手で先手の勝ち` | `SpecialMove::Resign` | `GameResult::BlackWin` |
| `まで102手で後手の勝ち` | `SpecialMove::Resign` | `GameResult::WhiteWin` |
| `まで80手で千日手` | `SpecialMove::RepetitionDraw` | `GameResult::DrawByRepetition` |
| `まで180手で持将棋` | `SpecialMove::Impasse` | `GameResult::DrawByImpasse` |
| `まで256手で最大手数` | `SpecialMove::MaxMoves` | `GameResult::DrawByMaxPlies` |
| `まで40手で中断` | `SpecialMove::Interrupt` | `GameResult::Paused` |
| `まで51手で先手の反則勝ち` | `SpecialMove::WinByIllegalMove` | `GameResult::BlackWinByIllegalMove` |
| `まで51手で後手の反則負け` | `SpecialMove::LoseByIllegalMove` | `GameResult::BlackWinByIllegalMove` |
| `まで75手で入玉宣言勝ち` | `SpecialMove::WinByDeclaration` | 直前の指し手側の入玉宣言勝ち |
| `まで15手で詰み` | `SpecialMove::Mate` | 直前の指し手側の勝ち |

### 要約行が欠落している場合の挙動

KIF や CSA とは異なり、KI2 パーサ（`parse_ki2_str` / `parse_ki2_bytes`）は入力末尾に要約行が存在しない場合、自動的に本手順の終局特殊手に `SpecialMove::Interrupt` を設定し、対局結果を `GameResult::Paused`（中断）とする。
一方、エクスポート時に `Record` の `main_terminal` が `None` である場合は、要約行を出力せずに手順のみを書き出す。

## 4. 変化手順（分岐）

KIF と同様に、本手順の後に `変化：N手` セクションを記述して分岐手順を表現できる。

```text
変化：3手
▲２六歩    △８四歩
```

## コード例

### Python

```python
from rsshogi.core import Board, Move
from rsshogi.record import Record

# KI2 文字列のパースと書き出し
record = Record.from_ki2_str(ki2_text)
output_text = record.to_ki2()
record.write_ki2("output.ki2", encoding="utf-8")

# 特定の局面における 1 手の KI2 表記を取得する
board = Board()
mv = Move.from_usi("7g7f")
print(mv.to_ki2(board))  # "▲７六歩"
```

### Rust

Rust では、KI2 のパースおよびエクスポート関数は `rsshogi::records::formats::kif` モジュールで提供され、`rsshogi::records` および `rsshogi::records::formats` の直下にも再エクスポートされている（`ki2` という名前のサブモジュールは存在しない）。

```rust
use rsshogi::records::formats::{
    common::{ExportOptions, TextEncoding},
    kif::{export_ki2, export_ki2_bytes_with_options, parse_ki2_bytes, parse_ki2_str},
};

let record = parse_ki2_str(ki2_str)?;
let utf8_ki2 = export_ki2(&record)?;

let sjis_ki2 = export_ki2_bytes_with_options(
    &record,
    ExportOptions::new(TextEncoding::ShiftJis),
)?;
let sjis_bytes = sjis_ki2.into_bytes();
```

## 関連項目

- [KIF フォーマット仕様](kif.md)
- [Record と棋譜データモデル](../records.md)
