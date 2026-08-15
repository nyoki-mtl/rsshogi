# 局面

> **前提知識**: [基本型](../types/index.md)

`Position` は盤面、持ち駒、手番、手数、および make/unmake に必要な状態を保持する。
公開 API は局面の構築、照会、合法性判定、更新、ハッシュを提供する。

## 局面がまとめる情報

盤面は各升の `Piece` と駒集合用の bitboard で表現される。
持ち駒は先後ごとの `Hand`、手番は `Color`、手数は `Ply` として保持される。
更新に伴う直前手、捕獲駒、反復、キー、王手・ピンなどは current state と履歴に結び付く。

この分割により、盤上の一点を読む `piece_on` と、駒集合を使う `pieces_for` のどちらも公開 API から選べる。
呼び出し側は内部ストレージを直接同期する必要がない。

## 局面を作る

`Position::from_sfen` は SFEN を解析して局面を作る。
`Position::empty` は空の局面を作り、`set_sfen`、`set_position_state`、`set_hirate` は既存局面を置き換える。
これらの構築 API はビットボード、ハッシュ、戦術キャッシュを現在の盤面から再構築する。

```rust,ignore
use rsshogi::board::Position;

let position = Position::from_sfen(
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
)?;
assert_eq!(position.to_sfen(None),
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1");
# Ok::<(), rsshogi::board::parser::SfenError>(())
```

`set_position_state` は構造化済みの `PositionState` を受け入れるが、将棋のルール上の妥当性は検証しない。
入力局面を検査する用途では `validate()` または `validate_all()` を使う。

`validate()` は最初に見つけた問題を `ValidationError` として返す。
`validate_all()` は玉の枚数、二歩、持ち駒の上限、行き所のない歩・香・桂をすべて `ValidationReport` に集める。
詰将棋や駒落ちのように玉が欠ける局面を許すかどうかは、用途に応じて report の `ValidationIssue::NoKing` を扱う側で決める。

SFEN の構文エラーと盤面規則のエラーは別の層である。
`from_sfen` の成功は SFEN を読めたことを表し、二歩や行き所のない駒まで受け入れる用途では別途 `validate` を呼ぶ。

## 読み取り API

盤上の駒は `piece_on(square)`、持ち駒は `hand(color)`、手番は `turn()`、手数は `game_ply()` で取得する。
ビットボードを必要とする処理には `bitboards()`、特定の駒集合には `pieces_for` や `pieces_for_types` を使う。
玉の位置、王手駒、ピン候補は `king_square`、`checkers`、`blockers_for_king`、`pinners` で取得する。

```rust,ignore
use rsshogi::types::{Color, Piece, SQ_77};

assert_eq!(position.piece_on(SQ_77), Piece::B_PAWN);
assert_eq!(position.turn(), Color::BLACK);
assert!(!position.is_in_check());
```

`current_state_cache()` は `checkers`、`check_squares`、`blockers_for_king`、`pinners` を同じ current state からまとめて読むための view を返す。
この view は `Position` への borrow なので、局面を更新した後に使わず、必要なら再取得する。

`gives_check_move` と `gives_check_move32` は候補が相手玉へ王手するかを局面を変えずに調べる。
`is_capture_move`、`is_capture_or_promotion`、`is_pawn_promotion` は手の並べ替えや評価差分の入口になる。

| 問いたいもの | API | 返り値 |
|---|---|---|
| 手番側の玉への王手 | `is_in_check()` | `bool`。 |
| 王手している駒 | `checkers()` | `Bitboard`。 |
| 指定色の玉の遮蔽駒 | `blockers_for_king(color)` | `Bitboard`。 |
| 指定駒種の王手候補升 | `check_square(piece_type)` | `Bitboard`。 |
| 直前の手と捕獲駒 | `last_move()`、`captured_piece()` | `Move32`、`Piece`。 |
| 現在局面のキー | `key()`、`board_key()` | `ZobristKey`。 |

## 合法性とルール

`is_pseudo_legal_move` と `is_pseudo_legal_move32` は駒の動き、成り、駒打ちなどの局所条件を調べる。
`is_legal_move` と `is_legal_move32` は玉を危険にさらす手と打ち歩詰めも除外する。
通常は生成された `Legal` または `LegalAll` の手をそのまま適用し、個別入力の検証にだけ合法性 API を使う。

`repetition_state()` は千日手、連続王手千日手、優劣局面を `RepetitionState` で返す。
`evaluate_declaration()` は入玉宣言の現在の規則、可否、条件の詳細を返す。
`declaration_win_move()` は宣言できるときだけ `MOVE_WIN` を返す。

## 更新の入口

通常手は `apply_move` または `apply_move32` で適用し、対応する `undo_move` または `undo_move32` で戻す。
これらの apply API は指し手を再検証しないため、呼び出し側は合法な手を渡す。
探索で事前に王手判定を済ませた場合は `apply_move32_with_gives_check` を使える。

`try_apply_search_move32_with_facts` は検索用 capacity を使う fallible な更新入口である。
この API は capacity が足りない場合に `MoveError` を返すが、渡した指し手の合法性を検査する API ではない。

`apply_move32_with_delta` と `apply_move32_with_facts` は、更新で得た差分または検索連携用の事実を返す。
詳細は [差分更新と状態管理](./state-management.md) を参照する。

`to_sfen(None)` は Position に記録された手数を出力する。
`to_sfen(Some(negative_value))` は手数フィールドを出力せず、`to_sfen_flipped` は盤面を 180 度回転して色と手番を反転した SFEN を出力する。

## 守るべき不変条件

公開更新 API は盤面、bitboard、玉位置、キー、current state をまとめて更新する。
一部だけを更新する setter は公開せず、入力局面の一括置換は構築 API に限定する。
探索側は `Position` の clone を枝ごとに共有せず、枝内では apply/undo の LIFO 対応を守る。

`undo_move32` の引数は直前に apply した手と同じでなければならない。
通常手の undo と null move の undo を混ぜると履歴の意味が崩れるため、各枝で操作種別を対応させる。

## 次に読む

→ [差分更新と状態管理](./state-management.md) で apply/undo の境界と履歴の読み方を確認する。
