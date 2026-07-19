//! `apply_move32` / `apply_null_move` の carry-over 責務に関する regression test。
//!
//! Phase 3 で `prepare_next_for_move` / `prepare_next_for_null_move` に責務を寄せたため、
//! ここでは「stack 側で carry-over される最小フィールド」が期待通り動くことを固定する。

use super::helpers::apply_usi_move;
use crate::board::position_from_sfen;
use crate::types::Color;

/// `apply_move32` 後、`plies_from_null` が prev + 1 で carry-over される。
#[test]
fn apply_move_carries_plies_from_null() {
    let mut pos = crate::board::hirate_position();
    let prev_plies = pos.current_state().plies_from_null;
    apply_usi_move(&mut pos, "7g7f");
    assert_eq!(pos.current_state().plies_from_null, prev_plies + 1);

    let prev_plies = pos.current_state().plies_from_null;
    apply_usi_move(&mut pos, "3c3d");
    assert_eq!(pos.current_state().plies_from_null, prev_plies + 1);
}

/// `apply_move32` 後、`continuous_check` の non-mover 側は前局面値が carry-over される。
/// （mover 側のみ gives_check に応じて加算 / 0 リセットされる。）
#[test]
fn apply_move_carries_continuous_check_for_non_mover() {
    let mut pos = crate::board::hirate_position();
    pos.state_stack_mut().current_mut().continuous_check = [3, 5];

    apply_usi_move(&mut pos, "7g7f");
    let new_check = pos.current_state().continuous_check;
    // 平手の 7g7f は王手ではない → mover (BLACK) は 0 にリセットされる
    assert_eq!(new_check[Color::BLACK.to_index()], 0);
    // non-mover (WHITE) は carry-over される
    assert_eq!(new_check[Color::WHITE.to_index()], 5);
}

/// `apply_null_move` 後、`plies_from_null` は 0 にリセットされる。
#[test]
fn apply_null_move_resets_plies_from_null() {
    let mut pos = crate::board::hirate_position();
    apply_usi_move(&mut pos, "7g7f");
    apply_usi_move(&mut pos, "3c3d");
    assert!(pos.current_state().plies_from_null > 0);

    pos.apply_null_move().expect("null move from non-check is legal");
    assert_eq!(pos.current_state().plies_from_null, 0);
}

/// `apply_null_move` 後、prev side の `continuous_check` は 0 にリセットされる。
///
/// 注意: `apply_null_move` は debug_assert で「次に指す側の continuous_check == 0」を
/// 要求するため、carry-over される反対側（新 turn 側）は実質常に 0 となる。
/// このテストでは prev_side のリセットのみを検証する。
#[test]
fn apply_null_move_resets_prev_side_continuous_check() {
    let mut pos = crate::board::hirate_position();
    // BLACK が WHITE に連続王手していたとする（assert 条件は満たす配置）
    pos.state_stack_mut().current_mut().continuous_check = [3, 0];
    let turn_before = pos.turn(); // BLACK

    pos.apply_null_move().expect("null move from non-check is legal");
    let after = pos.current_state().continuous_check;

    // prev_side (= turn_before, BLACK) は 0 にリセット
    assert_eq!(after[turn_before.to_index()], 0);
    // assert 条件で 0 が要求されている側もそのまま 0
    assert_eq!(after[turn_before.flip().to_index()], 0);
}

/// `apply_null_move` 後の `captured` / `checkers` / `repetition_*` は初期化される。
#[test]
fn apply_null_move_initializes_null_specific_fields() {
    let mut pos = crate::board::hirate_position();
    // 一手指して captured 以外のフィールドが 0 でない可能性を作る
    apply_usi_move(&mut pos, "7g7f");

    pos.apply_null_move().expect("null move from non-check is legal");
    let state = pos.current_state();

    assert_eq!(state.cold.captured_piece(), crate::types::Piece::NONE);
    assert!(state.hot.tactical.checkers().is_empty());
    assert_eq!(state.repetition_counter, 0);
    assert_eq!(state.repetition_distance, 0);
    assert_eq!(state.repetition_times, 0);
    assert_eq!(state.repetition_type, crate::types::RepetitionState::None);
}

/// SFEN 経由で読み込んだ局面でも `apply_move32` は plies_from_null を carry-over する。
#[test]
fn apply_move_after_sfen_load_carries_plies() {
    let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";
    let mut pos = position_from_sfen(sfen).expect("valid sfen");
    let prev_plies = pos.current_state().plies_from_null;

    apply_usi_move(&mut pos, "7g7f");
    assert_eq!(pos.current_state().plies_from_null, prev_plies + 1);
}
