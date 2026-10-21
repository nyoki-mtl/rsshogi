# 探索エンジン連携プリミティブ

`Position` は、指し手の適用結果と現在の戦術キャッシュを利用側が再利用するための primitive を提供する。
本ページでは、`MoveApplyFacts`、`StateCacheView`、探索側のキー予測の責務、千日手判定、独立した探索用コピーを解説する。

## 1. 指し手の適用結果と戦術キャッシュ

`Position::try_apply_move32_with_facts(mv)` は、局面を更新して `Result<MoveApplyFacts, MoveError>` を返す。
`MoveApplyFacts` は、更新中に確定した情報を利用側が盤面から再取得せずに利用するための型である。

| フィールド | 内容 |
|---|---|
| `mv`、`from`、`to` | 適用した `Move32` と移動元、移動先。駒打ちの `from` は `None`。 |
| `moved_piece_before`、`moved_piece_after` | 移動前後の駒。 |
| `captured_piece` | 捕獲した駒。捕獲がない場合は `Piece::NONE`。 |
| `dropped_piece_type`、`promoted`、`moved_king`、`gives_check` | 駒打ち、成り、玉の移動、王手の情報。 |
| `key_after`、`board_key_after`、`partial_keys_after` | 適用後の局面キー、盤面キー、部分キー群。 |
| `delta` | 駒打ち、移動、成り、捕獲を表す `MoveDelta32` 型の盤面差分。 |

捕獲前または駒打ち前の持ち駒枚数と、移動側と次の手番も保持する。
利用側は探索スタックや評価器の更新に必要な値を選んで利用できる。
全フィールドと契約は Rust API の `MoveApplyFacts` を参照する。

現在の戦術キャッシュは `Position::current_state_cache()` が返す `StateCacheView` から取得する。
この view は `checkers()`、`check_squares()`、`check_square(piece_type)`、`blockers_for_king(color)`、`pinners(color)` を提供する。
ハッシュ値、直前手、捕獲駒、連続王手カウントはこの view に含まれず、必要に応じて `Position` の個別 accessor を使う。
view は局面への共有借用を保持するため、局面を更新する前に借用を終え、更新後は取得し直す。

## 2. 探索前のキー予測と prefetch の責務

候補手を適用する前のキー予測、TT の事前照会、prefetch は利用側が実装する。
探索用局面の盤面表現や、次に照会するテーブルの管理は、その利用側が所有する。
本体は `Zobrist::psq`、`Zobrist::side`、`Zobrist::hand_delta` などの基本操作を提供する。

1.x では既存のキー予測 API を非推奨の互換実装として維持する。
2.0.0 の削除対象と移行方法は [移行ガイド](../../getting-started/migration-1.6.md) を参照する。

通常の `Position` で着手後のキーが必要な場合は、実際に手を適用してから `key()` や `board_key()` を読む。
`try_apply_move32_with_facts()` が返す `MoveApplyFacts` にも、適用後のキーと盤面差分が含まれる。
この更新結果は利用側が再利用できるが、着手前の独立した予測 API ではない。

```rust
let facts = pos.try_apply_move32_with_facts(mv)?;
assert_eq!(facts.key_after, pos.key());
assert_eq!(facts.board_key_after, pos.board_key());
```

## 3. 千日手判定

局面更新時には `board_key` と履歴中の手番側の持ち駒を比較し、反復回数、距離、種類をキャッシュする。
同一局面に戻るには少なくとも 4 手を要するため、4 手前から 2 手ずつ遡る。
null move より前や保持範囲外の履歴は対象にしない。
根履歴に一致候補がないことを索引で確認できた場合は、根以前の走査を省く。

盤面キーと持ち駒が一致する場合は、反復回数を更新し、連続王手の継続手数と反復距離から `Lose`、`Win`、`Draw` を判定する。
盤面キーが一致して持ち駒が異なる場合は、`Hand::is_equal_or_superior` による駒種ごとの比較で `Superior` または `Inferior` を判定する。

呼び出し側は、規則に基づく判定と探索経路内の反復を区別して API を選ぶ。

| API | 判定範囲 |
|---|---|
| `is_repetition(3)` | 現局面が 3 回以上繰り返され、同一局面の 4 回目になったか。 |
| `repetition_state()` | 4 回目以降の反復の種類、または持ち駒の優劣。 |
| `repetition_state_with_ply(ply)` | 指定範囲の履歴を走査する。規則上成立済みの 4 回目は `ply` に関わらず返す。 |
| `repetition_state_with_distance(depth)` | 指定範囲だけを走査し、反復の種類と距離を返す。4 回目でも範囲外の一致は返さない。 |
| `cached_repetition_state_with_ply(ply)` | 履歴を再走査せず、現在の state にある反復距離と種類を使う。 |

## 4. 独立した探索用コピー

`Position::clone()` は全履歴と予約容量を複製し、元局面と独立した局面を作る。
同じ保存先へ繰り返し複製する場合は、`saved.clone_from(&pos)` で保存先の履歴領域を再利用できる。
`clone_from()` も全履歴を複製し、複製元の予約容量を維持する。
保存先に余る領域は保持するため、履歴が短い局面を複製した後も再利用できる。

`clone_for_search()` は null move と保持上限に応じた履歴を詰め直し、不要な予約領域を引き継がない。
内部の履歴ストレージを理解したコピー操作として、両 API を提供する。

`clone_for_search()` は根履歴の board key 索引を作る。
一致候補がない場合だけ根以前の走査を省き、一致した場合は持ち駒と連続王手を含む詳細判定を行う。
索引だけで千日手を判定することはない。
索引構築には確保を伴い、一致候補が多い履歴では照会の費用が加わる。
コピーの準備費用と、その後の走査の削減効果を分けて評価する。

履歴上限を変更する `set_max_repetition_ply()` は 1.6.0 から非推奨で、2.0.0 で削除予定である。
既存の動作と既定値 `Ply::MAX` は維持する。
上限を短くする近似方針は利用側で管理する。

軽量な探索専用局面と評価器に固有の状態は、必要な性能と探索方式に合わせて利用側で実装する。
