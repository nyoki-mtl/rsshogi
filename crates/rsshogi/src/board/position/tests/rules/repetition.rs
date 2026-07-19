use super::helpers::{apply_usi_move, move_from_usi};
use super::*;
use crate::types::{Color, HandPiece, PieceType, RepetitionState};

#[test]
// StateStack初期化時にrepetition_counterとplies_from_nullが0で始まることを確認
fn test_repetition_state_initializes_to_zero() {
    let pos = crate::board::hirate_position();

    assert_eq!(pos.state_stack().current().repetition_counter, 0);
    assert_eq!(pos.state_stack().current().plies_from_null, 0);
}

#[test]
// 通常手のみを進めた場合にplies_from_nullが手数分だけ増えるか検証
fn test_plies_from_null_increments_without_events() {
    let mut pos = crate::board::hirate_position();

    apply_usi_move(&mut pos, "2g2f");
    assert_eq!(pos.state_stack().current().plies_from_null, 1);

    apply_usi_move(&mut pos, "8c8d");
    assert_eq!(pos.state_stack().current().plies_from_null, 2);
}

#[test]
// 駒取り発生時にもplies_from_nullがリセットされないことを検証
fn test_plies_from_null_does_not_reset_on_capture() {
    let mut pos = crate::board::hirate_position();

    // ▲７六歩△８四歩▲８六歩△８五歩
    for usi in ["7g7f", "8c8d", "8g8f", "8d8e"] {
        apply_usi_move(&mut pos, usi);
    }

    assert_eq!(pos.state_stack().current().plies_from_null, 4);

    // ▲８五歩×同歩 （駒取りでも継続）
    apply_usi_move(&mut pos, "8f8e");

    assert_eq!(pos.state_stack().current().plies_from_null, 5);
    assert_ne!(pos.state_stack().current().cold.captured_piece(), Piece::NONE);
}

#[test]
// 駒打ち発生時にもplies_from_nullがリセットされないことを検証
fn test_plies_from_null_does_not_reset_on_drop() {
    let mut pos = crate::board::hirate_position();

    // ▲７六歩△３四歩▲２二角成△同銀
    for usi in ["7g7f", "3c3d", "8h2b+", "3a2b"] {
        apply_usi_move(&mut pos, usi);
    }

    assert_eq!(pos.state_stack().current().plies_from_null, 4);
    assert_eq!(pos.turn(), Color::BLACK);

    // 持ち駒に角が1枚あることを確認
    let hand_piece = HandPiece::from_piece_type(PieceType::BISHOP).unwrap();
    let count_of = pos.hand(pos.turn()).count(hand_piece);
    assert_eq!(count_of, 1);

    // ▲４五角（角を打った）
    let drop_mv = move_from_usi(&pos, "B*4e");
    pos.apply_move32(drop_mv);

    assert_eq!(pos.state_stack().current().plies_from_null, 5);
}

#[test]
// 同一局面に戻った際にrepetition_counterがインクリメントされることを確認
fn test_repetition_counter_increments_when_position_repeats() {
    let mut pos = crate::board::hirate_position();

    // ▲３八飛△７二飛▲２八飛△８二飛
    let sequences = ["2h3h", "8b7b", "3h2h", "7b8b"];
    let initial_zobrist = pos.key();

    for usi in sequences {
        apply_usi_move(&mut pos, usi);
    }

    assert_eq!(pos.key(), initial_zobrist);
    assert_eq!(pos.state_stack().current().repetition_counter, 1);

    for usi in sequences {
        apply_usi_move(&mut pos, usi);
    }

    assert_eq!(pos.key(), initial_zobrist);
    assert_eq!(pos.state_stack().current().repetition_counter, 2);
}

