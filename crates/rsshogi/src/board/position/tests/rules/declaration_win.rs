use crate::board::InitialPosition;
use crate::board::position::DeclarationDetail;
use crate::types::{
    Bitboard, Color, EnteringKingRule, HandPiece, MOVE_NONE, MOVE_WIN, PieceType, Rank, Square,
};

fn required_points_for(rule: EnteringKingRule, initial_position: InitialPosition) -> i32 {
    let mut pos =
        crate::board::position_from_sfen(initial_position.to_sfen()).expect("valid initial sfen");
    pos.set_entering_king_rule(rule);

    match pos.evaluate_declaration().detail {
        DeclarationDetail::PointRule { required_points, .. } => required_points,
        _ => panic!("expected PointRule detail"),
    }
}

#[test]
fn test_declaration_win_succeeds_for_black() {
    let sfen = "K+N5+L1/G+L+P+B1+R+P2/3+P2G2/9/2+p+n5/3s2+ss1/3+p+p1+s1+r/7g+n/6g+nk b 2L8Pb4p 1";
    let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    pos.set_entering_king_rule(crate::types::EnteringKingRule::Point27);

    assert_eq!(pos.declaration_win_move(), MOVE_WIN, "black meets declaration win requirements");
}

#[test]
fn test_declaration_win_fails_on_insufficient_points() {
    let sfen = "K+N5+L1/G+L+P+B1+R+P2/3+P2G2/9/2+p+n5/3s2+ss1/3+p+p1+s1+r/7g+n/6g+nk b 2L7Pb5p 1";
    let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    pos.set_entering_king_rule(crate::types::EnteringKingRule::Point27);

    assert_eq!(pos.declaration_win_move(), MOVE_NONE, "insufficient points should fail");
}

#[test]
fn test_declaration_win_fails_when_king_outside_enemy_camp() {
    let sfen =
        "1+N2+P2+L1/G1+P+B1+R+P2/1+L1+P5/K8/2+p+n5/3s2+ss1/3+p+p1+s1+r/7g+n/6g+nk b G2L7Pb4p 1";
    let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    pos.set_entering_king_rule(crate::types::EnteringKingRule::Point27);

    assert_eq!(pos.declaration_win_move(), MOVE_NONE, "king outside enemy camp should fail");
}

#[test]
fn test_declaration_win_fails_on_insufficient_enemy_camp_pieces() {
    let sfen = "K+N5+L1/G+L+P+B1+R+P2/3+P5/9/2+p+n5/3s2+ss1/3+p+p1+s1+r/7g+n/6g+nk b G2L8Pb4p 1";
    let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    pos.set_entering_king_rule(crate::types::EnteringKingRule::Point27);

    assert_eq!(pos.declaration_win_move(), MOVE_NONE, "insufficient enemy camp pieces should fail");
}

#[test]
fn test_declaration_win_fails_while_in_check() {
    let sfen =
        "K+N5+L1/G1+P+B1+R+P2/+P+L1+P2G2/9/2+p+n5/3s2+ss1/3+p+p1b1+r/6+sg+n/6g+nk b 2L7P4p 1";
    let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    pos.set_entering_king_rule(crate::types::EnteringKingRule::Point27);

    assert_eq!(pos.declaration_win_move(), MOVE_NONE, "cannot declare while in check");
}

#[test]
fn test_declaration_win_succeeds_for_white() {
    let sfen = "K+N5+L1/G+P+P+B1+R+P2/+P+L1+P2G2/9/9/9/6b1+r/4+ss+sg+n/5+pg+nk w 2L5Psn7p 1";
    let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    pos.set_entering_king_rule(crate::types::EnteringKingRule::Point27);

    assert_eq!(pos.declaration_win_move(), MOVE_WIN, "white meets declaration win requirements");
}

// --- evaluate_declaration() tests ---

#[test]
fn test_evaluate_declaration_no_rule() {
    let pos = crate::board::hirate_position();
    let eval = pos.evaluate_declaration();
    assert!(!eval.can_declare);
    assert!(matches!(eval.detail, DeclarationDetail::NoRule));
}

#[test]
fn test_evaluate_declaration_point_rule_succeeds() {
    let sfen = "K+N5+L1/G+L+P+B1+R+P2/3+P2G2/9/2+p+n5/3s2+ss1/3+p+p1+s1+r/7g+n/6g+nk b 2L8Pb4p 1";
    let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    pos.set_entering_king_rule(EnteringKingRule::Point27);

    let eval = pos.evaluate_declaration();
    assert!(eval.can_declare);
    assert_eq!(eval.rule, EnteringKingRule::Point27);

    if let DeclarationDetail::PointRule {
        not_in_check,
        king_in_enemy_camp,
        enough_pieces,
        points,
        required_points,
        ..
    } = eval.detail
    {
        assert!(not_in_check);
        assert!(king_in_enemy_camp);
        assert!(enough_pieces);
        assert!(points >= required_points);
    } else {
        panic!("expected PointRule detail");
    }
}

