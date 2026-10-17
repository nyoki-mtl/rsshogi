# CSA フォーマット仕様

CSA 形式は、コンピュータ将棋協会（CSA）によって策定された標準棋譜フォーマットである。
すべて半角 ASCII 記号と英数字を主体として構成されており、通信対局プロトコルやプログラムによる自動解析に適している。
`rsshogi` は CSA 標準棋譜ファイル形式（V2.1 / V2.2 / V3.0）の入出力をサポートする。

## 基本構造とマルチゲーム区切り

CSA ファイルは以下の要素を上から順に並べて構成する。

1. **バージョン宣言と対局者情報**：`'CSA encoding=...`（V3.0）、`V2.2` または `V3.0`、`N+`、`N-`、および `$KEY:VALUE` 行。
2. **初期局面**：`PI`（平手または駒落ち）、`P1`〜`P9`（一括配置）、`P+` / `P-`（個別配置と持ち駒）、および手番行（`+` または `-`）。
3. **手順と消費時間**：`+7776FU` 形式の指し手、`T12` または `T6.123` 形式の消費時間、`'*` / `'**` コメント行、および `%TORYO` 形式の終局特殊手。

### カンマ連結とマルチゲーム区切り（`/`）

1 行の中にカンマ `,` を置くことで、複数の文（例えば指し手と消費時間 `+7776FU,T12`）を 1 行に連結して記述できる。
また、`/` のみの行は複数局を 1 つのファイルに格納する際のセパレータとして機能する。

## 1. ヘッダとメタデータ

- **バージョン行**：`V2.2` や `V3.0` などのバージョン指定行。エクスポート時は既定で `V2.2` を出力し、`ExportOptions::with_csa_version(CsaVersion::V3_0)` を指定したときは先頭に `'CSA encoding=UTF-8`（または `SHIFT_JIS`）と `V3.0` を出力する。
- **対局者名**：`N+<先手名>` で先手（下手）の対局者名、`N-<後手名>` で後手（上手）の対局者名を指定する。
- **拡張メタデータ（`$` 行）**：`$KEY:VALUE` 形式で対局情報を記述する。

| CSA キー | `RecordMetadata` の対応先 | 説明 |
| :--- | :--- | :--- |
| `N+` | `black_player` | 先手の対局者名。 |
| `N-` | `white_player` | 後手の対局者名。 |
| `$EVENT:` | `event` | 棋戦名または大会名。 |
| `$SITE:` | `site` | 対局場所。 |
| `$START_TIME:` | `start_date` | 対局開始日時。 |
| `$END_TIME:` | `end_date` | 対局終了日時。 |
| `$TIME_LIMIT:` / `$TIME:` | `time_control` | 共通の持ち時間設定（フィッシャー加算がある場合は `$TIME:` で書き出される）。 |
| `$TIME+:` / `$TIMET+:` | `black_time_control` | 先手の個別持ち時間設定（書き出しは `$TIME+:`）。 |
| `$TIME-:` / `$TIMET-:` | `white_time_control` | 後手の個別持ち時間設定（書き出しは `$TIME-:`）。 |
| `$MAX_MOVES:` | `max_moves` | 最大手数（`u32`）。 |
| `$JISHOGI:` | `impasse_rule` | 持将棋ルール（例：`24`、`27`）。 |
| `$NOTE:` | `comment` | 棋譜全体に対する備考テキスト（`\n` と `\\` のエスケープを相互変換）。 |

表以外の `$KEY:VALUE` 行は、キーから先頭の `$` を除いた名前で `RecordMetadata` のカスタム属性（Rust の `attributes()`、Python の `custom`）に保持される。

## 2. 初期局面と手番

初期局面の記述には、以下の 3 方式を組み合わせて使用できる。

### 平手および駒落ち表現（`PI`）

`PI` から始めて、平手初期配置から取り除く駒のマス座標と駒種を列挙する。

- 平手：`PI`
- 二枚落ち（後手の飛車と角を除く）：`PI82HI22KA`

### 一括配置表現（`P1`〜`P9`）

1 段目（`P1`）から 9 段目（`P9`）まで、9 筋から 1 筋へ向かって 1 マス 3 文字で盤面を記述する。
先手の駒は `+FU`、後手の駒は `-FU`、空きマスは ` * `（空白と `*` を組み合わせた 3 文字）で表す。

```text
P1-KY-KE-GI-KI-OU-KI-GI-KE-KY
P2 * -HI *  *  *  *  * -KA *
P3-FU-FU-FU-FU-FU-FU-FU-FU-FU
P4 *  *  *  *  *  *  *  *  *
P5 *  *  *  *  *  *  *  *  *
P6 *  *  *  *  *  *  *  *  *
P7+FU+FU+FU+FU+FU+FU+FU+FU+FU
P8 * +KA *  *  *  *  * +HI *
P9+KY+KE+GI+KI+OU+KI+GI+KE+KY
```

