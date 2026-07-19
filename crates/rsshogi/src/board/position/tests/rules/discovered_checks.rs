use crate::board::test_support::move_from_usi_expect;
use crate::board::{Move32List, generate_legal_all_move32};

#[test]
fn test_gives_check_detects_discovered_check() {
    let pos =
        crate::board::position_from_sfen("4k4/9/9/9/9/9/4S4/9/4R3K b - 1").expect("valid SFEN");

    let mv = move_from_usi_expect(&pos, "5g4f");
    assert!(pos.gives_check_move32(mv), "discovered check should be detected");

    let mut next = pos;
    next.init_stack();
    next.apply_move32(mv);
    assert!(!next.checkers().is_empty(), "move must give check on board");
}

#[test]
fn test_gives_check_does_not_trigger_when_still_aligned() {
    let pos =
        crate::board::position_from_sfen("4k4/9/9/9/9/9/4S4/9/4R3K b - 1").expect("valid SFEN");

    let mv = move_from_usi_expect(&pos, "5g5f");
    assert!(!pos.gives_check_move32(mv), "moving along the line must not be a discovered check");

    let mut next = pos;
    next.init_stack();
    next.apply_move32(mv);
    assert!(next.checkers().is_empty(), "move must not give check on board");
}

#[test]
fn test_gives_check_snapshot_paths_match_generic_path() {
    let sfens = [
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        "4k4/9/9/9/9/9/4S4/9/4R3K b - 1",
        "l+N4knl/6g2/4+P2p1/p1s2Pp1p/1pp1l2P1/P1sK2P1P/1P3S1r1/5G3/LN7 w R2BGSN3Pg2p 1",
        "l6+P1/3g3+R1/p1p1p1p2/3p1s3/1P3p3/1nP2Pkp1/P1KPP4/3S1g3/LN5LP b 2BGNrg2snl3p 121",
    ];

    for sfen in sfens {
        let pos = crate::board::position_from_sfen(sfen).expect("valid SFEN");
        let check_squares = *pos.check_squares_cache();
        let mut moves = Move32List::new();
        generate_legal_all_move32(&pos, &mut moves);

        for mv in moves.iter().copied() {
            let generic = pos.gives_check_move32(mv);
            assert_eq!(
                generic,
                pos.gives_check_with_check_squares(mv, &check_squares),
                "snapshot gives_check mismatch for {} on {sfen}",
                mv.to_usi()
            );

            if mv.has_piece_info()
                && check_squares.get(mv.piece_after_move().piece_type()).test(mv.to_sq())
            {
                assert!(
                    generic,
                    "direct check fast path must imply gives_check for {}",
                    mv.to_usi()
                );
            }
        }
    }
}
