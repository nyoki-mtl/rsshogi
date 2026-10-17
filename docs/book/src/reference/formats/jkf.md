# JKF フォーマット仕様

JKF（JSON Kifu Format）は、将棋の棋譜データを構造化された JSON オブジェクトとして表現するフォーマットである。
文字コードが UTF-8 に統一されており、Web アプリケーションやスクリプト言語から標準の JSON パーサだけで直接読み書きできる利点がある。

## トップレベル構造

JKF のルートオブジェクトは、`header`、`initial`（省略可）、および `moves` の 3 つのプロパティで構成される。

```json
{
  "header": {
    "先手": "先手太郎",
    "後手": "後手次郎",
    "棋戦": "テスト棋戦"
  },
  "initial": {
    "preset": "HIRATE"
  },
  "moves": [
    {
      "comments": ["初期局面コメント"]
    },
    {
      "move": {
        "color": 0,
        "from": { "x": 7, "y": 7 },
        "to": { "x": 7, "y": 6 },
        "piece": "FU"
      },
      "time": {
        "now": { "m": 0, "s": 3 },
        "total": { "m": 0, "s": 3 }
      }
    },
    {
      "move": {
        "color": 1,
        "from": { "x": 3, "y": 3 },
        "to": { "x": 3, "y": 4 },
        "piece": "FU"
      }
    },
    {
      "move": {
        "color": 0,
        "from": { "x": 8, "y": 8 },
        "to": { "x": 2, "y": 2 },
        "piece": "KA",
        "promote": true,
        "capture": "KA"
      }
    },
    {
      "special": "TORYO",
      "time": {
        "now": { "m": 0, "s": 5 }
      }
    }
  ]
}
```

## 1. `header` オブジェクト

対局メタデータを格納する文字列辞書である。
`rsshogi` は以下の日本語キーを `RecordMetadata` の標準フィールドと相互マッピングし、それ以外のキーをカスタム属性に格納する。

| JKF `header` キー | `RecordMetadata` の対応先 |
| :--- | :--- |
| `"先手"` | `black_player` |
| `"後手"` | `white_player` |
| `"棋戦"` | `event` |
| `"場所"` | `site` |
| `"開始日時"` | `start_date` |
| `"終了日時"` | `end_date` |
| `"持ち時間"` | `time_control` |
| `"先手持ち時間"` | `black_time_control` |
| `"後手持ち時間"` | `white_time_control` |
| `"最大手数"` | `max_moves` |
| `"持将棋"` | `impasse_rule` |
| `"備考"` | `comment` |

表に含まれないキー（`"戦型"`、`"手合割"` など）は、`RecordMetadata` のカスタム属性（Rust の `attributes()`、Python の `custom`）に格納される。

## 2. `initial` オブジェクト（初期局面）

`initial` が省略された場合は平手初期局面（`"HIRATE"`）として扱われる。
`rsshogi` がサポートする `preset` 文字列は、定型局面 14 種類と任意局面を表す `"OTHER"` の計 15 種類である。

| `preset` 値 | 手合割 | `preset` 値 | 手合割 |
| :--- | :--- | :--- | :--- |
| `"HIRATE"` | 平手 | `"3"` | 三枚落ち |
| `"KY"` | 香落ち | `"4"` | 四枚落ち |
| `"KY_R"` | 右香落ち | `"5"` | 五枚落ち |
| `"KA"` | 角落ち | `"5_L"` | 左五枚落ち |
| `"HI"` | 飛車落ち | `"6"` | 六枚落ち |
| `"HIKY"` | 飛香落ち | `"8"` | 八枚落ち |
| `"2"` | 二枚落ち | `"10"` | 十枚落ち |
| `"OTHER"` | 任意局面（`data` 必須） | | |

### `preset: "OTHER"` と `data.board` のインデックス順序

`"preset": "OTHER"` を指定する場合は、同階層に `"data"` オブジェクト（`color`、`board`、`hands`）を記述する。