#[test]
// repetition_counterを用いた三回（三度目）繰り返し判定を検証
fn test_is_repetition_detects_threefold() {
    let mut pos = crate::board::hirate_position();

    assert!(!pos.is_repetition(1));
    assert!(!pos.is_repetition(3));

    // ▲３八飛△７二飛▲２八飛△８二飛
    let sequences = ["2h3h", "8b7b", "3h2h", "7b8b"];

    for repeat in 0..3 {
        for usi in sequences {
            apply_usi_move(&mut pos, usi);
        }

        match repeat {
            0 => {
                assert_eq!(pos.state_stack().current().repetition_counter, 1);
                assert!(!pos.is_repetition(3));
            }
            1 => {
                assert_eq!(pos.state_stack().current().repetition_counter, 2);
                assert!(!pos.is_repetition(3));
            }
            _ => {}
        }
    }

    assert!(pos.is_repetition(3));
}

#[test]
fn test_clone_for_search_truncates_history_and_preserves_current_state() {
    let mut pos = crate::board::hirate_position();

    let moves = [
        "7g7f", "3c3d", "2g2f", "4c4d", "3i4h", "3a4b", "5g5f", "5c5d", "4h5g", "4b5c", "5g4f",
        "5c4b", "4f5g", "4b5c", "5g4f", "5c4b", "4f5g", "4b5c", "5g4f", "5c4b",
    ];
    for usi in moves {
        apply_usi_move(&mut pos, usi);
    }

    let cloned = pos.clone_for_search();

    assert_eq!(cloned.to_sfen(None), pos.to_sfen(None));
    assert_eq!(cloned.key(), pos.key());
    assert_eq!(cloned.board_key(), pos.board_key());
    assert_eq!(cloned.hand_key(), pos.hand_key());
    assert_eq!(cloned.checkers(), pos.checkers());
    assert_eq!(cloned.last_move(), pos.last_move());
    assert_eq!(cloned.last_moved_piece_type(), pos.last_moved_piece_type());
    assert_eq!(cloned.repetition_counter(), pos.repetition_counter());
    assert_eq!(cloned.repetition_distance(), pos.repetition_distance());
    assert_eq!(cloned.state_stack_depth(), 16);
    assert!(cloned.state_stack_depth() < pos.state_stack_depth());
}

#[test]
fn test_clone_for_search_keeps_enough_history_for_repetition() {
    let mut pos = crate::board::hirate_position();

    let loop_moves = ["2h3h", "8b7b", "3h2h", "7b8b", "2h3h", "8b7b", "3h2h", "7b8b"];
    for usi in loop_moves {
        apply_usi_move(&mut pos, usi);
    }

    let mut cloned = pos.clone_for_search();

    for usi in ["2h3h", "8b7b", "3h2h", "7b8b"] {
        apply_usi_move(&mut cloned, usi);
    }

    assert_eq!(cloned.repetition_state(), RepetitionState::Draw);
}

