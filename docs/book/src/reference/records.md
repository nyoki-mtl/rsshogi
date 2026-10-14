# Record と棋譜データモデル

`Record` は 1 局の棋譜全体を表現するデータ構造である。
初期局面、初期局面コメント、対局メタデータ、本手順および変化手順を保持する有向木、ならびに対局結果から構成される。

## 全体構造と主要型

`Record` を構成する主要な型と責務は以下の通りである。

| 型 | 役割 |
| :--- | :--- |
| `Record` | 初期局面（`RecordInitialPosition`）、`RecordMetadata`、手順ツリー、および `GameResult` を統合管理する棋譜コンテナ。 |
| `RecordMetadata` | 対局者名、棋戦名、日時、持ち時間（`TimeControl`）、最大手数、持将棋ルール、カスタム属性を保持するヘッダ情報。 |
| `RecordNodeId` | 手順ツリー内のノードを一意に識別するインデックス型（`0` は常に初期局面を表す root ノード）。 |
| `RecordNode` | 親ノード ID、子ノード ID リスト（`children`）、`RecordEntry`（root では `None`）、および `RecordAnnotation` を保持するツリーノード。 |
| `RecordEntry` | ノードの具体要素（`Move(MoveEntry)` または `Special(SpecialMoveEntry)`）。 |
| `MoveEntry` | 1 手分の指し手（`Move`）を保持するエントリ（Python の `Record.moves` では消費時間やコメント等を含む 1 手分のエントリとして公開される）。 |
| `SpecialMoveEntry` | 投了や千日手などの終局特殊手種別（`SpecialMove`）、対局結果（`GameResult`）、および原文文字列（`raw`）の組。 |
| `RecordAnnotation` | 1 手の消費時間（`elapsed_ms`）、コメント文字列（`comment`）、および `EngineInfo` を保持する注釈データ。 |
| `EngineInfo` | 探索評価値（`Eval`）、探索深さ（`depth` / `seldepth`）、探索ノード数（`nodes`）、および拡張属性辞書（`extras`）を保持するエンジン解析情報。 |
| `SpecialMove` | 投了、中断、千日手、入玉宣言勝ち、反則、最大手数などの終局事由を表す列挙型（全 16 種）。 |
| `GameResult` | 先手勝ち、後手勝ち、千日手引分、持将棋引分、最大手数引分、中断、エラー、未確定を表す対局結果（全 18 種）。 |
| `RecordEditor` | 現在位置（カーソル）と `Position` を同期しながら、合法手検証付きの指し手追加、分岐作成、ツリー移動、棋譜マージを行う編集ヘルパ。 |

## 手順ツリーと `RecordNodeId`

`Record` の内部では、本手順とすべての変化手順が単一のアリーナ配列上の木構造として管理される。
`Record::root_id()`（`raw == 0`）は常に初期局面を表す root ノードであり、各 `RecordNode` は親ノード ID（`parent()`）と子ノード ID のスライス（`children()`）を持つ。

あるノードに複数の子ノードが存在する場合、`children[0]`（`main_child(node_id)`）がその枝の主手順（本手順または変化の主ライン）を表し、`children[1..]`（`variation_children(node_id)`）が同手数への別分岐（変化手順）を表す。
`Record::main_moves()`（Python では `Record.moves`）は root ノードから常に `children[0]` を辿って得られる本手順の指し手列を返す。

ツリー全体の走査や各ノードへのアクセスには、以下のメソッドを使用する。

- **`Record::root_id()` / `Record::root()`**：ルートノード ID（`0`）およびルート `RecordNode` への参照を返す。
- **`Record::node(node_id)` / `Record::try_node(node_id)`**：指定 ID の `RecordNode` を取得する（外部由来の ID には `Result` を返す `try_node` を使用する）。
- **`Record::children(node_id)` / `Record::variation_children(node_id)`**：全子ノード ID、または `children[1..]` の分岐子ノード ID スライスを返す。
- **`RecordNode::mv()` / `RecordNode::special()`**：ノードが指し手であれば `Some(&MoveEntry)`、終局特殊手であれば `Some(&SpecialMoveEntry)` を返す。
- **`RecordNode::annotation()`**：ノードに付随する `RecordAnnotation`（`comment()`、`time_ms()`、`engine_info()`）を参照する。
- **`Record::main_line_ids()`**：本手順上のすべての指し手ノード ID を先頭から順に返す。

## 終局表現と `GameResult`

`Record` は対局の終局状態を「終局特殊手エントリ（`SpecialMoveEntry`）」と「対局結果（`GameResult`）」の 2 層で管理する。

- **終局特殊手（`SpecialMoveEntry`）**：本手順や分岐手順の末尾に配置される葉ノードであり、投了（`SpecialMove::Resign`）や時間切れ（`SpecialMove::Timeout`）などの具体的な終局事由と、対応する `GameResult` および原文表現（`raw`）を保持する。本手順の終局特殊手は `Record::main_terminal()`（ノード ID は `Record::main_terminal_node()`）で取得できる。
- **対局結果（`GameResult`）**：本手順の `main_terminal` が持つ勝敗結果を返す集計値であり、`Record::result()`（Python では `Record.result`）で取得できる。`main_terminal` が `None` の場合は `GameResult::Invalid` となる。

### フォーマット別の終局行欠落時の挙動

棋譜ファイルに終局行や要約行が存在しない場合の挙動は、フォーマットの仕様によって異なる。

