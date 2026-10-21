# 1.6.0 の互換性と 2.0.0 への移行

1.6.0 は、公開済みの Rust API のシグネチャと動作を維持する。
下記の API は 1.6.0 から非推奨であり、2.0.0 で削除予定である。
非推奨警告が出ても 1.x では従来の処理を継続できる。
警告をエラーとして扱う環境では、移行するまで対象の呼び出しに限って `#[allow(deprecated)]` を指定する。

## 本体に残す操作

本体は将棋の基本型、盤面の照会、合法性判定、通常の着手と undo、適用結果、反復判定、棋譜と定跡の入出力を提供する。
利用側は探索局面、評価器、探索スタック、TT と prefetch、着手前の探索用情報を所有する。

`moved_piece_before(Move32)` と `moved_piece_before_move(Move)` は、通常の局面照会として両方維持する。
`MoveApplyFacts` とそのフィールド、`clone_for_search()`、反復判定 API も維持する。
二歩の照会 `has_pawn_on_file` と飛車の横利き `rook_rank_attacks` も、本体の primitive として維持する。
`prepare_search_state_capacity()` と `has_prepared_search_state()` は、通常の `Position` の容量確保と確認に利用できる。
これらを探索専用の着手 API と一括して廃止することはしない。

## 2.0.0 で削除する探索用 API

| 対象 | 移行方法 |
|---|---|
| `Position::key_after`、`key_after_move` | 着手前のキー予測は利用側の盤面表現で実装する。通常の着手後は `key()` または `MoveApplyFacts::key_after` を読む。 |
| `Position::board_key_after`、`board_key_after_move` | 着手前の盤面キー予測は利用側で実装する。通常の着手後は `board_key()` または `MoveApplyFacts::board_key_after` を読む。 |
| `Position::key_after_null` | 手番のみを反転する予測は `key() ^ Zobrist::side()` で表せる。 |
| `Position::move32_metadata`、`classify_move32`、`types::Move32Metadata` | 利用側の型に、必要な移動後の駒、捕獲駒、成り、駒打ち、移動元と移動先のインデックスをまとめる。 |
| `MoveApplyFacts::search_stack_site`、`board::MoveStackSiteFacts` | `MoveApplyFacts` の `moved_piece_after` と `to` から、利用側の探索スタック用の値を構築する。`board::position::MoveStackSiteFacts` も同じ廃止対象である。 |
| `Position::try_apply_search_move32_with_facts` | 予約領域だけを使う探索専用の着手は利用側へ移す。通常の局面更新には `try_apply_move32_with_facts` を使う。 |
| `Position::try_apply_search_null_move`、`undo_search_null_move` | 対局手数を進めない探索用 null move と履歴は利用側で管理する。通常の `apply_null_move` と `undo_null_move` は対局手数も更新するため、単純な置換はできない。 |
| `Position::set_max_repetition_ply` | 保持履歴を短くする探索上の近似方針は利用側で管理する。既定の全履歴保持と setter の既存動作は 1.x 中維持する。 |

`Move32Metadata` はデータ型であり、合法性判定や局面更新を行わない。
独自の型へ移行する際は、import、型注釈、比較テストも変更する。
本体の `Position` 版を期待値としているテストは、独立した盤面照会または固定した期待値で検査する。

`MoveStackSiteFacts` と `Move32Metadata` の公開パスは 1.x 中維持する。
型の reexport や内部フィールドを非公開化して、削除前に互換性を失わせることもしない。

## 2.0.0 で削除する補助 API

| 対象 | 移行方法 |
|---|---|
| `Position::is_capture_or_promotion` | `is_capture_move32(mv) || mv.is_promotion()`。 |
| `Position::is_pawn_promotion` | 成り手かを調べ、移動元の駒が歩かを確認する。 |
| `Position::is_capture_or_pawn_promotion` | 駒取り判定と歩の成り判定を組み合わせる。 |
| `Position::is_capture_or_valuable_promotion` | 歩、角、飛の成りを優先する分類方針を利用側で実装する。 |
| `Position::is_capture_stage` | 駒取りかを知りたい場合は `is_capture_move32` を使い、MovePicker の段階は利用側が決める。 |
| `Position::is_discovery_check_on_king` | 旧関数は `blockers_for_king(color).test(mv.to_move().from_sq())` に等しい。これは移動先まで含めた空き王手の確定判定ではない。 |
| `Position::pieces_by_types`、`pieces_by_types_array` | 駒種ごとの `pieces_by_type` の結果を論理和でまとめる。 |
| `Position::pieces_for_types`、`pieces_for_types_array` | 駒種ごとの `pieces_for` の結果を論理和でまとめる。 |
| `CheckSquares::to_full_array` | 必要な駒種を `CheckSquares::get` で参照する。 |
| `Hand::pawn_count`、`lance_count`、`knight_count`、`silver_count`、`bishop_count`、`rook_count`、`gold_count` | `Hand::count(HandPiece::...)` を使う。 |
| `Hand::sub_one` | `checked_sub(piece, 1)` を使う。枚数不足時に元の値を返す旧動作を維持するには `hand.checked_sub(piece, 1).unwrap_or(hand)` とする。 |
| `MoveGenType::is_generate_all_legal` | `MoveGenType::GENERATE_ALL_LEGAL` を参照する。 |
| `pawn_attacks_unchecked`、`knight_attacks_unchecked`、`silver_attacks_unchecked`、`gold_attacks_unchecked`、`king_attacks_unchecked` | 公開の `PAWN_ATTACKS`、`KNIGHT_ATTACKS`、`SILVER_ATTACKS`、`GOLD_ATTACKS`、`KING_ATTACKS` を添字で参照する。通常の添字アクセスは範囲を検査する。 |

## 更新時に確認すること

1.6.0 を使うために、非推奨 API をすべて置き換える必要はない。
ビルド時の非推奨警告を確認し、2.0.0 へ更新するまでに、利用している API を上表の方法で移行する。
探索用 null move の手数管理や履歴上限の変更は、通常の API への置換で動作が変わるため、移行後の反復判定も確認する。
Python の呼び出し方法に変更はない。
