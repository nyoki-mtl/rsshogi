//! 王手生成を、全候補から王手を選ぶ独立した参照経路と照合する。
//!
//! 参照経路は王手候補表や専用生成器を使わず、pseudo-legal 判定と王手判定で絞る。
//! 王手中は generic 経路が王手回避手に絞る契約も検査する。

use rsshogi::board::{
    Checks, ChecksAll, Move32List, MoveList, NonEvasions, NonEvasionsAll, Position, QuietChecks,
    QuietChecksAll, generate_legal_all_move32, generate_moves, hirate_position,
};
use rsshogi::movegen::generate_checks;

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 =
            self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
}

fn sorted(list: &MoveList) -> Vec<String> {
    let mut v: Vec<String> = list.iter().map(|m| m.to_usi()).collect();
    v.sort_unstable();
    v
}

fn reference_checks(pos: &Position, all: bool, quiet_only: bool) -> MoveList {
    let mut candidates = MoveList::new();
    if all {
        generate_moves::<NonEvasionsAll>(pos, &mut candidates);
    } else {
        generate_moves::<NonEvasions>(pos, &mut candidates);
    }
    let mut list = MoveList::new();
    for &mv in candidates.iter() {
        if (!quiet_only || pos.piece_on(mv.to_sq()).is_empty())
            && pos.is_pseudo_legal_move(mv, all)
            && pos.gives_check_move(mv)
        {
            list.push(mv);
        }
    }
    list
}

#[test]
fn check_generation_matches_reference_outside_check() {
    let mut rng = Lcg(0x1234_5678_9abc_def0);
    let mut compared = 0usize;

    for seed in 0..40u64 {
        let mut pos = hirate_position();
        pos.init_stack();
        rng.0 = 0x9e37_79b9_7f4a_7c15 ^ seed.wrapping_mul(0x5851_f42d_4c95_7f2d);

        for _ply in 0..160 {
            let mut legal = Move32List::new();
            generate_legal_all_move32(&pos, &mut legal);
            if legal.is_empty() {
                break;
            }
            pos.apply_move32(legal[(rng.next() as usize) % legal.len()]);

            if pos.is_in_check() {
                continue;
            }
            let sfen = pos.to_sfen(None);
            let p = Position::from_sfen(&sfen).expect("roundtrip");
            compared += 1;

            let mut generic = MoveList::new();
            generate_moves::<Checks>(&p, &mut generic);
            let expected = reference_checks(&p, false, false);
            assert_eq!(sorted(&generic), sorted(&expected), "Checks mismatch for {sfen}");

            let mut generic_all = MoveList::new();
            generate_moves::<ChecksAll>(&p, &mut generic_all);
            assert_eq!(
                sorted(&generic_all),
                sorted(&reference_checks(&p, true, false)),
                "ChecksAll mismatch for {sfen}"
            );

            let mut generic_quiet = MoveList::new();
            generate_moves::<QuietChecks>(&p, &mut generic_quiet);
            let expected_quiet = reference_checks(&p, false, true);
            assert_eq!(
                sorted(&generic_quiet),
                sorted(&expected_quiet),
                "QuietChecks mismatch for {sfen}"
            );

            let mut generic_quiet_all = MoveList::new();
            generate_moves::<QuietChecksAll>(&p, &mut generic_quiet_all);
            assert_eq!(
                sorted(&generic_quiet_all),
                sorted(&reference_checks(&p, true, true)),
                "QuietChecksAll mismatch for {sfen}"
            );
        }
    }

    assert!(compared > 4_000, "corpus is too small: {compared} positions");
}

/// 王手中は generic 経路が王手回避手に絞るため、専用経路より狭い集合を返す。
#[test]
fn generic_check_generation_stays_restricted_while_in_check() {
    let pos = Position::from_sfen(
        "1k+B4n1/l4sg1l/p1r1Npppn/9/1p1pp2Pp/1P1PG3P/4P1P1g/LBp2K1R1/1SG4NS w Sl3p 90",
    )
    .expect("valid fixture");
    assert!(pos.is_in_check());

    let mut generic = MoveList::new();
    generate_moves::<Checks>(&pos, &mut generic);
    let mut direct = MoveList::new();
    generate_checks(&pos, &mut direct);

    assert!(generic.is_empty());
    assert!(!direct.is_empty());
}
