use super::*;
use crate::board::test_support::move_from_usi_expect;
use crate::board::zobrist::Zobrist;
use crate::types::{Color, Hand, MOVE_NONE, Piece, PieceType};

#[allow(clippy::cognitive_complexity)]
fn assert_state_info_matches(actual: &StateInfo, expected: &StateInfo) {
    assert_eq!(actual.board_key, expected.board_key);
    assert_eq!(actual.hand_key, expected.hand_key);
    assert_eq!(actual.pawn_key, expected.pawn_key);
    assert_eq!(actual.minor_piece_key, expected.minor_piece_key);
    assert_eq!(actual.non_pawn_key, expected.non_pawn_key);
    assert_eq!(actual.material_key, expected.material_key);
    assert_eq!(actual.material_value, expected.material_value);
    assert_eq!(actual.tactical, expected.tactical);
    assert_eq!(actual.continuous_check, expected.continuous_check);
    assert_eq!(actual.repetition_counter, expected.repetition_counter);
    assert_eq!(actual.repetition_distance, expected.repetition_distance);
    assert_eq!(actual.repetition_times, expected.repetition_times);
    assert_eq!(actual.plies_from_null, expected.plies_from_null);
    assert_eq!(actual.hand, expected.hand);
    assert_eq!(actual.repetition_type, expected.repetition_type);
    assert_eq!(actual.last_move(), expected.last_move());
    assert_eq!(actual.last_moved_piece_type(), expected.last_moved_piece_type());
}

#[test]
#[allow(clippy::cognitive_complexity)]
fn test_state_info_default() {
    let info = StateInfo::default();
    assert_eq!(info.cold.captured_piece(), Piece::NONE);
    assert_eq!(info.board_key, ZobristKey::default());
    assert_eq!(info.hand_key, ZobristKey::default());
    assert_eq!(info.pawn_key, Zobrist::no_pawns());
    assert_eq!(info.minor_piece_key, ZobristKey::default());
    assert_eq!(info.non_pawn_key, [ZobristKey::default(); Color::COUNT]);
    assert_eq!(info.material_key, ZobristKey::default());
    assert_eq!(info.material_value, 0);
    assert_eq!(info.repetition_counter, 0);
    assert_eq!(info.repetition_distance, 0);
    assert_eq!(info.repetition_times, 0);
    assert_eq!(info.plies_from_null, 0);
    assert_eq!(info.hot.tactical.checkers(), Bitboard::EMPTY);
    assert_eq!(info.hot.tactical.pinners(Color::BLACK), Bitboard::EMPTY);
    assert_eq!(info.hot.tactical.pinners(Color::WHITE), Bitboard::EMPTY);
    assert_eq!(info.blockers_for_king(Color::BLACK), Bitboard::EMPTY);
    assert_eq!(info.blockers_for_king(Color::WHITE), Bitboard::EMPTY);
    assert_eq!(info.hot.tactical.check_squares, CheckSquares::EMPTY);
    // 連続王手カウンタのデフォルト値確認
    assert_eq!(info.continuous_check, [0u16, 0u16]);
    assert_eq!(info.hand, Hand::ZERO);
    // 千日手状態のデフォルト値確認
    assert_eq!(info.repetition_type, RepetitionState::None);
    assert_eq!(info.last_move(), MOVE_NONE);
    assert_eq!(info.last_moved_piece_type(), PieceType::NONE);
}

#[test]
fn test_state_info_check_squares_layout() {
    assert_eq!(core::mem::size_of::<Bitboard>(), 16);
    assert_eq!(core::mem::size_of::<CheckSquares>(), 144);
    assert!(core::mem::size_of::<StateInfo>() <= 384);
}