- **`color`**：開始手番（`0` が先手番、`1` が後手番）。
- **`board`**：9 要素 × 9 要素の 2 次元配列であり、**外側の配列が筋（1 筋〜9 筋：`x = file - 1`）、内側の配列が段（一段〜九段：`y = rank - 1`）** を表す（`board[x][y]`）。すなわち `board[0]` は **1 筋**（`board[0][0]` が 1 一、`board[0][8]` が 1 九）であり、`board[8]` は **9 筋**（`board[8][0]` が 9 一、`board[8][8]` が 9 九）である。空きマスは空オブジェクト `{}` で表し、駒があるマスは `{"color": 0, "kind": "FU"}` のように記述する。
- **`hands`**：要素数 2 の配列 `[先手の持駒, 後手の持駒]` で、各要素は `{"FU": 0, "KY": 0, "KE": 0, "GI": 0, "KI": 2, "KA": 0, "HI": 0}` の形式で枚数を指定する。

```json
"initial": {
  "preset": "OTHER",
  "data": {
    "color": 0,
    "board": [
      [ { "color": 1, "kind": "OU" }, {}, {}, {}, {}, {}, {}, {}, {} ],
      [ { "color": 1, "kind": "FU" }, { "color": 1, "kind": "GI" }, {}, {}, {}, {}, {}, {}, {} ],
      [ {}, {}, {}, {}, {}, {}, {}, {}, {} ],
      [ {}, {}, {}, {}, {}, {}, {}, {}, {} ],
      [ {}, {}, {}, {}, {}, {}, {}, {}, {} ],
      [ {}, {}, {}, {}, {}, {}, {}, {}, {} ],
      [ {}, {}, {}, {}, {}, {}, {}, {}, {} ],
      [ {}, {}, {}, {}, {}, {}, {}, {}, {} ],
      [ {}, {}, {}, {}, {}, {}, {}, {}, {} ]
    ],
    "hands": [
      { "FU": 0, "KY": 0, "KE": 0, "GI": 0, "KI": 2, "KA": 0, "HI": 0 },
      { "FU": 17, "KY": 4, "KE": 4, "GI": 3, "KI": 2, "KA": 2, "HI": 2 }
    ]
  }
}
```

上記の例では、`board[0][0]`（1 一）に後手の玉、`board[1][0]`（2 一）に後手の歩、`board[1][1]`（2 二）に後手の銀が配置されている。

## 3. `moves` 配列と指し手オブジェクト

`moves` 配列はインデックス `0` の要素が常に初期局面を表し、インデックス `1` 以降が 1 手目、2 手目の指し手または終局特殊手を表す。
`moves[0]` には `"move"` を含めず、初期局面コメントがある場合にのみ `"comments"`（および先頭手からの分岐がある場合は `"forks"`）を持たせる。

### `move` オブジェクトのフィールド

| フィールド | 型 | 必須 | 説明 |
| :--- | :--- | :--- | :--- |
| `color` | `0` または `1` | ○ | 手番（`0`：先手、`1`：後手）。 |
| `from` | `{"x": 1..9, "y": 1..9}` | 盤上移動時 | 移動元の筋 `x` と段 `y`。駒打ちの場合は `from` を省略（または `null`）する。 |
| `to` | `{"x": 1..9, "y": 1..9}` | ○ | 移動先の筋 `x` と段 `y`。 |
| `piece` | `string` | ○ | **移動前の駒種コード**（CSA 2 文字表記）。 |
| `same` | `boolean` | 任意 | 直前の指し手と同一の移動先（同）である場合に `true`。 |
| `promote` | `boolean` | 任意 | 移動時に成る場合は `true`、成れる移動で成らない（不成）場合は `false`。 |
| `capture` | `string` | 任意 | 移動先にあった捕獲対象の駒種コード（成駒を取った場合は `"UM"` などの成駒コード）。 |
| `relative` | `string` | 任意 | 同種駒が同じマスへ動ける場合の相対位置または動作修飾子（`"L"`, `"C"`, `"R"`, `"U"`, `"M"`, `"D"`, `"LU"`, `"LM"`, `"LD"`, `"RU"`, `"RM"`, `"RD"`, `"H"`）。 |

### 成り手における `piece` フィールドの注意点

CSA 形式の指し手文字列（`+8822UM`）とは異なり、JKF の `move.piece` には **成る前の駒種（移動元にある駒の種別）** を指定する。
例えば、8 八の角を 2 二へ進めて馬に成る手では、`"piece": "KA"` かつ `"promote": true` と記述する（`"piece": "UM"` は既に馬に成っている駒を動かす場合のみ使われる）。

```json
{
  "move": {
    "color": 0,
    "from": { "x": 8, "y": 8 },
    "to": { "x": 2, "y": 2 },
    "piece": "KA",
    "promote": true,
    "capture": "KA"
  }
}
```