#[test]
fn test_evaluate_declaration_point_rule_fails_insufficient_points() {
    let sfen = "K+N5+L1/G+L+P+B1+R+P2/3+P2G2/9/2+p+n5/3s2+ss1/3+p+p1+s1+r/7g+n/6g+nk b 2L7Pb5p 1";
    let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    pos.set_entering_king_rule(EnteringKingRule::Point27);

    let eval = pos.evaluate_declaration();
    assert!(!eval.can_declare);

    if let DeclarationDetail::PointRule { points, required_points, .. } = eval.detail {
        assert!(points < required_points, "points={points} should be < required={required_points}");
    } else {
        panic!("expected PointRule detail");
    }
}

#[test]
fn test_evaluate_declaration_point_rule_fails_king_outside() {
    let sfen =
        "1+N2+P2+L1/G1+P+B1+R+P2/1+L1+P5/K8/2+p+n5/3s2+ss1/3+p+p1+s1+r/7g+n/6g+nk b G2L7Pb4p 1";
    let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    pos.set_entering_king_rule(EnteringKingRule::Point27);

    let eval = pos.evaluate_declaration();
    assert!(!eval.can_declare);

    if let DeclarationDetail::PointRule { king_in_enemy_camp, .. } = eval.detail {
        assert!(!king_in_enemy_camp);
    } else {
        panic!("expected PointRule detail");
    }
}

#[test]
fn test_evaluate_declaration_point_rule_reports_points_without_king_subtraction_outside_camp() {
    let sfen =
        "1+N2+P2+L1/G1+P+B1+R+P2/1+L1+P5/K8/2+p+n5/3s2+ss1/3+p+p1+s1+r/7g+n/6g+nk b G2L7Pb4p 1";
    let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    pos.set_entering_king_rule(EnteringKingRule::Point27);

    let eval = pos.evaluate_declaration();

    if let DeclarationDetail::PointRule { king_in_enemy_camp, pieces_in_camp, points, .. } =
        eval.detail
    {
        assert!(!king_in_enemy_camp);

        let enemy_camp_mask = Bitboard::rank_mask(Rank::new(0))
            | Bitboard::rank_mask(Rank::new(1))
            | Bitboard::rank_mask(Rank::new(2));
        let major_count = i32::try_from(
            ((pos.bitboards().pieces_for(PieceType::BISHOP, Color::BLACK)
                | pos.bitboards().pieces_for(PieceType::HORSE, Color::BLACK)
                | pos.bitboards().pieces_for(PieceType::ROOK, Color::BLACK)
                | pos.bitboards().pieces_for(PieceType::DRAGON, Color::BLACK))
                & enemy_camp_mask)
                .count(),
        )
        .expect("major count fits in i32");
        let hand_points = i32::try_from(pos.hand(Color::BLACK).count(HandPiece::ROOK))
            .expect("rook count fits in i32")
            * 5
            + i32::try_from(pos.hand(Color::BLACK).count(HandPiece::BISHOP))
                .expect("bishop count fits in i32")
                * 5
            + i32::try_from(pos.hand(Color::BLACK).count(HandPiece::GOLD))
                .expect("gold count fits in i32")
            + i32::try_from(pos.hand(Color::BLACK).count(HandPiece::SILVER))
                .expect("silver count fits in i32")
            + i32::try_from(pos.hand(Color::BLACK).count(HandPiece::KNIGHT))
                .expect("knight count fits in i32")
            + i32::try_from(pos.hand(Color::BLACK).count(HandPiece::LANCE))
                .expect("lance count fits in i32")
            + i32::try_from(pos.hand(Color::BLACK).count(HandPiece::PAWN))
                .expect("pawn count fits in i32");

        assert_eq!(points, hand_points + pieces_in_camp + major_count * 4);
    } else {
        panic!("expected PointRule detail");
    }
}