#[test]
fn test_check_squares_piece_type_mapping() {
    let mut check_squares = CheckSquares::EMPTY;
    let values = [
        Bitboard::from_square(crate::types::SQ_11),
        Bitboard::from_square(crate::types::SQ_12),
        Bitboard::from_square(crate::types::SQ_13),
        Bitboard::from_square(crate::types::SQ_14),
        Bitboard::from_square(crate::types::SQ_15),
        Bitboard::from_square(crate::types::SQ_16),
        Bitboard::from_square(crate::types::SQ_17),
        Bitboard::from_square(crate::types::SQ_18),
        Bitboard::from_square(crate::types::SQ_19),
    ];

    for (idx, value) in values.into_iter().enumerate() {
        check_squares.set(idx, value);
    }

    assert_eq!(check_squares.get(PieceType::NONE), Bitboard::EMPTY);
    assert_eq!(check_squares.get(PieceType::PAWN), values[CHECK_SQ_PAWN]);
    assert_eq!(check_squares.get(PieceType::LANCE), values[CHECK_SQ_LANCE]);
    assert_eq!(check_squares.get(PieceType::KNIGHT), values[CHECK_SQ_KNIGHT]);
    assert_eq!(check_squares.get(PieceType::SILVER), values[CHECK_SQ_SILVER]);
    assert_eq!(check_squares.get(PieceType::BISHOP), values[CHECK_SQ_BISHOP]);
    assert_eq!(check_squares.get(PieceType::ROOK), values[CHECK_SQ_ROOK]);
    assert_eq!(check_squares.get(PieceType::GOLD), values[CHECK_SQ_GOLD]);
    assert_eq!(check_squares.get(PieceType::KING), Bitboard::EMPTY);
    assert_eq!(check_squares.get(PieceType::PRO_PAWN), values[CHECK_SQ_GOLD]);
    assert_eq!(check_squares.get(PieceType::PRO_LANCE), values[CHECK_SQ_GOLD]);
    assert_eq!(check_squares.get(PieceType::PRO_KNIGHT), values[CHECK_SQ_GOLD]);
    assert_eq!(check_squares.get(PieceType::PRO_SILVER), values[CHECK_SQ_GOLD]);
    assert_eq!(check_squares.get(PieceType::HORSE), values[CHECK_SQ_HORSE]);
    assert_eq!(check_squares.get(PieceType::DRAGON), values[CHECK_SQ_DRAGON]);
    assert_eq!(check_squares.get(PieceType::GOLD_LIKE), Bitboard::EMPTY);
}

#[test]
fn test_state_stack_new() {
    let stack = StateStack::new();
    assert_eq!(stack.depth(), 0);
}

#[test]
fn test_state_stack_push_pop() {
    let mut stack = StateStack::new();

    // 初期状態
    assert_eq!(stack.depth(), 0);

    // プッシュ
    let idx1 = stack.push_empty();
    assert_eq!(idx1, 1);
    assert_eq!(stack.depth(), 1);

    // さらにプッシュ
    let idx2 = stack.push_empty();
    assert_eq!(idx2, 2);
    assert_eq!(stack.depth(), 2);

    // ポップ
    let popped = stack.pop();
    assert_eq!(popped, Some(2));
    assert_eq!(stack.depth(), 1);

    // もう一度ポップ
    let popped = stack.pop();
    assert_eq!(popped, Some(1));
    assert_eq!(stack.depth(), 0);

    // 空の状態でポップ
    let popped = stack.pop();
    assert_eq!(popped, None);
}

#[test]
fn test_state_stack_reset_sfen() {
    let mut stack = StateStack::new();

    // いくつかプッシュ
    stack.push_empty();
    stack.push_empty();
    stack.push_empty();
    assert_eq!(stack.depth(), 3);

    // リセット
    stack.reset_sfen();
    assert_eq!(stack.depth(), 0);
}

#[test]
fn test_state_stack_reset_with_position_matches_init_stack() {
    let mut stack_from_reset = StateStack::new();
    let mut pos = crate::board::hirate_position();

    pos.init_stack();
    stack_from_reset.reset_with_position(&mut pos);

    let init_state = pos.state_stack().current().clone();
    let reset_state = stack_from_reset.current();
    assert_state_info_matches(reset_state, &init_state);
}

#[test]
fn test_state_stack_apply_and_undo_restore_state() {
    let mut pos = crate::board::hirate_position();
    pos.init_stack();

    let initial = pos.state_stack().current().clone();
    let mv = move_from_usi_expect(&pos, "7g7f");
    pos.apply_move32(mv);
    pos.undo_move32(mv).expect("undo move");

    let stack = pos.state_stack();
    let restored = stack.current();
    assert_state_info_matches(restored, &initial);
}

