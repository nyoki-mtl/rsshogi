//! プロセス全体の履歴保持上限と、索引付きコピーの互換性を専用バイナリで検査する。
//!
//! static を変更するため、このバイナリにはテストを 1 件だけ置く。

// 2.0.0 で削除するまで、公開済み setter の動作を検査する。
#![allow(deprecated)]

use rsshogi::board::{Position, position_from_sfen};
use rsshogi::types::{Move, Move32, RepetitionState};

const LONG_CYCLE: [&str; 18] = [
    "9i8i", "1a2a", "8i7i", "2a3a", "7i7h", "3a3b", "7h7g", "3b3c", "7g8g", "3c2c", "8g9g", "2c1c",
    "9g9h", "1c1b", "9h8h", "1b2b", "8h9i", "2b1a",
];

struct RestoreDefaultLimit;

impl Drop for RestoreDefaultLimit {
    fn drop(&mut self) {
        Position::set_max_repetition_ply(u16::MAX);
    }
}

fn apply(position: &mut Position, usi: &str) -> Move32 {
    let mv = Move::from_usi(usi).unwrap();
    assert!(position.is_legal_move(mv), "{usi}");
    let mv = position.move32_from_move(mv);
    position.try_apply_move32(mv).unwrap();
    mv
}

fn assert_same_repetition(plain: &Position, indexed: &Position) {
    assert_eq!(plain.to_sfen(None), indexed.to_sfen(None));
    assert_eq!(plain.key(), indexed.key());
    assert_eq!(plain.repetition_counter(), indexed.repetition_counter());
    assert_eq!(plain.repetition_distance(), indexed.repetition_distance());
    assert_eq!(plain.repetition_state(), indexed.repetition_state());
    for ply in [0, 1, 4, 17, 18, 64, usize::MAX] {
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

fn check_limits(root: &Position, history: &[&str], future: &[&str], limits: &[u16]) {
    let mut source = root.clone();
    let mut snapshots = vec![source.to_sfen(None)];
    for usi in history {
        apply(&mut source, usi);
        snapshots.push(source.to_sfen(None));
    }
    for &limit in limits {
        Position::set_max_repetition_ply(limit);
        let mut compact = source.clone_compact();
        assert_eq!(compact.state_stack_depth(), source.state_stack_depth());
        assert_same_repetition(&source, &compact);
        for _ in history {
            compact.undo_move32(compact.last_move()).unwrap();
        }
        assert_eq!(compact.to_sfen(None), root.to_sfen(None));
        assert_eq!(compact.state_stack_depth(), root.state_stack_depth());
        let kept = usize::from(limit).min(history.len());
        let mut indexed = source.clone_for_search();
        // 独立した SFEN から保持対象だけを再生し、索引のない参照局面を作る。
        let mut plain = position_from_sfen(&snapshots[history.len() - kept]).unwrap();
        for usi in &history[history.len() - kept..] {
            apply(&mut plain, usi);
        }
        assert_same_repetition(&plain, &indexed);

        // 保持された履歴をちょうど kept 手だけ undo でき、それ以前には戻れない。
        let mut undo_copy = indexed.clone();
        for _ in 0..kept {
            undo_copy.undo_move32(undo_copy.last_move()).unwrap();
        }
        assert_eq!(undo_copy.to_sfen(None), snapshots[history.len() - kept]);
        assert!(undo_copy.undo_move32(undo_copy.last_move()).is_err());

        // setter は既存コピーの保持範囲を変えず、新しい clone_for_search にだけ効く。
        Position::set_max_repetition_ply(0);
        for usi in future {
            apply(&mut plain, usi);
            apply(&mut indexed, usi);
            assert_same_repetition(&plain, &indexed);
        }
        if future.len() >= LONG_CYCLE.len() * 3 {
            assert_eq!(indexed.repetition_state(), RepetitionState::Draw);
        }

        // 通常の clone は上限 0 でも元の履歴を維持する。
        let mut full = source.clone();
        for _ in history {
            full.undo_move32(full.last_move()).unwrap();
        }
        assert_eq!(full.to_sfen(None), snapshots[0]);
    }
}

#[test]
fn global_history_limit_preserves_bounded_indexed_clone_behavior() {
    let _restore = RestoreDefaultLimit;
    let root = position_from_sfen("8k/9/9/9/9/9/9/9/K8 b - 1").unwrap();
    let mut future = vec![LONG_CYCLE[17]];
    future.extend(LONG_CYCLE.repeat(3));
    check_limits(&root, &LONG_CYCLE[..17], &future, &[0, 1, 4, 16, 17, 18, u16::MAX]);

    let mut after_null = rsshogi::board::hirate_position();
    apply(&mut after_null, "7g7f");
    apply(&mut after_null, "3c3d");
    after_null.apply_null_move().unwrap();
    // 上限が大きくても null move より前の履歴は持ち込まない。
    check_limits(&after_null, &["2c2d", "2g2f", "2d2e"], &["2f2e"], &[0, 1, 2, 3, 4, u16::MAX]);
}
