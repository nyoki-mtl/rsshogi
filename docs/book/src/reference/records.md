# Record と棋譜の構造

`Record` は棋譜全体を表す型で、初期局面・開始局面コメント・指し手列・メタデータ・終局情報を保持します。
本手順と分岐（変化手順）の両方を扱えます。

## 終局情報モデル

- 勝敗/引き分け種別は `Record.result()` で `GameResult` として取得します
- 終局行そのもの（投了・中断・千日手・最大手数・持将棋など）は、本手順末尾の終局特殊手ノードとして保持されます
- 終局特殊手が存在しない棋譜では `Record.result()` は `GameResult::Invalid` になります
- 終局特殊手が存在しない棋譜では `Record.main_terminal()` も `None` になります

## 対応フォーマット

各フォーマットの詳細な仕様については、[棋譜フォーマット一覧](formats/index.md) を参照してください：

- [KIF形式](formats/kif.md)：人間が読みやすい標準形式
- [KI2形式](formats/ki2.md)：KIFの簡略版
- [CSA形式](formats/csa.md)：コンピュータ将棋協会の標準形式
- [JKF形式](formats/jkf.md)：JSON形式の棋譜フォーマット

## パース時の動作

- 指し手に紐づく KIF の `*...` / CSA の `'*...` コメントは指し手ノードの annotation へ取り込み
- 初手前の KIF の `*...` / CSA の `'*...` コメントは `Record.initial_comment` / `Record::initial_comment()` へ取り込み
- 明示的な `備考:` / `$NOTE:` は `RecordMetadata.comment` へ取り込み
- 消費時間は指し手ノードまたは終局特殊手ノードの annotation へ取り込み
- KIF の指し手行末尾にある `(<elapsed>/<total>)` 形式の消費時間も取り込み
- ヘッダは `RecordMetadata` に集約
- `変化：N手` は本手順上の指定手数からの分岐として扱う
- KIF/KI2 の `まで...手で最大手数` / `まで...手で持将棋` は正しく終局種別へ反映
- CSA の `%...` 終局行に後続する `T...` と `'*...` コメント行は終局特殊手に保持
- KIF/KI2/CSA では終局行が欠けた棋譜も受理し、その場合は終局特殊手なしで保持

## 書き出し時の動作

| フォーマット | 本手順 | 分岐 | 備考 |
|-------------|--------|------|------|
| KIF | ✅ | ✅ | `変化：N手` で出力 |
| KI2 | ✅ | ✅ | `変化：N手` で出力 |
| CSA | ✅ | ❌ | 仕様上の制限 |
| JKF | ✅ | ✅ | `forks` フィールドとして出力 |

- KIF/KI2/CSA は終局特殊手がない `Record` も出力でき、その場合は終局行を省略します

## Rust core の補助 API

Rust crate では通常の parse/export に加えて、棋譜ツリーを扱う補助 API を提供します。

- `records::formats::traversal::traverse_with_position(&record, visitor)`
  - DFS で全手順を走査し、各ノードで指し手適用後の `Position` を受け取れます
- `records::formats::traversal::position_at(&record, node_id)`
  - 任意ノード時点の局面を復元します
- `kif::export_kif_bytes()` / `kif::export_ki2_bytes()` / `csa::export_csa_bytes()`
  - `TextEncoding::{Utf8, ShiftJis}` または `ExportOptions` を指定してバイト列を書き出せます
- `EncodedText::has_unmappable_chars()`
  - Shift_JIS に変換できない文字が含まれていたかを確認できます

## 制限事項

- **評価値の出力**: KIF/CSA/KI2/JKF の出力では評価値を含めません
- **Python API の終局特殊手**: 本手順末尾は `Record.main_terminal`、任意ノードは `node_terminal(node_id)` で取得できます
- **Python API の分岐**: `Record.moves` は本手順のみを返します。分岐の走査には `children()` / `node_move()` / `node_terminal()` を使用してください
- **Python API の対応範囲**: callback ベースの traversal API や `*_bytes` API は未公開です。Python では `write_kif(..., encoding=...)` / `write_csa(..., encoding=...)` / `write_ki2(..., encoding=...)` を使用してください

## 関連項目

- [Record API（Python）](../python/record.md)
- [sbinpack 仕様](sbinpack.md)：バイナリ棋譜形式