## 4. 消費時間（`time`）とコメント（`comments`）

各手オブジェクトには `"time"` と `"comments"` を付与できる。

- **`time.now` と `time.total`**：それぞれ 1 手の消費時間と累計消費時間を `{"h": 時間, "m": 分, "s": 秒}` で表す。`"h"` は 0 時間の場合には省略される（1 時間以上のときのみ `"h"` が出力される）。
- **終局特殊手における `time`**：`"special"` を持つ終局特殊手エントリでは `"time": {"now": ...}` のみが出力され、`"total"` は出力されない。
- **`comments`**：文字列の配列 `["1行目", "2行目"]` として保持される。JKF には評価値専用フィールドがないため、`rsshogi` からのエクスポート時に `EngineInfo.eval` は出力されない。

## 5. 終局特殊手（`special`）

終局を表すエントリは `"move"` の代わりに `"special"` フィールドを持つ。

| JKF `special` 値 | `SpecialMove` | 勝敗結果（`GameResult`） |
| :--- | :--- | :--- |
| `"TORYO"` | `SpecialMove::Resign` | 直前の指し手側の勝ち（`BlackWin` / `WhiteWin`）。 |
| `"CHUDAN"` | `SpecialMove::Interrupt` | `GameResult::Paused` |
| `"SENNICHITE"` | `SpecialMove::RepetitionDraw` | `GameResult::DrawByRepetition` |
| `"TIME_UP"` | `SpecialMove::Timeout` | 直前の指し手側の時間切れ勝ち（`BlackWinByTimeout` / `WhiteWinByTimeout`）。 |
| `"ILLEGAL_MOVE"` | `SpecialMove::WinByIllegalMove` | 直前の指し手側の反則勝ち（`BlackWinByIllegalMove` / `WhiteWinByIllegalMove`）。 |
| `"JISHOGI"` | `SpecialMove::Impasse` | `GameResult::DrawByImpasse` |
| `"KACHI"` | `SpecialMove::WinByDeclaration` | 宣言した手番側の入玉宣言勝ち（`BlackWinByDeclaration` / `WhiteWinByDeclaration`）。 |
| `"HIKIWAKE"` | `SpecialMove::Draw` | `GameResult::DrawByImpasse` |
| `"TSUMI"` | `SpecialMove::Mate` | 直前の指し手側の勝ち（`BlackWin` / `WhiteWin`）。 |
| `"FUZUMI"` | `SpecialMove::NoMate` | `GameResult::Paused` |
| `"MAX_MOVES"` | `SpecialMove::MaxMoves` | `GameResult::DrawByMaxPlies` |
| `"ERROR"` | `SpecialMove::Interrupt` | `GameResult::Error` |

上記以外の文字列（例えば `"MATTA"`、`"+ILLEGAL_ACTION"`、`"-ILLEGAL_ACTION"` など）が指定された場合は、`SpecialMove::Unknown` および `GameResult::Invalid` として保持される。

## 6. 分岐手順（`forks`）

ある手に対する別候補の分岐手順は、置き換え対象となる指し手の **1 つ前の要素**（すなわち分岐元の親局面に対応する要素）の `"forks"` フィールドに格納される。
`"forks"` は「指し手オブジェクト配列」の配列（`Vec<Vec<MoveFormat>>`）であり、複数の変化手順および再帰的な入れ子分岐を表現できる。

- **初手（1 手目）からの分岐**：`moves[0]` の `"forks"` に格納される。
- **N 手目（`N >= 2`）からの分岐**：`moves[N - 1]` の `"forks"` に格納される。

## コード例

### Python

```python
import json
from rsshogi.record import Record

# JKF 文字列またはファイルからの読み込み
record = Record.from_jkf_str(jkf_text)
record_from_file = Record.from_jkf_file("sample.jkf")

# JKF 文字列へのエクスポートと JSON パース
jkf_out = record.to_jkf()
data = json.loads(jkf_out)
```

### Rust

```rust
use rsshogi::records::formats::jkf::{export_jkf, parse_jkf_bytes, parse_jkf_str};

let record = parse_jkf_str(jkf_str)?;
let json_text = export_jkf(&record)?;
```

## 関連項目

- [Record と棋譜データモデル](../records.md)
- [CSA フォーマット仕様](csa.md)