#[test]
// 連続王手カウンタが王手をかけた際に増加することを確認
fn test_continuous_check_counter_increments_on_check() {
    // 連続王手の千日手局面: 先手が連続で王手をかけ続ける
    let sfen = "lr2+R2G1/6g1k/p3ppppp/2p1b4/9/2P+b4P/P+p3PPP1/4G1SK1/LN3+p1NL b 2SNLgsn3p 1";
    let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");

    // 初期状態では連続王手カウンタは0
    assert_eq!(pos.state_stack().current().continuous_check[Color::BLACK.to_index()], 0u16);
    assert_eq!(pos.state_stack().current().continuous_check[Color::WHITE.to_index()], 0u16);

    // 先手が王手をかける手を指す: 2a1a
    apply_usi_move(&mut pos, "2a1a");
    // 先手が王手をかけたので、先手の連続王手カウンタが2に増加
    assert_eq!(pos.state_stack().current().continuous_check[Color::BLACK.to_index()], 2u16);
    assert_eq!(pos.state_stack().current().continuous_check[Color::WHITE.to_index()], 0u16);

    // 後手が王手を回避: 1b2b
    apply_usi_move(&mut pos, "1b2b");
    // 後手は王手をかけていないので、後手のカウンタは0のまま
    // 先手のカウンタは継続（リセットされない）
    assert_eq!(pos.state_stack().current().continuous_check[Color::BLACK.to_index()], 2u16);
    assert_eq!(pos.state_stack().current().continuous_check[Color::WHITE.to_index()], 0u16);

    // 先手が再び王手: 1a2a
    apply_usi_move(&mut pos, "1a2a");
    // 先手の連続王手カウンタが4に増加
    assert_eq!(pos.state_stack().current().continuous_check[Color::BLACK.to_index()], 4u16);
    assert_eq!(pos.state_stack().current().continuous_check[Color::WHITE.to_index()], 0u16);

    // 後手が王手を回避: 2b1b
    apply_usi_move(&mut pos, "2b1b");
    // 先手のカウンタは継続
    assert_eq!(pos.state_stack().current().continuous_check[Color::BLACK.to_index()], 4u16);
    assert_eq!(pos.state_stack().current().continuous_check[Color::WHITE.to_index()], 0u16);
}

#[test]
// 連続王手カウンタが王手でない手で0のままであることを確認
fn test_continuous_check_counter_stays_zero_on_non_check() {
    let mut pos = crate::board::hirate_position();

    // 初期状態では連続王手カウンタは0
    assert_eq!(pos.state_stack().current().continuous_check[Color::BLACK.to_index()], 0u16);
    assert_eq!(pos.state_stack().current().continuous_check[Color::WHITE.to_index()], 0u16);

    // 王手でない手を指す
    apply_usi_move(&mut pos, "7g7f");

    // 王手でない手を指したので、先手のカウンタは0のまま
    assert_eq!(pos.state_stack().current().continuous_check[Color::BLACK.to_index()], 0u16);
    assert_eq!(pos.state_stack().current().continuous_check[Color::WHITE.to_index()], 0u16);

    // 後手も王手でない手を指す
    apply_usi_move(&mut pos, "8c8d");

    // 王手でない手なので、両者とも0のまま
    assert_eq!(pos.state_stack().current().continuous_check[Color::BLACK.to_index()], 0u16);
    assert_eq!(pos.state_stack().current().continuous_check[Color::WHITE.to_index()], 0u16);
}

#[test]
// get_repetition_state()が通常の千日手を検出することを確認
fn test_get_repetition_state_detects_normal_repetition() {
    let mut pos = crate::board::hirate_position();

    // 初期状態では千日手でない
    assert_eq!(pos.repetition_state(), RepetitionState::None);

    // ▲３八飛△７二飛▲２八飛△８二飛を3回繰り返す
    let sequences = ["2h3h", "8b7b", "3h2h", "7b8b"];

    // 1回目
    for usi in sequences {
        apply_usi_move(&mut pos, usi);
    }
    assert_eq!(pos.repetition_state(), RepetitionState::None);

    // 2回目
    for usi in sequences {
        apply_usi_move(&mut pos, usi);
    }
    assert_eq!(pos.repetition_state(), RepetitionState::None);

    // 3回目（4回目の同一局面出現）
    for usi in sequences {
        apply_usi_move(&mut pos, usi);
    }
    assert_eq!(pos.repetition_state(), RepetitionState::Draw);
}

#[test]
fn test_cached_repetition_state_with_ply_uses_signed_cached_distance() {
    let mut pos = crate::board::hirate_position();

    for usi in ["2h3h", "8b7b", "3h2h", "7b8b"] {
        apply_usi_move(&mut pos, usi);
    }

    assert_eq!(pos.repetition_counter(), 1);
    assert_eq!(pos.repetition_distance(), 4);
    assert_eq!(pos.current_state().repetition_type, RepetitionState::Draw);

    // cached API は `repetition_distance < ply` の
    // signed 比較を使うので、distance == ply では検出しない。
    assert_eq!(pos.cached_repetition_state_with_ply(4), RepetitionState::None);
    assert_eq!(pos.cached_repetition_state_with_ply(5), RepetitionState::Draw);

    // full scan API は state stack を遡って調べるため、同じ ply でも検出しうる。
    assert_eq!(pos.repetition_state_with_ply(4), RepetitionState::Draw);
}

