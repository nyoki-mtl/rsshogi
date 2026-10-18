//! `check_candidate_bb` が「王手になりうる移動元」の上位集合であることを検査する。

use rsshogi::board::{Move32List, check_candidate_bb, generate_legal_all_move32, hirate_position};
use rsshogi::types::{Color, PieceType};

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 =
            self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
}

/// 実際に王手になった盤上手の移動元が、候補集合か開き王手の候補に含まれることを確かめる。
///
/// `check_candidate_bb` は直接王手だけを覆うので、開き王手は
/// `blockers_for_king` 側で拾う。両方から漏れる移動元が 1 つでもあれば
/// 王手生成が手を取りこぼす。
#[test]
fn every_checking_board_move_starts_from_a_candidate_square() {
    let mut rng = Lcg(0x1234_5678_9abc_def0);
    let mut checked_moves = 0usize;
    let mut seen_types = [false; PieceType::COUNT];

    for seed in 0..150u64 {
        let mut pos = hirate_position();
        pos.init_stack();
        rng.0 = 0x9e37_79b9_7f4a_7c15 ^ seed.wrapping_mul(0x5851_f42d_4c95_7f2d);

        for _ply in 0..200 {
            let mut moves = Move32List::new();
            generate_legal_all_move32(&pos, &mut moves);
            if moves.is_empty() {
                break;
            }

            let us = pos.turn();
            let them = us.flip();
            let king_sq = pos.king_square(them);
            let discovered = pos.blockers_for_king(them);
            if king_sq.is_valid() {
                for &mv in moves.iter() {
                    if mv.is_drop() || !pos.gives_check_move32(mv) {
                        continue;
                    }
                    let from = mv.from_sq();
                    let mover = pos.piece_on(from).piece_type();
                    let candidates = check_candidate_bb(us, mover, king_sq);
                    assert!(
                        candidates.test(from) || discovered.test(from),
                        "candidate set misses a checking move: sfen={} move={} mover={mover:?}",
                        pos.to_sfen(None),
                        mv.to_usi(),
                    );
                    if candidates.test(from) {
                        seen_types[mover.to_index()] = true;
                    }
                    checked_moves += 1;
                }
            }

            let mv = moves[(rng.next() as usize) % moves.len()];
            pos.apply_move32(mv);
        }
    }

    assert!(checked_moves > 10_000, "corpus is too small: {checked_moves} checking moves");
    // 玉以外の全駒種が王手の実例で踏まれていることを確かめる。
    for pt in PieceType::iter() {
        if pt == PieceType::KING || pt.raw() == 0 {
            continue;
        }
        assert!(seen_types[pt.to_index()], "no checking move observed for {pt:?}");
    }
}

/// 飛・龍は絞り込めないため全升、玉は王手をかけられないため空集合を返す。
#[test]
fn sliders_are_unrestricted_and_the_king_is_empty() {
    use rsshogi::board::Bitboard;
    use rsshogi::types::Square;

    for ksq_idx in 0..Square::COUNT {
        let ksq = Square::from_index(ksq_idx);
        for us in [Color::BLACK, Color::WHITE] {
            assert_eq!(check_candidate_bb(us, PieceType::ROOK, ksq), Bitboard::ALL);
            assert_eq!(check_candidate_bb(us, PieceType::DRAGON, ksq), Bitboard::ALL);
            assert_eq!(check_candidate_bb(us, PieceType::KING, ksq), Bitboard::EMPTY);
        }
    }
}
