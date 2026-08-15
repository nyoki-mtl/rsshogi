# 合法手生成

> **前提知識**: [局面](../position/index.md) と [指し手](../types/moves.md)

手生成は `MoveGenType` marker 型で目的を指定する。
通常の呼び出し側は `generate_moves::<T>`、または `Move32` 用の対応 API を使う。

## 候補生成から手リストまで

盤上の駒は駒種ごとの attack pattern と occupancy から移動先候補を作る。
候補は mode に応じて捕獲、静かな手、王手、指定移動先で絞り込まれる。
`Legal` 系では、さらに玉の安全、ピン、王手回避、駒打ちの禁手を適用する。

```text
piece attacks + occupancy
        ↓
mode-specific candidates
        ↓
legal filtering when required
        ↓
Move or completed Move32 output
```

この順序は候補 mode と legal mode の責務を分ける。
候補を使う探索では必要な時点だけ合法性を確定し、外部へ出す手では legal mode を選ぶ。

## 基本 API

`generate_moves::<T>(position, list)` は `MoveList` を空にしてから `Move` を追加する。
`generate_moves_move32::<T>(position, list)` は同じ集合を移動後の駒情報付き `Move32` として追加する。
`*_into` 版は `MoveSink` または `Move32Sink` へ直接出力する。

```rust,ignore
use rsshogi::board::{movegen::{generate_moves, LegalAll}, MoveList};

let mut moves = MoveList::new();
generate_moves::<LegalAll>(&position, &mut moves);
assert!(moves.iter().all(|&mv| position.is_legal_move(mv)));
```

`Move` 出力はその 16 bit 表現だけを持つ。
`Move32` 出力は同じ下位 16 bit に、局面から得た移動後の駒を加える。
そのため `Move32` の集合を `to_move()` で変換した raw `Move` の集合は、対応する `Move` 出力と一致する。

`MoveList` と `Move32List` は生成済みの手を保持する固定容量リストである。
既存の順序を保って絞り込む必要があるときはリストの `retain` を使い、順序を問わない除外には `retain_unordered` を使える。

## `Legal` と `LegalAll`

`Legal` は現在局面で合法な手だけを生成し、探索向けの省略方針を適用する。
`Legal` は歩、香、角、飛に探索向けの成り優先方針を適用する。
`LegalAll` は同じ合法性条件で、可能な不成も含める。

成りが必須の升では、`LegalAll` も成り手だけを生成する。
`LegalAll` は完全な選択肢を必要とする検証、探索、詰み判定に適する。
`Legal` は通常の対局手を扱う経路に適する。

`generate_legal_all`、`generate_legal_all_move32`、`generate_legal_all_move32_into` は `LegalAll` の名前付き入口である。

```rust,ignore
use rsshogi::board::{movegen::{generate_moves, Legal, LegalAll}, MoveList};

let mut usual = MoveList::new();
let mut complete = MoveList::new();
generate_moves::<Legal>(&position, &mut usual);
generate_moves::<LegalAll>(&position, &mut complete);
assert!(usual.iter().all(|mv| complete.as_slice().contains(mv)));
```

不成が追加されない局面では両リストは同じ集合になる。
成り可能な歩、香、角、飛がある局面では `LegalAll` の方が多くなりうる。

## pseudo-legal mode と legal mode

`NonEvasions`、`Evasions`、`Checks`、`Captures` などの mode は用途別の候補を生成する。
実際に適用する手は legal check を通して確定する。
特に `Evasions` と `EvasionsAll` は王手回避の pseudo-legal 手であり、ピンによる王手放置や安全でない玉移動を含みうる。

王手局面の合法な回避手だけが必要なら `generate_legal_evasions` または `generate_legal_evasions_all` を使う。
任意の候補を個別に検査する場合は `Position::is_legal_move` または `is_legal_move32` を使う。

`NonEvasions` と `NonEvasionsAll` は非王手局面の候補生成に使う。
王手局面では `generate_legal_evasions` 系を使う。

`Checks` と `QuietChecks` は相手玉への王手という目的で候補を作る。
自玉が王手されている局面でこれらを使う場合、候補が回避も満たすとは限らないため、最終的な手には legal check が必要である。

## 主な generator mode

| Mode | 生成する手 | 合法性 |
|---|---|---|
| `Legal` / `LegalAll` | 合法手。`LegalAll` は任意不成を含む完全な集合 | 保証する。 |
| `Evasions` / `EvasionsAll` | 王手回避候補 | pseudo-legal。 |
| `NonEvasions` / `NonEvasionsAll` | 非王手局面の盤上手と駒打ち | 候補生成。 |
| `Captures` / `CapturesAll` | 捕獲手 | 候補生成。 |
| `CapturePlusPro` / `CapturePlusProAll` | 捕獲手と歩の成り手 | 候補生成。 |
| `Quiets` / `QuietsAll` | 非捕獲手 | 候補生成。 |
| `QuietsProMinus` / `QuietsProMinusAll` | 歩成りを除く非捕獲手 | 候補生成。 |
| `Checks` / `ChecksAll` | 王手となる手 | 候補生成。 |
| `QuietChecks` / `QuietChecksAll` | 捕獲しない王手 | 候補生成。 |
| `Recaptures` / `RecapturesAll` | 指定升への移動手 | 候補生成。 |

各 `*All` は、対応する mode で通常省略する歩、香、角、飛の不成を含める。
`generate_moves_to` と `generate_moves_to_move32` は `Recaptures` だけでなく、任意の mode の移動先を一つの升に絞れる。

`Recaptures` は指定升への盤上移動だけを対象にし、空き升への駒打ちは出さない。
`CapturePlusPro` は捕獲に加えて歩の成りを含むため、静止探索の候補を絞る用途に使える。

## 候補から完全な手へ

王手候補には `generate_checks`、`generate_checks_move32`、不成も含む `generate_checks_all_move32` を使える。
`generate_quiet_checks` は捕獲しない王手だけを出す。
これらの候補を実際に適用する前には、必要な legal check を行う。

`MoveListGen::<T>` は出力引数を使わずに固定容量の `MoveList` view を作る。
対象升が既知なら `MoveListGen::<T>::new_with_target` を使える。

候補を比較、永続化、または決定規則へ渡すときは、呼び出し側で必要な順序に並べる。

## 出力先を選ぶ

`MoveList` は compact な手だけを必要とする処理に適する。
`Move32List` は移動後の駒を使う apply、棋譜出力、評価差分に適する。
独自の固定配列やスコア付きキューへ直接出力するときは、`MoveSink` または `Move32Sink` を実装して `*_into` API を使う。

sink と list は同じ候補集合を受け取り、順序付けは利用側が担当する。
sink の `retain_unordered` は generator が候補を後段で除外するために必要な操作である。

## 次に読む

→ [特殊ルール](./special-rules.md) で、駒打ち、成り、王手回避の境界を確認する。