#[test]
fn test_cached_repetition_state_with_ply_returns_fourfold_even_at_ply_zero() {
    let mut pos = crate::board::hirate_position();

    for _ in 0..3 {
        for usi in ["2h3h", "8b7b", "3h2h", "7b8b"] {
            apply_usi_move(&mut pos, usi);
        }
    }

    assert_eq!(pos.repetition_counter(), 3);
    assert_eq!(pos.repetition_distance(), -4);

    // 4回目以降は cached repetition_distance が負になるため、
    // signed 比較では ply=0 でも即座に返る。
    // 検出距離 (=4) は上の repetition_distance() == -4 で被覆済み。
    assert_eq!(pos.cached_repetition_state_with_ply(0), RepetitionState::Draw);
}

#[test]
// get_repetition_state()が連続王手の千日手を検出することを確認（先手が王手をかけ続ける）
fn test_get_repetition_state_detects_perpetual_check_by_black() {
    // 連続王手の千日手局面: 先手が連続で王手をかけ続ける
    let sfen = "lr2+R2G1/6g1k/p3ppppp/2p1b4/9/2P+b4P/P+p3PPP1/4G1SK1/LN3+p1NL b 2SNLgsn3p 1";
    let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");

    // 初期状態では千日手でない
    assert_eq!(pos.repetition_state(), RepetitionState::None);

    // 王手の手順: 2a1a 1b2b 1a2a 2b1b を3回繰り返す
    let sequences = ["2a1a", "1b2b", "1a2a", "2b1b"];

    // 1回目
    for usi in sequences {
        apply_usi_move(&mut pos, usi);
    }
    assert_eq!(pos.repetition_state(), RepetitionState::None);

    // 2回目
    for usi in sequences {
        apply_usi_move(&mut pos, usi);
    }
    assert_eq!(pos.repetition_state(), RepetitionState::None);

    // 3回目（4回目の同一局面出現）
    for usi in sequences {
        apply_usi_move(&mut pos, usi);
    }

    // 先手が連続王手をかけ続けたので、RepetitionState::Lose（先手の負け）
    assert_eq!(pos.repetition_state(), RepetitionState::Lose);
}

/// 千日手カウンタと閾値判定を検証する。
#[test]
fn test_repetition_counter_thresholds() {
    let mut pos = crate::board::hirate_position();

    // 1. repetition_counterは同一局面の登場回数をカウントする
    let sequences = ["2h3h", "8b7b", "3h2h", "7b8b"];
    let initial_zobrist = pos.key();

    // 1回目の繰り返し
    for usi in sequences {
        apply_usi_move(&mut pos, usi);
    }
    assert_eq!(pos.key(), initial_zobrist);
    assert_eq!(pos.state_stack().current().repetition_counter, 1);

    // 2回目の繰り返し
    for usi in sequences {
        apply_usi_move(&mut pos, usi);
    }
    assert_eq!(pos.key(), initial_zobrist);
    assert_eq!(pos.state_stack().current().repetition_counter, 2);

    // 3回目の繰り返し（4回目の同一局面出現）
    for usi in sequences {
        apply_usi_move(&mut pos, usi);
    }
    assert_eq!(pos.key(), initial_zobrist);
    assert_eq!(pos.state_stack().current().repetition_counter, 3);

    // 2. get_repetition_stateがrepetition_counter >= 3で千日手を検出
    assert_eq!(pos.repetition_state(), RepetitionState::Draw);

    // 3. is_repetition(threshold)がrepetition_counter >= thresholdで判定
    assert!(!pos.is_repetition(4)); // 4回目未満
    assert!(pos.is_repetition(3)); // 3回目以上
    assert!(pos.is_repetition(2)); // 2回目以上
    assert!(pos.is_repetition(1)); // 1回目以上
}

