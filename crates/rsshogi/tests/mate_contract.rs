use rsshogi::board::{
    Move32List, Position, generate_legal_all_move32, hirate_position, move_from_usi,
};
use rsshogi::mate::solve_mate_in_one;
use rsshogi::types::Move32;

const MATE_POSITIONS: &[&str] = &[
    "lnsG5/4g4/prpp1p1pp/1p4p1k/4+B4/2P1P3P/P+b1PSP1L1/4K2SL/2G2G1r1 b SP3nl3p 73",
    "ln2+P2nl/2R1+S1g2/p2p1p1p+B/8p/5+R3/2p3PkP/PP1PPP3/2+bS1KS2/5G1NL b GL4Pgsn 83",
    "l2+R3g1/2ln5/2k1ps+Bp1/2p3P2/p3Sp1P1/7b1/PLPKPP3/1S1G2G2/LN1s+n+r1N1 b G4P3p 103",
    "l2+S3kl/9/3p1pG+S1/ppp3Pp1/4+RP1np/P1n1S1p2/1P1PS1gN1/2G1G4/L2K3RL b 3P2bn2p 115",
    "l1ggk3l/3plsG2/4sp+P2/p2+Bp2pp/5n1+b1/2+RSPN3/Pp1P1P+n1P/3K5/L7+n b RS3Pg2p 117",
    "4K4/2sP1Ps2/9/4r4/9/9/9/9/4k4 w - 1",
];

fn position(sfen: &str) -> Position {
    Position::from_sfen(sfen).unwrap_or_else(|error| panic!("invalid fixture {sfen}: {error}"))
}

fn assert_checkmate_move(pos: &Position, mv: Move32) {
    assert!(pos.is_legal_move32(mv), "illegal candidate: {}", mv.to_usi());
    assert!(pos.gives_check_move32(mv), "candidate is not check: {}", mv.to_usi());

    let mut next = pos.clone();
    next.init_stack();
    next.apply_move32(mv);
    let mut replies = Move32List::new();
    generate_legal_all_move32(&next, &mut replies);
    assert!(replies.is_empty(), "candidate permits a reply: {}", mv.to_usi());
}

fn assert_solver_finds_mate(pos: &Position) -> Move32 {
    let mv = solve_mate_in_one(pos).expect("mate in one must be found");
    assert_checkmate_move(pos, mv);
    mv
}

#[test]
fn finds_rule_valid_mates_for_both_colors_and_drops() {
    for sfen in MATE_POSITIONS {
        assert_solver_finds_mate(&position(sfen));
    }
}

#[test]
fn capture_and_promotion_candidates_are_mates() {
    let capture =
        position("6+S2/ln5gP/n1sg+R1n1+N/1S2ppp2/P2gkP3/3l3P1/1+bPpKB3/s3G1Pp+l/L4r3 w 2P5p 178");
    let capture_move = move_from_usi(&capture, "3c4e").expect("valid capture fixture");
    assert!(!capture.piece_on(capture_move.to_sq()).is_empty());
    assert_checkmate_move(&capture, capture_move);
    assert_solver_finds_mate(&capture);

    let promotion =
        position("6+S2/ln5gP/n1sg+R1n1+N/1S2ppp2/P3kP3/3lB2P1/1+bPpK4/s3G1Pp+l/L4r3 w G2P5p 180");
    let promotion_move = move_from_usi(&promotion, "4i4f+").expect("valid promotion fixture");
    assert!(promotion_move.is_promotion());
    assert_checkmate_move(&promotion, promotion_move);
    assert_solver_finds_mate(&promotion);
}

#[test]
fn returns_none_without_a_mate() {
    assert!(solve_mate_in_one(&hirate_position()).is_none());
    assert!(solve_mate_in_one(&position("4k4/9/4G4/9/9/9/9/9/4K4 b - 1")).is_none());
}

#[test]
fn checking_moves_with_escape_capture_or_interposition_are_not_mates() {
    let cases = [
        ("4k4/9/4G4/9/9/9/9/9/4K4 b - 1", "5c4b", "5a6a"),
        ("4k4/9/4G4/9/9/9/9/9/4K4 b - 1", "5c5b", "5a5b"),
        ("4k4/9/4G4/4R4/9/9/9/9/4K4 b g 1", "5c4c", "G*5b"),
    ];

    for (sfen, checking_usi, reply_usi) in cases {
        let pos = position(sfen);
        let checking = move_from_usi(&pos, checking_usi).expect("valid checking fixture");
        assert!(pos.is_legal_move32(checking));
        assert!(pos.gives_check_move32(checking));

        let mut next = pos.clone();
        next.init_stack();
        next.apply_move32(checking);
        let reply = move_from_usi(&next, reply_usi).expect("valid reply fixture");
        let mut replies = Move32List::new();
        generate_legal_all_move32(&next, &mut replies);
        assert!(replies.iter().any(|&mv| mv == reply));
    }
}