### 個別配置と持ち駒表現（`P+` / `P-`）

`P+`（先手）または `P-`（後手）に続けて、4 文字単位（座標 2 文字 + 駒種 2 文字）で駒の位置を指定する。
座標 `00` は持ち駒を表す。

```text
P+00KI00FU
P-11OU22GI
```

上記の `P+00KI00FU` は、先手が持ち駒に金 1 枚と歩 1 枚を持つことを表す。
`00AL` が指定された場合は、盤上および指定済みの駒を除いた残りのすべての駒をその手番側の持ち駒とする。

### 初期局面の開始手番

局面定義の末尾に `+`（先手番）または `-`（後手番）の 1 文字行を置いて開始手番を指定する。
CSA ヘッダ内に手番指定行（`+` または `-`）が存在しない場合、パーサはエラーとせずデフォルトで先手番（`+`）として初期局面を構築する。

## 3. 指し手と駒種コード

各指し手は `[手番(1文字)][移動元(2文字)][移動先(2文字)][移動後の駒種(2文字)]` の計 7 文字で記述する。

- **手番**：先手は `+`、後手は `-`。現在の手番と一致しない符号が与えられた場合は `CsaError::MoveSideMismatch` を返す。
- **移動元座標**：筋（`1`〜`9`）と段（`1`〜`9`）。持ち駒を打つ場合は `00` とする。
- **移動先座標**：筋（`1`〜`9`）と段（`1`〜`9`）。
- **移動後の駒種**：移動または成りが完了した時点での盤上の駒種コードを指定する。成る手では移動前の駒種ではなく成駒のコード（例えば角成なら `UM`）を記述する。

| 駒種コード | 意味 | 成駒コード | 意味 |
| :--- | :--- | :--- | :--- |
| `FU` | 歩 | `TO` | と金 |
| `KY` | 香 | `NY` | 成香 |
| `KE` | 桂 | `NK` | 成桂 |
| `GI` | 銀 | `NG` | 成銀 |
| `KI` | 金 | （なし） | 金は成れない |
| `KA` | 角 | `UM` | 馬 |
| `HI` | 飛 | `RY` | 龍 |
| `OU` | 玉 / 王 | （なし） | 玉は成れない |

```text
+7776FU
-3334FU
+8822UM
-3122GI
+0045KA
```

## 4. 消費時間（`T` 行）

指し手行または終局特殊手行の直後に `T<秒数>` を記述して、その手に要した消費時間を記録する。

- **精度と型**：V2.2 の整数秒表記（例：`T12`）および V3.0 で追加された小数点以下 3 桁までのミリ秒表記（例：`T6.123`）の双方を受理し、ミリ秒単位の `u32`（`time_ms`）へ変換する。
- **エクスポート時の出力形式**：既定の `CsaVersion::V2_2` では整数秒（`T6`）に切り捨てて出力する。`CsaVersion::V3_0` を指定し、かつミリ秒端数（`time_ms % 1000 != 0`）がある場合は `T6.123` の形式で出力する。

## 5. コメントと `'**` 解析行

CSA 形式ではアポストロフィ `'` で始まる行がコメントおよび解析情報として扱われる。

- **通常コメント（`'`）**：先頭が `'`（ただし `'*` 以外）の行はファイルコメントとして読み飛ばされる。
- **プログラムコメント（`'*`）**：`'*`（ただし `'**` 以外）で始まる行は、初手より前であれば `Record::initial_comment()` に、指し手や終局特殊手の後であれば直前のノードの `comment` に格納される。
- **評価値と読み筋（`'**`）**：
  - V3.0 の `'** <評価値> <読み筋...> #<ノード数>` 形式の行（および非標準の `'**評価値=<数値>` 行）は、直前の指し手または終局特殊手ノードの `EngineInfo` に取り込まれる。
  - 評価値（`i16` 範囲内）は `EngineInfo.eval`、ノード数は `EngineInfo.nodes`、読み筋文字列は `EngineInfo.extras["csa_pv"]` に格納される。
  - CSA 入出力では wire 往復互換性を維持するため、評価値の符号反転は行わずファイル上の数値をそのまま保持および再出力する。
  - 評価値として解釈できない `'**` 行や `i16` の範囲を超える評価値行は `EngineInfo.extras["csa_analysis_raw"]` に原文のまま退避され、エクスポート時にそのまま書き戻される。