/// 連続王手の千日手を検出する。
#[test]
fn test_continuous_check_repetition() {
    // 連続王手の千日手局面: 先手が連続で王手をかけ続ける
    let sfen = "lr2+R2G1/6g1k/p3ppppp/2p1b4/9/2P+b4P/P+p3PPP1/4G1SK1/LN3+p1NL b 2SNLgsn3p 1";
    let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");

    let sequences = ["2a1a", "1b2b", "1a2a", "2b1b"];

    // 3回繰り返して4回目の同一局面を出現させる
    for _ in 0..3 {
        for usi in sequences {
            apply_usi_move(&mut pos, usi);
        }
    }

    // 先手が連続王手をかけ続けたので、RepetitionState::Lose（先手の負け）
    assert_eq!(pos.repetition_state(), RepetitionState::Lose);

    // continuous_checkカウンタが立っていることを確認
    assert!(pos.state_stack().current().continuous_check[Color::BLACK.to_index()] > 0);
}

/// 優等局面と劣等局面の状態値を検証する。
#[test]
fn test_superior_inferior_state_variants() {
    // 優等/劣等局面のテストは複雑なため、簡易的な検証のみ実施
    // 実装の存在確認
    let pos = crate::board::hirate_position();

    // RepetitionState::SuperiorとInferiorが定義されていることを確認
    let _ = (RepetitionState::Superior, RepetitionState::Inferior);

    // 通常局面では千日手でない
    assert_eq!(pos.repetition_state(), RepetitionState::None);
}

/// `plies_from_null` が通常手と駒取りで増加することを検証する。
#[test]
fn test_plies_from_null_progression() {
    let mut pos = crate::board::hirate_position();

    // 通常手でplies_from_nullが増加
    apply_usi_move(&mut pos, "7g7f");
    assert_eq!(pos.state_stack().current().plies_from_null, 1);

    apply_usi_move(&mut pos, "8c8d");
    assert_eq!(pos.state_stack().current().plies_from_null, 2);

    // 駒取りでもplies_from_nullは継続
    apply_usi_move(&mut pos, "8g8f");
    apply_usi_move(&mut pos, "8d8e");
    apply_usi_move(&mut pos, "8f8e"); // 駒取り
    assert_eq!(pos.state_stack().current().plies_from_null, 5);
}

#[test]
// 優等/劣等局面の検出が指定局面で成立することを確認
fn test_repetition_state_detects_superior_inferior() {
    let sfen = "lr6l/4g1pkp/2ns1s3/4pp1SP/PPBP2Pp1/2gpPP3/2B2S3/2K6/L6RL b 4P2g3np 1";
    let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");

    for usi in ["P*2c", "2b2a", "2c2b+", "2a2b"] {
        apply_usi_move(&mut pos, usi);
    }

    assert_eq!(pos.turn(), Color::BLACK);
    assert_eq!(pos.repetition_state(), RepetitionState::Inferior);
}

#[test]
// 連続王手が途切れる反復では通常の千日手になることを確認
fn test_repetition_state_handles_broken_perpetual_check() {
    let sfen = "kns5l/lsp2+R3/1p2B3+P/p1+S+B4p/9/3pP3P/PPG1+pP3/L7s/KGg+r5 b G3NL4P2p 1";
    let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");

    let sequences = ["G*8h", "7i8i", "8h8i", "G*7i"];

    for _ in 0..3 {
        for usi in sequences {
            apply_usi_move(&mut pos, usi);
        }
    }

    assert_eq!(pos.repetition_state(), RepetitionState::Draw);
}
