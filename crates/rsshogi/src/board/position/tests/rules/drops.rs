use crate::types::{Move, Move32, PieceType, Square};

#[test]
fn test_is_legal_rejects_invalid_raw_move32_without_panicking() {
    let pos = crate::board::hirate_position();
    let invalid = Move32::from_raw((82 << 7) | 1);
    assert!(!pos.is_legal_move32(invalid));
    let drop_and_promote = Move32::from_raw(u32::from(
        Move::drop(PieceType::PAWN, Square::from_usi("5e").unwrap()).raw() | Move::MOVE_PROMOTE,
    ));
    assert!(!pos.is_legal_move32(drop_and_promote));
}

#[test]
fn test_is_legal_rejects_dead_end_drops_and_unpromoted_knight_move() {
    let pos =
        crate::board::position_from_sfen("4k4/9/9/9/9/9/9/9/4K4 b PLN 1").expect("valid SFEN");
    for usi in ["P*4a", "L*4a", "N*4a", "N*4b"] {
        let mv = Move::from_usi(usi).expect("valid move encoding");
        assert!(!pos.is_legal_move(mv), "{usi} must be rejected");
    }

    let knight_pos =
        crate::board::position_from_sfen("4k4/9/1N7/9/9/9/9/9/4K4 b - 1").expect("valid SFEN");
    assert!(!knight_pos.is_legal_move(Move::from_usi("8c7a").unwrap()));
    assert!(knight_pos.is_legal_move(Move::from_usi("8c7a+").unwrap()));
}

#[test]
fn test_pawn_drop_mate_reference_position_detected() {
    let sfen = "l+N4knl/6g2/4+P2p1/p2s1Pp1p/1pp1l2P1/P1sK2P1P/1P3S1r1/5G3/LN7 w R2BGSN4Pgp 106";
    let pos = crate::board::position_from_sfen(sfen).expect("valid SFEN");

    let to = Square::from_usi("6e").expect("valid square");
    let drop = Move32::drop(PieceType::PAWN, to, pos.turn());
    assert!(pos.gives_check_move32(drop), "drop should give check");
    assert!(!pos.is_legal_pawn_drop(pos.turn(), to));
    assert!(!pos.is_legal_move32(drop), "pawn drop mate must be rejected by is_legal");
}

#[test]
fn test_pawn_drop_mate_reference_position_not_detected() {
    let sfen = "l+N4knl/6g2/4+P2p1/p1s2Pp1p/1pp1l2P1/P1sK2P1P/1P3S1r1/5G3/LN7 w R2BGSN3Pg2p 1";
    let pos = crate::board::position_from_sfen(sfen).expect("valid SFEN");

    let to = Square::from_usi("6e").expect("valid square");
    let drop = Move32::drop(PieceType::PAWN, to, pos.turn());
    assert!(pos.gives_check_move32(drop), "drop should give check");
    assert!(pos.is_legal_pawn_drop(pos.turn(), to));
    assert!(pos.is_legal_move32(drop), "non-mating pawn drop must be legal");
}