#[test]
fn test_evaluate_declaration_point_rule_fails_in_check() {
    let sfen =
        "K+N5+L1/G1+P+B1+R+P2/+P+L1+P2G2/9/2+p+n5/3s2+ss1/3+p+p1b1+r/6+sg+n/6g+nk b 2L7P4p 1";
    let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    pos.set_entering_king_rule(EnteringKingRule::Point27);

    let eval = pos.evaluate_declaration();
    assert!(!eval.can_declare);

    if let DeclarationDetail::PointRule { not_in_check, .. } = eval.detail {
        assert!(!not_in_check);
    } else {
        panic!("expected PointRule detail");
    }
}

#[test]
fn test_evaluate_declaration_consistency_with_declaration_win_move() {
    // evaluate_declaration と declaration_win_move の結果が一致するか
    let sfens = [
        "K+N5+L1/G+L+P+B1+R+P2/3+P2G2/9/2+p+n5/3s2+ss1/3+p+p1+s1+r/7g+n/6g+nk b 2L8Pb4p 1",
        "K+N5+L1/G+L+P+B1+R+P2/3+P2G2/9/2+p+n5/3s2+ss1/3+p+p1+s1+r/7g+n/6g+nk b 2L7Pb5p 1",
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
    ];

    for sfen in sfens {
        let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
        pos.set_entering_king_rule(EnteringKingRule::Point27);

        let win_move = pos.declaration_win_move();
        let eval = pos.evaluate_declaration();

        assert_eq!(
            eval.can_declare,
            win_move == MOVE_WIN,
            "consistency check failed for sfen: {sfen}"
        );
    }
}

#[test]
fn test_entering_king_required_points_match_yaneuraou_non_handicap_rules() {
    assert_eq!(required_points_for(EnteringKingRule::Point27, InitialPosition::Standard), 28);
    assert_eq!(
        required_points_for(EnteringKingRule::Point27, InitialPosition::Handicap2Pieces),
        27
    );
    assert_eq!(required_points_for(EnteringKingRule::Point24, InitialPosition::Standard), 31);
    assert_eq!(
        required_points_for(EnteringKingRule::Point24, InitialPosition::Handicap2Pieces),
        31
    );
}

#[test]
fn test_entering_king_required_points_match_yaneuraou_handicap_rules() {
    assert_eq!(
        required_points_for(EnteringKingRule::Point27Handicap, InitialPosition::Standard),
        28
    );
    assert_eq!(
        required_points_for(EnteringKingRule::Point27Handicap, InitialPosition::Handicap2Pieces),
        17
    );
    assert_eq!(
        required_points_for(EnteringKingRule::Point27Handicap, InitialPosition::Handicap4Pieces),
        15
    );
    assert_eq!(
        required_points_for(EnteringKingRule::Point24Handicap, InitialPosition::Standard),
        31
    );
    assert_eq!(
        required_points_for(EnteringKingRule::Point24Handicap, InitialPosition::Handicap2Pieces),
        21
    );
    assert_eq!(
        required_points_for(EnteringKingRule::Point24Handicap, InitialPosition::Handicap4Pieces),
        19
    );
}

#[test]
fn test_evaluate_declaration_try_rule_succeeds_with_empty_target() {
    let sfen = "k8/4K4/9/9/9/9/9/9/9 b - 1";
    let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    pos.set_entering_king_rule(EnteringKingRule::TryRule);

    let eval = pos.evaluate_declaration();
    assert!(eval.can_declare);

    if let DeclarationDetail::TryRule { king_square, try_square, can_reach, can_occupy, is_safe } =
        eval.detail
    {
        assert_eq!(king_square, Some("5b".parse::<Square>().expect("valid square")));
        assert_eq!(try_square, "5a".parse::<Square>().expect("valid square"));
        assert!(can_reach);
        assert!(can_occupy);
        assert!(is_safe);
    } else {
        panic!("expected TryRule detail");
    }

    let mv = pos.declaration_win_move();
    assert!(mv.is_normal());
    assert_eq!(mv.from_sq(), "5b".parse::<Square>().expect("valid square"));
    assert_eq!(mv.to_sq(), "5a".parse::<Square>().expect("valid square"));
}

#[test]
fn test_evaluate_declaration_try_rule_reports_own_piece_blocking_target() {
    let sfen = "4G3k/4K4/9/9/9/9/9/9/9 b - 1";
    let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    pos.set_entering_king_rule(EnteringKingRule::TryRule);

    let eval = pos.evaluate_declaration();
    assert!(!eval.can_declare);

    if let DeclarationDetail::TryRule { can_reach, can_occupy, is_safe, .. } = eval.detail {
        assert!(can_reach);
        assert!(!can_occupy);
        assert!(is_safe);
    } else {
        panic!("expected TryRule detail");
    }

    assert_eq!(pos.declaration_win_move(), MOVE_NONE);
}
