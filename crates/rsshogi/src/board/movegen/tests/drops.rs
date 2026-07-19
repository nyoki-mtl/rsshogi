use super::{NonEvasionsAll, generate_moves};
use crate::board::MoveList;
use crate::types::{File, Move, PieceType, Rank};

#[test]
fn test_pawn_drops_with_double_pawn_rule() {
    // この局面ではFILE_2とFILE_8以外の筋に既に歩が存在するため、
    // 二歩ルールにより歩を打てるのはFILE_2とFILE_8のみ
    let sfen = "lnsgk1snl/6gb1/p1pppp2p/6pR1/9/1rP6/P2PPPP1P/1BG6/LNS1KGSNL b 2P2p 1";
    let pos = crate::board::position_from_sfen(sfen).expect("Invalid SFEN");
    let mut list = MoveList::new();

    generate_moves::<NonEvasionsAll>(&pos, &mut list);

    let mut pawn_drops: Vec<String> = list
        .iter()
        .filter(|m| m.is_drop() && m.dropped_piece() == Some(PieceType::PAWN))
        .map(|m| m.to_usi())
        .collect();
    pawn_drops.sort();

    // 期待される具体的な歩打ち手（FILE_2とFILE_8のみ）
    let expected =
        vec!["P*2c", "P*2e", "P*2f", "P*2g", "P*2h", "P*8b", "P*8c", "P*8d", "P*8e", "P*8g"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();

    assert_eq!(pawn_drops, expected, "Pawn drops should match expected legal squares");

    // 二歩ルールの検証：FILE_2とFILE_8以外には打てないことを確認
    let pawn_drop_moves: Vec<Move> = list
        .iter()
        .copied()
        .filter(|m| m.is_drop() && m.dropped_piece() == Some(PieceType::PAWN))
        .collect();

    for drop in &pawn_drop_moves {
        let file = drop.to_sq().file();
        assert!(
            file == File::FILE_2 || file == File::FILE_8,
            "Only FILE_2 and FILE_8 should allow pawn drops (found drop on {file:?})"
        );
    }
}

#[test]
fn test_multiple_piece_types_in_hand() {
    let sfen = "l4k1nl/5g1s1/p4p2p/3pp4/2P3n2/PPK1+bP2P/3P+n1P2/2G6/2S2G2L b RBGSNL4Prs2p 1";
    let pos = crate::board::position_from_sfen(sfen).expect("Invalid SFEN");
    let mut list = MoveList::new();

    generate_moves::<NonEvasionsAll>(&pos, &mut list);

    // 各駒種が持ち駒から生成する合法打ち数を検証する
    let drop_moves: Vec<_> = list.iter().filter(|m| m.is_drop()).collect();

    let rook_drops =
        drop_moves.iter().filter(|m| m.dropped_piece() == Some(PieceType::ROOK)).count();
    let bishop_drops =
        drop_moves.iter().filter(|m| m.dropped_piece() == Some(PieceType::BISHOP)).count();
    let gold_drops =
        drop_moves.iter().filter(|m| m.dropped_piece() == Some(PieceType::GOLD)).count();
    let silver_drops =
        drop_moves.iter().filter(|m| m.dropped_piece() == Some(PieceType::SILVER)).count();
    let knight_drops =
        drop_moves.iter().filter(|m| m.dropped_piece() == Some(PieceType::KNIGHT)).count();
    let lance_drops =
        drop_moves.iter().filter(|m| m.dropped_piece() == Some(PieceType::LANCE)).count();
    let pawn_drops =
        drop_moves.iter().filter(|m| m.dropped_piece() == Some(PieceType::PAWN)).count();

    assert_eq!(rook_drops, 55, "Rook should have 55 legal drops");
    assert_eq!(bishop_drops, 55, "Bishop should have 55 legal drops");
    assert_eq!(gold_drops, 55, "Gold should have 55 legal drops");
    assert_eq!(silver_drops, 55, "Silver should have 55 legal drops");
    assert_eq!(knight_drops, 43, "Knight should have 43 legal drops");
    assert_eq!(lance_drops, 50, "Lance should have 50 legal drops");
    assert_eq!(pawn_drops, 12, "Pawn should have 12 legal drops");

    // 打てない段の検証（歩・香・桂）
    let pawn_drop_moves: Vec<Move> = list
        .iter()
        .copied()
        .filter(|m| m.is_drop() && m.dropped_piece() == Some(PieceType::PAWN))
        .collect();
    let lance_drop_moves: Vec<Move> = list
        .iter()
        .copied()
        .filter(|m| m.is_drop() && m.dropped_piece() == Some(PieceType::LANCE))
        .collect();
    let knight_drop_moves: Vec<Move> = list
        .iter()
        .copied()
        .filter(|m| m.is_drop() && m.dropped_piece() == Some(PieceType::KNIGHT))
        .collect();

    // 歩と香は1段目に打てない
    for drop in &pawn_drop_moves {
        assert_ne!(drop.to_sq().rank(), Rank::RANK_1, "Pawn cannot be dropped on rank 1");
    }
    for drop in &lance_drop_moves {
        assert_ne!(drop.to_sq().rank(), Rank::RANK_1, "Lance cannot be dropped on rank 1");
    }
    // 桂は1段目と2段目に打てない
    for drop in &knight_drop_moves {
        assert_ne!(drop.to_sq().rank(), Rank::RANK_1, "Knight cannot be dropped on rank 1");
        assert_ne!(drop.to_sq().rank(), Rank::RANK_2, "Knight cannot be dropped on rank 2");
    }

    assert_eq!(pawn_drop_moves.len(), 12, "Pawn should have 12 legal drops");
    assert_eq!(lance_drop_moves.len(), 50, "Lance should have 50 legal drops");
    assert_eq!(knight_drop_moves.len(), 43, "Knight should have 43 legal drops");
}

#[test]
fn test_drop_pawn_mate_illegal() {
    // 打ち歩詰めの実戦形：逃げることも取ることもできない
    let sfen = "l1+R2+R3/6ggl/p3ppppk/2p1b4/6S2/2P+b3N1/P+p3PPP1/4G1SK1/1N3+p1N1 b Pg2sn2l4p 1";
    let pos = crate::board::position_from_sfen(sfen).expect("Invalid SFEN");
    let mut list = MoveList::new();

    generate_moves::<NonEvasionsAll>(&pos, &mut list);

    // P*1d は打ち歩詰めとなるため生成されないはず
    let illegal_drop = list.iter().copied().find(|m| {
        m.is_drop()
            && m.dropped_piece() == Some(PieceType::PAWN)
            && m.to_sq().file() == File::FILE_1
            && m.to_sq().rank() == Rank::RANK_4
    });

    assert!(illegal_drop.is_none(), "P*1d should be filtered out as drop pawn mate");
}

#[test]
fn test_drop_pawn_mate_legal_can_capture() {
    // 打った歩を取れる場合は打ち歩詰めではない
    let sfen = "l1+R2+R3/6ggl/p3ppppk/2p1b4/6Ss1/2P+b3N1/P+p3PPP1/4G1SK1/1N3+p1N1 b Pgsn2l4p 1";
    let pos = crate::board::position_from_sfen(sfen).expect("Invalid SFEN");
    let mut list = MoveList::new();

    generate_moves::<NonEvasionsAll>(&pos, &mut list);

    // P*1d は銀で取れるため合法
    let legal_drop = list.iter().copied().find(|m| {
        m.is_drop()
            && m.dropped_piece() == Some(PieceType::PAWN)
            && m.to_sq().file() == File::FILE_1
            && m.to_sq().rank() == Rank::RANK_4
    });

    assert!(legal_drop.is_some(), "P*1d should be legal because opponent can capture the pawn");
}

#[test]
fn test_pawn_move_mate_is_legal() {
    // 盤上の歩を移動しての詰みは合法（打ち歩詰めではない）
    let sfen = "l1+R2+R3/6ggl/p3ppppk/2p1b4/6S1P/2P+b3N1/P+p3PPP1/4G1SK1/1N3+p1N1 b g2sn2l4p 1";
    let pos = crate::board::position_from_sfen(sfen).expect("Invalid SFEN");
    let mut list = MoveList::new();

    generate_moves::<NonEvasionsAll>(&pos, &mut list);

    // 1e1d（歩の移動での王手）は合法手として生成されるはず
    let legal_move = list.iter().copied().find(|m| {
        !m.is_drop()
            && m.from_sq().file() == File::FILE_1
            && m.from_sq().rank() == Rank::RANK_5
            && m.to_sq().file() == File::FILE_1
            && m.to_sq().rank() == Rank::RANK_4
    });

    assert!(legal_move.is_some(), "Pawn move mate (1e1d) should be legal (not pawn drop mate)");
}

#[test]
fn test_drop_pawn_mate_legal_with_escape() {
    // 歩打ちだが玉が逃げられる場合は合法
    let sfen = "l1+R2+R1g1/6g1l/p3ppppk/2p1b4/6S2/2P+b3N1/P+p3PPP1/4G1SK1/1N3+p1N1 b Pg2sn2l4p 1";
    let pos = crate::board::position_from_sfen(sfen).expect("Invalid SFEN");
    let mut list = MoveList::new();

    generate_moves::<NonEvasionsAll>(&pos, &mut list);

    // P*1d は玉が逃げられるため合法
    let legal_drop = list.iter().copied().find(|m| {
        m.is_drop()
            && m.dropped_piece() == Some(PieceType::PAWN)
            && m.to_sq().file() == File::FILE_1
            && m.to_sq().rank() == Rank::RANK_4
    });

    assert!(legal_drop.is_some(), "P*1d should be legal because the king can escape");
}
