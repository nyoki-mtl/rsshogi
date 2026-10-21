use super::helpers::move_from_usi;
use crate::board::{Position, hirate_position, position_from_sfen};
use crate::types::{Move32, RepetitionState};

const LONG_CYCLE: [&str; 18] = [
    "9i8i", "1a2a", "8i7i", "2a3a", "7i7h", "3a3b", "7h7g", "3b3c", "7g8g", "3c2c", "8g9g", "2c1c",
    "9g9h", "1c1b", "9h8h", "1b2b", "8h9i", "2b1a",
];

fn assert_same_repetition(plain: &Position, indexed: &Position) {
    assert_eq!(plain.to_sfen(None), indexed.to_sfen(None));
    assert_eq!(plain.key(), indexed.key());
    assert_eq!(plain.plies_from_null(), indexed.plies_from_null());
    assert_eq!(plain.continuous_checks(), indexed.continuous_checks());
    assert_eq!(plain.repetition_counter(), indexed.repetition_counter());
    assert_eq!(plain.repetition_distance(), indexed.repetition_distance());
    assert_eq!(plain.repetition_times(), indexed.repetition_times());
    assert_eq!(plain.repetition_type(), indexed.repetition_type());
    assert_eq!(plain.repetition_state(), indexed.repetition_state());
    for ply in [0, 3, 4, 8, 16, 64, usize::MAX] {
        assert_eq!(
            plain.repetition_state_with_distance(ply),
            indexed.repetition_state_with_distance(ply)
        );
        assert_eq!(plain.repetition_state_with_ply(ply), indexed.repetition_state_with_ply(ply));
        assert_eq!(
            plain.cached_repetition_state_with_ply(ply),
            indexed.cached_repetition_state_with_ply(ply)
        );
    }
}

fn apply_pair(plain: &mut Position, indexed: &mut Position, usi: &str) -> Move32 {
    let mv = move_from_usi(plain, usi);
    plain.try_apply_move32(mv).unwrap();
    indexed.try_apply_move32(mv).unwrap();
    assert_same_repetition(plain, indexed);
    mv
}

#[test]
fn indexed_repetition_matches_full_scan_across_root_and_long_cycles() {
    let root = position_from_sfen("8k/9/9/9/9/9/9/9/K8 b - 1").unwrap();
    let sequence = LONG_CYCLE.repeat(3);
    for split in [0, 1, 17, 18, 35, 53] {
        let mut plain = root.clone();
        for usi in &sequence[..split] {
            let mv = move_from_usi(&plain, usi);
            plain.apply_move32(mv);
        }
        let mut indexed = plain.clone_for_search();
        assert_same_repetition(&plain, &indexed);
        for usi in &sequence[split..] {
            apply_pair(&mut plain, &mut indexed, usi);
        }
        assert_eq!(indexed.repetition_state(), RepetitionState::Draw);
        assert_eq!(indexed.repetition_distance(), -18);
    }
}

#[test]
fn indexed_repetition_preserves_captures_drops_hands_and_perpetual_check() {
    let cases = [
        (
            "4r3k/9/9/9/9/9/9/9/K4R3 b P 1",
            vec!["P*5h", "5a5h", "9i8i", "P*4g", "4i4g", "1a2a", "4g4i", "2a1a", "8i9i", "5h5a"],
        ),
        (
            "lr2+R2G1/6g1k/p3ppppp/2p1b4/9/2P+b4P/P+p3PPP1/4G1SK1/LN3+p1NL b 2SNLgsn3p 1",
            vec!["2a1a", "1b2b", "1a2a", "2b1b"],
        ),
    ];
    for (sfen, cycle) in cases {
        let sequence = cycle.repeat(3);
        for split in 0..sequence.len() {
            let mut plain = position_from_sfen(sfen).unwrap();
            for usi in &sequence[..split] {
                let mv = move_from_usi(&plain, usi);
                plain.apply_move32(mv);
            }
            let mut indexed = plain.clone_for_search();
            for usi in &sequence[split..] {
                apply_pair(&mut plain, &mut indexed, usi);
            }
        }
    }
}

#[test]
fn indexed_repetition_keeps_hand_dominance_candidates() {
    let mut plain =
        position_from_sfen("lr6l/4g1pkp/2ns1s3/4pp1SP/PPBP2Pp1/2gpPP3/2B2S3/2K6/L6RL b 4P2g3np 1")
            .unwrap();
    let mut indexed = plain.clone_for_search();
    for usi in ["P*2c", "2b2a", "2c2b+", "2a2b"] {
        apply_pair(&mut plain, &mut indexed, usi);
    }
    assert_eq!(indexed.repetition_state(), RepetitionState::Inferior);
}

#[test]
#[allow(deprecated)] // 1.x の探索用 null move と根履歴索引の組み合わせを検査する。
fn indexed_repetition_handles_clone_null_undo_before_root_and_reset() {
    let mut plain = hirate_position();
    for usi in ["7g7f", "3c3d", "8h2b+", "3a2b"] {
        let mv = move_from_usi(&plain, usi);
        plain.apply_move32(mv);
    }
    let mut indexed = plain.clone_for_search();
    plain.prepare_search_state_capacity(8).unwrap();
    indexed.prepare_search_state_capacity(8).unwrap();
    let mv = apply_pair(&mut plain, &mut indexed, "B*4e");
    // SAFETY: 両局面の stack には、直前に適用した mv の state がある。
    unsafe {
        plain.undo_move32_unchecked(mv);
        indexed.undo_move32_unchecked(mv);
    }
    assert_same_repetition(&plain, &indexed);
    for _ in 0..3 {
        plain.try_apply_search_null_move().unwrap();
        indexed.try_apply_search_null_move().unwrap();
        assert_same_repetition(&plain, &indexed);
    }
    for _ in 0..3 {
        plain.undo_search_null_move().unwrap();
        indexed.undo_search_null_move().unwrap();
        assert_same_repetition(&plain, &indexed);
    }
    indexed = indexed.clone();
    assert_same_repetition(&plain, &indexed.clone_for_search());
    for _ in 0..2 {
        let mv = plain.last_move();
        plain.undo_move32(mv).unwrap();
        indexed.undo_move32(mv).unwrap();
        assert_same_repetition(&plain, &indexed);
    }
    for usi in ["2g2f", "8c8d", "2f2e", "8d8e"] {
        apply_pair(&mut plain, &mut indexed, usi);
    }
    plain.set_sfen(crate::board::STARTPOS_SFEN).unwrap();
    indexed.set_sfen(crate::board::STARTPOS_SFEN).unwrap();
    apply_pair(&mut plain, &mut indexed, "7g7f");
}