#[test]
fn test_state_info_records_captured_piece() {
    let sfen = "ln1g3nl/1r3kg2/p2pppsp1/3s2p1p/1pp4P1/P1P1SP2P/1PSPP1P2/2GK3R1/LN3G1NL b Bb 1";
    let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    pos.init_stack();

    let mv = move_from_usi_expect(&pos, "7f7e");
    pos.apply_move32(mv);

    let captured = pos.state_stack().current().cold.captured_piece();
    assert_ne!(captured, Piece::NONE);
    assert_eq!(captured.color(), Color::WHITE);
}

#[test]
fn test_state_stack_get_mut() {
    let mut stack = StateStack::new();

    // プッシュして値を設定
    let idx = stack.push_empty();
    {
        let entry = stack.get_mut(idx);
        entry.repetition_counter = 3;
        entry.plies_from_null = 10;
    }

    // 取得して確認
    let entry = &stack[idx];
    assert_eq!(entry.repetition_counter, 3);
    assert_eq!(entry.plies_from_null, 10);
}

#[test]
fn test_state_stack_reuse_memory() {
    let mut stack = StateStack::new();

    // 深くプッシュ
    for _ in 0..10 {
        stack.push_empty();
    }

    // 特定の値を設定
    stack.current_mut().repetition_counter = 42;

    // リセット
    stack.reset_sfen();

    // 再度プッシュ（メモリは再利用される）
    for _ in 0..10 {
        stack.push_empty();
    }

    // 前の値が残っている可能性があるが、それは仕様
    // （reset時に全クリアしないため）
    // 重要なのは深さ管理が正しいこと
    assert_eq!(stack.depth(), 10);
}

#[test]
fn test_state_stack_can_grow() {
    let mut stack = StateStack::new();

    for _ in 0..512 {
        stack.push_empty();
    }

    assert_eq!(stack.depth(), 512);
}

#[test]
fn test_prepare_additional_covers_chunk_boundaries_from_root_and_nonzero_head() {
    for additional in [0, 1, 63, 64, 65] {
        let mut stack = StateStack::new();
        assert!(stack.prepare_additional(additional));
        let chunks = stack.chunk_count();
        for _ in 0..additional {
            assert!(stack.has_prepared_next());
            stack.push_empty();
            assert_eq!(stack.chunk_count(), chunks);
        }
    }

    let mut stack = StateStack::new();
    for _ in 0..17 {
        stack.push_empty();
    }
    assert!(stack.prepare_additional(65));
    let chunks = stack.chunk_count();
    for _ in 0..65 {
        assert!(stack.has_prepared_next());
        stack.push_empty();
        assert_eq!(stack.chunk_count(), chunks);
    }
}

#[test]
fn test_prepare_additional_covers_246_pushes_without_chunk_growth() {
    let mut stack = StateStack::new();
    assert!(stack.prepare_additional(246));
    let chunks = stack.chunk_count();

    for _ in 0..246 {
        assert!(stack.has_prepared_next());
        stack.push_empty();
        assert_eq!(stack.chunk_count(), chunks);
    }
}

#[test]
fn test_prepare_additional_overflow_fails_without_growing() {
    let mut stack = StateStack::new();
    stack.push_empty();
    let chunks = stack.chunk_count();
    let head = stack.current_index();
    let current = std::ptr::from_ref(&stack[head]);
    assert!(!stack.prepare_additional(usize::MAX));
    assert_eq!(stack.current_index(), head);
    assert_eq!(stack.chunk_count(), chunks);
    assert_eq!(std::ptr::from_ref(&stack[head]), current);
}

#[test]
fn test_prepare_additional_keeps_live_entry_address_stable() {
    let mut stack = StateStack::new();
    let before = std::ptr::from_ref(&stack[stack.current_index()]);
    assert!(stack.prepare_additional(STATE_CHUNK_CAPACITY * 2));
    let after = std::ptr::from_ref(&stack[stack.current_index()]);
    assert_eq!(before, after);
}

#[test]
fn test_reset_sfen_discards_prepared_extra_chunks() {
    let mut stack = StateStack::new();
    assert!(stack.prepare_additional(STATE_CHUNK_CAPACITY * 2));
    assert!(stack.chunk_count() > 1);

    stack.reset_sfen();

    assert_eq!(stack.chunk_count(), 1);
    assert_eq!(stack.current_index(), 0);
    assert_eq!(stack.prepared_additional_capacity(), STATE_CHUNK_CAPACITY - 1);
}