| フォーマット | 終局行や要約行が欠落している場合の `main_terminal` | 終局行や要約行が欠落している場合の `GameResult` | 理由 |
| :--- | :--- | :--- | :--- |
| KIF | `None` | `GameResult::Invalid` | 終局特殊手なしで手順のみを保持する。 |
| CSA | `None` | `GameResult::Invalid` | `%TORYO` 等の終局行がない場合は未決着手順として保持する。 |
| KI2 | `Some(SpecialMove::Interrupt)` | `GameResult::Paused` | KI2 形式では要約行（`までN手で...`）がない棋譜を中断局として扱う。 |
| JKF | `None` | `GameResult::Invalid` | 末尾に `"special"` フィールドを持つ手がない場合は未決着として保持する。 |

KIF、KI2、CSA、JKF の各エクスポータは、`main_terminal` が `None` の `Record` であってもエラーにせず、終局行を省略して手順のみを書き出すことができる。

## 評価値（`EngineInfo.eval`）の視点規則

`RecordAnnotation` 内の `EngineInfo.eval` は、原則として **その手を指す直前の手番側（side-to-move before the move）から見た評価値（単位：cp、`Eval(i16)`）** として保持される。
先手番の指し手では先手有利が正の値となり、後手番の指し手では後手有利が正の値となる。

外部フォーマットとの入出力時における評価値の扱いは以下の通りである。

| フォーマット | 入力（パース）時の挙動 | 出力（エクスポート）時の挙動 |
| :--- | :--- | :--- |
| KIF | 本手順および変化手順の `**評価値=` 行を先手視点として読み取り、後手番の手では符号を反転して手番視点の `EngineInfo.eval` に格納する。 | 本手順および変化手順の `EngineInfo.eval` を先手視点に変換（後手番の手では符号を反転）して `**評価値=` 行として出力する。 |
| KI2 | `**評価値=` 行も通常の指し手コメント（`評価値=...`）として `comment` に格納する。 | `EngineInfo.eval` の生の整数値を符号反転なしで `**評価値=<raw i32>` として出力する。 |
| CSA | `'** <eval> <pv> #<nodes>` 行（および `'**評価値=<eval>`）を解析し、wire 互換性維持のため符号反転を行わずそのまま `EngineInfo.eval`、`nodes`、`extras["csa_pv"]` に格納する。 | `EngineInfo` の `eval`、`extras["csa_pv"]`、`nodes`、および退避された `extras["csa_analysis_raw"]` を符号反転なしで `'** ...` 行として出力する。 |
| JKF | JKF 標準仕様に評価値フィールドがないため対象外。 | 評価値は出力しない。 |
| SBINPACK | ステム局面の `score` は `Record` にコピーしない。各手の評価値差分を復元して `EngineInfo.eval`（手番視点）に格納する。 | 各手の `EngineInfo.eval`（手番視点）をパリティ正規化差分として符号化する。 |

## パース時とエクスポート時の共通規則

各テキストフォーマットのパーサとエクスポータは以下の規則に従ってデータをマッピングする。

- **初期局面コメント**：初手の指し手より前に出現する KIF/KI2 の `*` コメント行および CSA の `'*` コメント行は、`Record::initial_comment()`（root ノードの `comment`）に格納される。
- **指し手コメント**：各指し手の直後に続くコメント行は、その指し手ノードの `RecordAnnotation::comment()`（複数行は `\n` で連結）に格納される。
- **消費時間**：KIF の `(<elapsed>/<total>)`、CSA の `T<seconds>`（V3.0 のミリ秒小数表記 `T6.123` を含む）、JKF の `"time.now"` オブジェクトは、対応する指し手または終局特殊手ノードの `time_ms`（ミリ秒単位の `u32`）として格納される。KIF や JKF のエクスポート時には、各ノードの `time_ms` を手番ごとに累積して累計消費時間を自動計算する。
- **分岐手順の表現**：KIF と KI2 は `変化：N手` セクションで分岐を入出力し、JKF は指し手オブジェクトの `"forks"` 配列で分岐を入出力する。CSA 形式はフォーマット仕様上、分岐手順の構文を持たないため本手順のみを扱う。

## Rust コアの編集および走査 API

Rust クレート（`rsshogi::records` feature 有効時）では、棋譜ツリーの編集と局面復元のための専用 API を提供する。

- **`RecordEditor`**：`RecordEditor::new(initial_position)` または `RecordEditor::from_record(record)` で生成し、`go_to`、`go_back`、`go_forward`、`branch_to`、`append_move`、`append_move_with_annotation`、`append_special`、`merge_record` などを用いて現在位置の合法性を検証しながら棋譜ツリーを編集できる。
- **`traversal::traverse_with_position(&record, visitor)`**：深さ優先探索（DFS）で全ノードを走査し、指し手ノードでは指し手適用後の `Position`、終局特殊手ノードでは終局直前の `Position` をコールバックへ渡す。
- **`traversal::position_at(&record, node_id)`**：任意の `RecordNodeId` 時点における `Position` を初期局面から再生して復元する。
- **`export_*_bytes` / `export_*_bytes_with_options`**：`ExportOptions::new(TextEncoding::ShiftJis)` や `.with_csa_version(CsaVersion::V3_0)` を指定してバイト列を生成する。返り値の `EncodedText::has_unmappable_chars()` により、Shift_JIS へ変換できなかった文字の有無を検査できる。

## 関連項目

- [フォーマット仕様一覧](formats/index.md)
- [Record API（Python）](../python/record.md)
- [sbinpack フォーマット仕様](sbinpack.md)
