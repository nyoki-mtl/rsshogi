use crate::board::state_info::{RepetitionInfo, StateStack};
use crate::board::zobrist::ZobristKey;
use crate::board::{Position, hirate_position};
use crate::types::{Hand, HandPiece, RepetitionState};

fn classify(distance: usize, checks: [u16; 2]) -> RepetitionState {
    if distance <= usize::from(checks[0]) {
        RepetitionState::Lose
    } else if distance <= usize::from(checks[1]) {
        RepetitionState::Win
    } else {
        RepetitionState::Draw
    }
}

// 反復キャッシュを使わず、全履歴と各一致区間の王手カウンタから求める。
fn reference(stack: &StateStack) -> RepetitionInfo {
    let current = stack.current_index();
    let state = stack.hot(current);
    let limit = usize::from(state.plies_from_null).min(current);
    let mut result = RepetitionInfo::default();
    let mut last_exact = current;
    for distance in (4..=limit).step_by(2) {
        let prior = stack.hot(current - distance);
        if prior.board_key != state.board_key {
            continue;
        }
        if prior.hand == state.hand {
            result.times += 1;
            let kind =
                classify(last_exact - (current - distance), stack.hot(last_exact).continuous_check);
            result.exact_type =
                if result.exact_type == RepetitionState::None || result.exact_type == kind {
                    kind
                } else {
                    RepetitionState::Draw
                };
            last_exact = current - distance;
        }
        if result.repetition_type == RepetitionState::None {
            let kind = if prior.hand == state.hand {
                // 後続で全区間の分類に置き換える。
                RepetitionState::Draw
            } else if Hand::is_equal_or_superior(state.hand, prior.hand) {
                RepetitionState::Superior
            } else if Hand::is_equal_or_superior(prior.hand, state.hand) {
                RepetitionState::Inferior
            } else {
                continue;
            };
            result.repetition_type = kind;
            result.distance = i32::try_from(distance).unwrap();
        }
    }
    if result.times >= 3 || result.repetition_type == RepetitionState::Draw {
        result.repetition_type = result.exact_type;
        let distance = (4..=limit)
            .step_by(2)
            .find(|&distance| {
                let prior = stack.hot(current - distance);
                prior.board_key == state.board_key && prior.hand == state.hand
            })
            .unwrap();
        result.distance = i32::try_from(distance).unwrap() * if result.times >= 3 { -1 } else { 1 };
    }
    result
}

fn write_repetition(stack: &mut StateStack, info: &RepetitionInfo) {
    let state = stack.current_mut();
    state.repetition_counter = info.times;
    state.repetition_distance = info.distance;
    state.repetition_type = info.repetition_type;
    state.exact_repetition_type = info.exact_type;
}

fn compute(pos: &Position, stack: &StateStack) -> RepetitionInfo {
    let idx = stack.current_index();
    let current = stack.hot(idx);
    pos.compute_repetition_info(
        stack,
        idx,
        current.plies_from_null,
        current.hand,
        current.continuous_check,
        current.board_key,
    )
}

#[test]
fn repetition_matches_full_history_with_incomparable_hands_and_checks() {
    let mut pawn = Hand::ZERO;
    pawn.add(HandPiece::PAWN, 2);
    let mut extra_pawn = pawn;
    extra_pawn.add(HandPiece::PAWN, 1);
    let mut gold = Hand::ZERO;
    gold.add(HandPiece::GOLD, 1);
    let hands = [
        pawn, pawn, pawn, extra_pawn, extra_pawn, gold, extra_pawn, pawn, pawn, extra_pawn, pawn,
        gold, pawn,
    ];
    let pos = hirate_position();
    for check_mode in 0..4 {
        for root in 0..hands.len() {
            for null_at in [None, Some(5)] {
                let mut stack = StateStack::new();
                let mut null_boundary = 0;
                for (sample, &hand) in hands.iter().enumerate() {
                    let idx = sample * 4;
                    while stack.current_index() < idx {
                        stack.push_empty();
                        let filler_ply =
                            u16::try_from(stack.current_index() - null_boundary).unwrap();
                        stack.current_mut().plies_from_null = filler_ply;
                    }
                    if null_at == Some(sample) {
                        null_boundary = idx;
                    }
                    let plies = u16::try_from(idx - null_boundary).unwrap();
                    let checks = match check_mode {
                        0 => [0, 0],
                        1 => [plies, 0],
                        2 => [0, plies],
                        _ => {
                            if sample < 6 {
                                [plies, 0]
                            } else {
                                [0, plies.min(4)]
                            }
                        }
                    };
                    let state = stack.current_mut();
                    state.board_key = ZobristKey::from_u64(1);
                    state.hand = hand;
                    state.plies_from_null = plies;
                    state.continuous_check = checks;
                    let actual = compute(&pos, &stack);
                    let expected = reference(&stack);
                    assert_eq!(actual.times, expected.times);
                    assert_eq!(actual.distance, expected.distance);
                    assert_eq!(actual.repetition_type, expected.repetition_type);
                    assert_eq!(actual.exact_type, expected.exact_type);
                    write_repetition(&mut stack, &actual);
                    if sample == root {
                        stack.prepare_repetition_index();
                    }
                }
            }
        }
    }
}

#[test]
fn repetition_respects_truncated_history() {
    let pos = hirate_position();
    let mut stack = StateStack::new();
    let mut pawn = Hand::ZERO;
    pawn.add(HandPiece::PAWN, 1);
    for sample in 0..=3 {
        while stack.current_index() < sample * 4 {
            stack.push_empty();
            let filler_ply = u16::try_from(stack.current_index()).unwrap();
            stack.current_mut().plies_from_null = filler_ply;
        }
        let state = stack.current_mut();
        state.board_key = ZobristKey::from_u64(1);
        state.hand = if sample == 0 { Hand::ZERO } else { pawn };
        state.plies_from_null = u16::try_from(sample * 4).unwrap();
        let info = compute(&pos, &stack);
        write_repetition(&mut stack, &info);
    }
    for start in 1..=12 {
        let mut truncated = stack.clone_suffix(start);
        let idx = truncated.current_index() + 4;
        while truncated.current_index() < idx {
            truncated.push_empty();
            let filler_ply = u16::try_from(truncated.current_index() + start).unwrap();
            truncated.current_mut().plies_from_null = filler_ply;
        }
        let state = truncated.current_mut();
        state.board_key = ZobristKey::from_u64(1);
        state.hand = Hand::ZERO;
        state.plies_from_null = 16;
        let info = compute(&pos, &truncated);
        assert_eq!(info.times, 0);
        assert_eq!(info.repetition_type, RepetitionState::Inferior);
        assert_eq!(info.distance, 4);
    }
}