/// 既存 live entry の物理アドレスが、stack 伸長で chunk 境界をまたいでも維持されることを検証。
#[test]
fn test_state_stack_address_stability_across_chunk_grow() {
    let mut stack = StateStack::new();
    let idx = stack.push_empty();
    let before_state = std::ptr::from_ref(&stack[idx]);
    let before_check_squares = std::ptr::from_ref(&stack[idx].hot.tactical.check_squares);

    // STATE_CHUNK_CAPACITY を超えて push し、新しい chunk を作らせる
    for _ in 0..(super::STATE_CHUNK_CAPACITY * 2) {
        stack.push_empty();
    }

    let after_state = std::ptr::from_ref(&stack[idx]);
    let after_check_squares = std::ptr::from_ref(&stack[idx].hot.tactical.check_squares);
    assert_eq!(before_state, after_state, "StateInfo address must be stable across chunk grow");
    assert_eq!(
        before_check_squares, after_check_squares,
        "check_squares field address must be stable across chunk grow"
    );
}

/// `push_for_move` / `push_clone_from_prev` でも live entry のアドレスは維持される。
#[test]
fn test_state_stack_address_stability_across_push_variants() {
    let mut stack = StateStack::new();
    let idx = stack.push_empty();
    let before = std::ptr::from_ref(&stack[idx]);

    for _ in 0..super::STATE_CHUNK_CAPACITY {
        stack.push_for_move();
    }
    for _ in 0..super::STATE_CHUNK_CAPACITY {
        stack.push_clone_from_prev();
    }

    let after = std::ptr::from_ref(&stack[idx]);
    assert_eq!(before, after);
}

/// `clone()` 後は別インスタンスとなり、pointer identity は引き継がれない。
#[test]
fn test_state_stack_clone_does_not_preserve_pointer_identity() {
    let mut stack = StateStack::new();
    let idx = stack.push_empty();
    let original = std::ptr::from_ref(&stack[idx]);

    let cloned = stack.clone();
    let cloned_ptr = std::ptr::from_ref(&cloned[idx]);

    assert_ne!(original, cloned_ptr, "cloned StateStack must be a different allocation");
}

/// chunk 境界をまたぐ index 計算が正しく動作することを検証。
#[test]
fn test_state_stack_index_across_chunks() {
    let mut stack = StateStack::new();
    let total = super::STATE_CHUNK_CAPACITY * 3 + 5;

    for i in 0..total {
        let idx = stack.push_empty();
        // i は 0-indexed なので push 後の idx は i+1
        stack.get_mut(idx).plies_from_null = u16::try_from(i + 1).unwrap();
    }

    for i in 0..total {
        let idx = i + 1;
        assert_eq!(stack[idx].plies_from_null, u16::try_from(idx).unwrap());
    }
}

#[test]
fn test_continuous_check_counter() {
    let mut info = StateInfo::default();

    // 初期値は両者とも0
    assert_eq!(info.continuous_check[Color::BLACK.to_index()], 0u16);
    assert_eq!(info.continuous_check[Color::WHITE.to_index()], 0u16);

    // 連続王手カウンタを更新
    info.continuous_check[Color::BLACK.to_index()] = 2u16;
    assert_eq!(info.continuous_check[Color::BLACK.to_index()], 2u16);
    assert_eq!(info.continuous_check[Color::WHITE.to_index()], 0u16);

    // リセット
    info.reset_sfen();
    assert_eq!(info.continuous_check[Color::BLACK.to_index()], 0u16);
    assert_eq!(info.continuous_check[Color::WHITE.to_index()], 0u16);
}

#[test]
fn test_repetition_type_field() {
    let mut info = StateInfo::default();

    // 初期値はNone
    assert_eq!(info.repetition_type, RepetitionState::None);

    // 千日手状態を更新
    info.repetition_type = RepetitionState::Win;
    assert_eq!(info.repetition_type, RepetitionState::Win);

    info.repetition_type = RepetitionState::Draw;
    assert_eq!(info.repetition_type, RepetitionState::Draw);

    // リセット
    info.reset_sfen();
    assert_eq!(info.repetition_type, RepetitionState::None);
}