## 6. 終局特殊手（`%` 行）

`%` で始まる行は終局特殊手や対局終了事由を表す。

| CSA 表記 | `SpecialMove` | 勝敗結果（`GameResult`） |
| :--- | :--- | :--- |
| `%TORYO` | `SpecialMove::Resign` | 直前の指し手側（手番の逆側）の勝ち（`BlackWin` / `WhiteWin`）。 |
| `%CHUDAN` | `SpecialMove::Interrupt` | `GameResult::Paused` |
| `%SENNICHITE` | `SpecialMove::RepetitionDraw` | `GameResult::DrawByRepetition` |
| `%TIME_UP` | `SpecialMove::Timeout` | 直前の指し手側の時間切れ勝ち（`BlackWinByTimeout` / `WhiteWinByTimeout`）。 |
| `%ILLEGAL_MOVE` | `SpecialMove::WinByIllegalMove` | 直前の指し手側の反則勝ち（`BlackWinByIllegalMove` / `WhiteWinByIllegalMove`）。 |
| `%+ILLEGAL_ACTION` | `SpecialMove::WinByIllegalMove` | 先手の反則行為による後手勝ち（`GameResult::WhiteWinByIllegalMove`）。 |
| `%-ILLEGAL_ACTION` | `SpecialMove::WinByIllegalMove` | 後手の反則行為による先手勝ち（`GameResult::BlackWinByIllegalMove`）。 |
| `%JISHOGI` | `SpecialMove::Impasse` | `GameResult::DrawByImpasse` |
| `%HIKIWAKE` | `SpecialMove::Draw` | `GameResult::DrawByImpasse` |
| `%MAX_MOVES` | `SpecialMove::MaxMoves` | `GameResult::DrawByMaxPlies` |
| `%KACHI` | `SpecialMove::WinByDeclaration` | 宣言した手番側の入玉宣言勝ち（`BlackWinByDeclaration` / `WhiteWinByDeclaration`）。 |
| `%TSUMI` | `SpecialMove::Mate` | 直前の指し手側の勝ち（`BlackWin` / `WhiteWin`）。 |
| `%FUZUMI` | `SpecialMove::NoMate` | `GameResult::Paused` |
| `%ERROR` | `SpecialMove::Unknown("ERROR")` | `GameResult::Error` |

上記以外の `%...` マーカーが入力された場合は `SpecialMove::Unknown` および `GameResult::Paused` として保持される。
`%` 行が存在しない棋譜は `main_terminal = None` かつ `GameResult::Invalid` となる。

## 7. 単一局およびマルチゲーム API

CSA ファイルには `/` 行で区切って複数局を収録できる。
単一局 API（`parse_csa_str`、`Record.from_csa_str`、`Record.from_csa_file`）に複数局を含むデータを与えた場合は、最初の `/` 区切りまでの先頭 1 局のみをパースする。
ファイル内の全対局をリストとして取得するには、マルチゲーム用 API を使用する。

### Python

```python
from rsshogi.core import Board
from rsshogi.record import Record

# 単一局の読み込みと書き出し
record = Record.from_csa_file("single_game.csa")
csa_str = record.to_csa()
record.write_csa("output.csa", encoding="utf-8")

# '/' 区切りのマルチゲーム CSA を一括読み込み
records_from_file = Record.from_csa_games_file("multi_games.csa")
records_from_str = Record.from_csa_games_str(multi_csa_text)

# CSA 文字列から盤面上の指し手を復元し、単独で CSA 文字列に戻す
board = Board()
mv32 = board.move_from_csa("+7776FU")
print(mv32.to_csa())  # "+7776FU"
```

### Rust

```rust
use rsshogi::records::formats::{
    common::{CsaVersion, ExportOptions, TextEncoding},
    csa::{
        export_csa, export_csa_with_options, parse_csa_bytes, parse_csa_games,
        parse_csa_games_bytes, parse_csa_str,
    },
};

// 単一局のパースと V2.2（既定）エクスポート
let record = parse_csa_str(csa_text)?;
let exported_v2 = export_csa(&record)?;

// V3.0 形式でのエクスポート
let options_v3 = ExportOptions::new(TextEncoding::Utf8).with_csa_version(CsaVersion::V3_0);
let exported_v3 = export_csa_with_options(&record, options_v3)?;

// '/' 区切りのマルチゲーム CSA の一括パース
let games: Vec<_> = parse_csa_games(multi_csa_text)?;
let games_from_bytes: Vec<_> = parse_csa_games_bytes(&raw_bytes)?;
```

## 関連項目

- [Record と棋譜データモデル](../records.md)
- [フォーマット仕様一覧](index.md)
