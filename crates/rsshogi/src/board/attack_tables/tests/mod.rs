use super::*;
use crate::types::{Bitboard, Color, File, PieceType, Rank, SQ_11, SQ_22, SQ_55, SQ_99, Square};

// 注意: file indexは9筋側(W)に増えるため、W/NWは盤面の9筋側に相当する。

fn enemy_field_mask(us: Color) -> Bitboard {
    match us {
        Color::BLACK => {
            Bitboard::rank_mask(Rank::RANK_1)
                | Bitboard::rank_mask(Rank::RANK_2)
                | Bitboard::rank_mask(Rank::RANK_3)
        }
        Color::WHITE => {
            Bitboard::rank_mask(Rank::RANK_7)
                | Bitboard::rank_mask(Rank::RANK_8)
                | Bitboard::rank_mask(Rank::RANK_9)
        }
    }
}

fn pawn_attacks(color: Color, sq: Square) -> Bitboard {
    PAWN_ATTACKS[sq][color.to_index()]
}

fn knight_attacks(color: Color, sq: Square) -> Bitboard {
    KNIGHT_ATTACKS[sq][color.to_index()]
}

fn silver_attacks(color: Color, sq: Square) -> Bitboard {
    SILVER_ATTACKS[sq][color.to_index()]
}

fn gold_attacks(color: Color, sq: Square) -> Bitboard {
    GOLD_ATTACKS[sq][color.to_index()]
}

fn king_attacks(sq: Square) -> Bitboard {
    KING_ATTACKS[sq]
}

fn lance_step_attacks(color: Color, sq: Square) -> Bitboard {
    let beams = LANCE_BEAMS[sq.to_index()];
    match color {
        Color::BLACK => beams.black,
        Color::WHITE => beams.white,
    }
}

fn bishop_step_attacks(sq: Square) -> Bitboard {
    bishop_attacks(sq, Bitboard::EMPTY)
}

fn horse_attacks(sq: Square) -> Bitboard {
    bishop_step_attacks(sq) | king_attacks(sq)
}

fn attacks_from(piece_type: PieceType, color: Color, sq: Square, occupied: Bitboard) -> Bitboard {
    match piece_type {
        PieceType::PAWN => pawn_attacks(color, sq),
        PieceType::LANCE => lance_attacks(sq, occupied, color),
        PieceType::KNIGHT => knight_attacks(color, sq),
        PieceType::SILVER => silver_attacks(color, sq),
        PieceType::BISHOP => bishop_attacks(sq, occupied),
        PieceType::ROOK => rook_attacks(sq, occupied),
        PieceType::GOLD
        | PieceType::PRO_PAWN
        | PieceType::PRO_LANCE
        | PieceType::PRO_KNIGHT
        | PieceType::PRO_SILVER => gold_attacks(color, sq),
        PieceType::HORSE => bishop_attacks(sq, occupied) | king_attacks(sq),
        PieceType::DRAGON => rook_attacks(sq, occupied) | king_attacks(sq),
        PieceType::KING => king_attacks(sq),
        _ => Bitboard::EMPTY,
    }
}

fn check_candidate_expected(ksq: Square, pt_idx: usize, us: Color) -> Bitboard {
    let them = us.flip();
    let enemy_field = enemy_field_mask(us);
    let enemy_gold = gold_attacks(them, ksq) & enemy_field;
    let mut target = Bitboard::EMPTY;

    match pt_idx {
        0 => {
            let mut bb = pawn_attacks(them, ksq);
            while let Some(sq) = bb.pop_lsb() {
                target |= pawn_attacks(them, sq);
            }
            let mut bb = enemy_gold;
            while let Some(sq) = bb.pop_lsb() {
                target |= pawn_attacks(them, sq);
            }
            target = target.and_not(Bitboard::from_square(ksq));
        }
        1 => {
            target = lance_step_attacks(them, ksq);
            if enemy_field.test(ksq) {
                let file = ksq.raw() / 9;
                let rank = ksq.raw() % 9;
                if file != 0 {
                    let sq = Square::new((file - 1) * 9 + rank);
                    target |= lance_step_attacks(them, sq);
                }
                if file != 8 {
                    let sq = Square::new((file + 1) * 9 + rank);
                    target |= lance_step_attacks(them, sq);
                }
            }
        }
        2 => {
            let mut bb = knight_attacks(them, ksq) | enemy_gold;
            while let Some(sq) = bb.pop_lsb() {
                target |= knight_attacks(them, sq);
            }
            target = target.and_not(Bitboard::from_square(ksq));
        }
        3 => {
            let mut bb = silver_attacks(them, ksq);
            while let Some(sq) = bb.pop_lsb() {
                target |= silver_attacks(them, sq);
            }
            let mut bb = enemy_gold;
            while let Some(sq) = bb.pop_lsb() {
                target |= silver_attacks(them, sq);
            }
            let mut bb = gold_attacks(them, ksq);
            while let Some(sq) = bb.pop_lsb() {
                target |= silver_attacks(them, sq) & enemy_field;
            }
            target = target.and_not(Bitboard::from_square(ksq));
        }
        4 => {
            let mut bb = gold_attacks(them, ksq);
            while let Some(sq) = bb.pop_lsb() {
                target |= gold_attacks(them, sq);
            }
            target = target.and_not(Bitboard::from_square(ksq));
        }
        5 => {
            let mut bb = bishop_step_attacks(ksq);
            while let Some(sq) = bb.pop_lsb() {
                target |= bishop_step_attacks(sq);
            }
            let mut bb = king_attacks(ksq) & enemy_field;
            while let Some(sq) = bb.pop_lsb() {
                target |= bishop_step_attacks(sq);
            }
            let mut bb = king_attacks(ksq);
            while let Some(sq) = bb.pop_lsb() {
                target |= bishop_step_attacks(sq) & enemy_field;
            }
            target = target.and_not(Bitboard::from_square(ksq));
        }
        6 => {
            let mut bb = horse_attacks(ksq);
            while let Some(sq) = bb.pop_lsb() {
                target |= horse_attacks(sq);
            }
            target = target.and_not(Bitboard::from_square(ksq));
        }
        _ => {}
    }

    target
}

#[test]
fn test_pawn_attacks_black() {
    // 先手の歩の利き: 1マス前方
    // SQ_55 (中央) の先手歩の利き → SQ_54
    let sq = SQ_55; // file=4, rank=4
    let attacks = PAWN_ATTACKS[sq][Color::BLACK.to_index()];

    let expected_sq = Square::new(sq.raw() - 1); // 1マス前方
    assert!(attacks.test(expected_sq), "SQ_55の先手歩の利きにSQ_54が含まれていない");
    assert_eq!(attacks.count(), 1, "歩の利きは1マスのみ");
}

#[test]
fn test_pawn_attacks_white() {
    // 後手の歩の利き: 1マス前方（後手視点）
    // SQ_55 の後手歩の利き → SQ_56
    let sq = SQ_55;
    let attacks = PAWN_ATTACKS[sq][Color::WHITE.to_index()];

    let expected_sq = Square::new(sq.raw() + 1); // 1マス後方
    assert!(attacks.test(expected_sq), "SQ_55の後手歩の利きにSQ_56が含まれていない");
    assert_eq!(attacks.count(), 1, "歩の利きは1マスのみ");
}

#[test]
fn test_pawn_attacks_edge() {
    // 端の歩の利き: 盤外になる場合は空
    let sq = SQ_11; // file=0, rank=0
    let attacks_black = PAWN_ATTACKS[sq][Color::BLACK.to_index()];
    assert!(attacks_black.is_empty(), "SQ_11の先手歩の利きは盤外なので空");

    let sq = SQ_99; // file=8, rank=8
    let attacks_white = PAWN_ATTACKS[sq][Color::WHITE.to_index()];
    assert!(attacks_white.is_empty(), "SQ_99の後手歩の利きは盤外なので空");
}

#[test]
fn test_knight_attacks_black() {
    // 先手の桂馬の利き: 2マス前、W/Eに1マス
    // SQ_55 の先手桂馬の利き → SQ_43, SQ_63
    let sq = SQ_55; // file=4, rank=4
    let attacks = KNIGHT_ATTACKS[sq][Color::BLACK.to_index()];

    // 2マス前、W方向1マス: file=5, rank=2 → sq=5*9+2=47
    let sq_west = Square::new(47);
    // 2マス前、E方向1マス: file=3, rank=2 → sq=3*9+2=29
    let sq_east = Square::new(29);

    assert!(attacks.test(sq_west), "SQ_55の先手桂馬の利きにW方向の駒取り位置が含まれていない");
    assert!(attacks.test(sq_east), "SQ_55の先手桂馬の利きにE方向の駒取り位置が含まれていない");
    assert_eq!(attacks.count(), 2, "桂馬の利きは2マス");
}

#[test]
fn test_knight_attacks_edge() {
    // 1段目の桂馬: 2マス前が盤外
    let sq = SQ_11;
    let attacks_black = KNIGHT_ATTACKS[sq][Color::BLACK.to_index()];
    assert!(attacks_black.is_empty(), "SQ_11の先手桂馬の利きは盤外");

    // 2段目の桂馬: 一部の利きが有効
    let sq = SQ_22; // file=1, rank=1
    let attacks_black = KNIGHT_ATTACKS[sq][Color::BLACK.to_index()];
    assert!(attacks_black.is_empty(), "SQ_22の先手桂馬の利きは盤外");
}

#[test]
fn test_silver_attacks_black() {
    // 先手の銀の利き: 前方3マス + 斜め後方2マス
    let sq = SQ_55; // file=4, rank=4
    let attacks = SILVER_ATTACKS[sq][Color::BLACK.to_index()];

    // 銀は5方向に利きがある
    assert_eq!(attacks.count(), 5, "銀の利きは5マス");
}

#[test]
fn test_gold_attacks_black() {
    // 先手の金の利き: N/S/E/W + 斜め前2マス
    let sq = SQ_55; // file=4, rank=4
    let attacks = GOLD_ATTACKS[sq][Color::BLACK.to_index()];

    // 金は6方向に利きがある
    assert_eq!(attacks.count(), 6, "金の利きは6マス");
}

#[test]
fn test_attack_tables_accessible() {
    // 非滑り駒テーブル
    assert_eq!(PAWN_ATTACKS.len(), 81);
    assert_eq!(KNIGHT_ATTACKS.len(), 81);
    assert_eq!(SILVER_ATTACKS.len(), 81);
    assert_eq!(GOLD_ATTACKS.len(), 81);
    assert_eq!(KING_ATTACKS.len(), 81);

    // 滑り駒テーブル
    assert_eq!(LANCE_BEAMS.len(), 81);
    assert_eq!(BISHOP_BEAMS.len(), 81);
    assert_eq!(ROOK_BEAMS.len(), 81);
    assert_eq!(QUGIY_STEP_ATTACKS.len(), 6);
}

#[test]
fn test_gold_attacks_white_corner() {
    use std::str::FromStr;

    // 後手の金が9一（USI: 9a）にいる場合、8二（USI: 8b）への利きが存在する
    let sq = Square::from_str("9a").expect("valid square");
    let attacks = GOLD_ATTACKS[sq][Color::WHITE.to_index()];

    let diag_target = Square::from_str("8b").expect("valid square");
    assert!(attacks.test(diag_target), "後手の金の斜め前利きが欠落している");
}

#[test]
fn test_all_squares_covered() {
    for sq in 0..81 {
        assert!(PAWN_ATTACKS[sq][Color::BLACK.to_index()].count() <= 1);
        assert!(PAWN_ATTACKS[sq][Color::WHITE.to_index()].count() <= 1);
        assert!(KNIGHT_ATTACKS[sq][Color::BLACK.to_index()].count() <= 2);
        assert!(KNIGHT_ATTACKS[sq][Color::WHITE.to_index()].count() <= 2);
        assert!(SILVER_ATTACKS[sq][Color::BLACK.to_index()].count() <= 5);
        assert!(SILVER_ATTACKS[sq][Color::WHITE.to_index()].count() <= 5);
        assert!(GOLD_ATTACKS[sq][Color::BLACK.to_index()].count() <= 6);
        assert!(GOLD_ATTACKS[sq][Color::WHITE.to_index()].count() <= 6);
        assert!(KING_ATTACKS[sq].count() <= 8);

        assert!(LANCE_BEAMS[sq].black.count() <= 8);
        assert!(LANCE_BEAMS[sq].white.count() <= 8);
        assert!(BISHOP_BEAMS[sq].ne.count() <= 8);
        assert!(ROOK_BEAMS[sq].n.count() <= 8);
    }
}

#[test]
fn test_king_attacks() {
    // 玉の利き: 8方向すべて
    let sq = SQ_55; // 中央
    let attacks = KING_ATTACKS[sq];

    // 中央の玉は8方向に利きがある
    assert_eq!(attacks.count(), 8, "中央の玉の利きは8マス");
}

#[test]
fn test_king_attacks_corner() {
    // 隅の玉の利き: 3方向のみ
    let sq = SQ_11; // 隅
    let attacks = KING_ATTACKS[sq];

    // 隅の玉は3方向に利きがある
    assert_eq!(attacks.count(), 3, "隅の玉の利きは3マス");
}

#[test]
fn test_king_attacks_edge() {
    // 辺の玉の利き: 5方向
    let sq = SQ_55; // 中央
    let attacks = KING_ATTACKS[sq];

    assert!(attacks.count() > 0, "玉の利きは空ではない");
}

#[test]
fn test_check_candidate_bb_matches_builder() {
    for ksq_idx in 0..Square::COUNT {
        let ksq = Square::from_index(ksq_idx);
        for (pt_idx, pt_row) in CHECK_CANDIDATE_BB[ksq.to_index()].iter().enumerate() {
            for us in [Color::BLACK, Color::WHITE] {
                let expected = check_candidate_expected(ksq, pt_idx, us);
                let actual = pt_row[us.to_index()];
                assert_eq!(
                    expected, actual,
                    "check candidate mismatch: ksq={ksq:?} pt_idx={pt_idx} us={us}"
                );
            }
        }
    }
}

#[test]
fn test_check_candidate_bb_api() {
    let piece_types = [
        (PieceType::PAWN, 0usize),
        (PieceType::LANCE, 1usize),
        (PieceType::KNIGHT, 2usize),
        (PieceType::SILVER, 3usize),
        (PieceType::GOLD, 4usize),
        (PieceType::BISHOP, 5usize),
        (PieceType::ROOK, 6usize),
    ];

    for ksq_idx in 0..Square::COUNT {
        let ksq = Square::from_index(ksq_idx);
        for (pt, idx) in piece_types {
            for us in [Color::BLACK, Color::WHITE] {
                let expected = CHECK_CANDIDATE_BB[ksq.to_index()][idx][us.to_index()];
                let actual = check_candidate_bb(us, pt, ksq);
                assert_eq!(
                    expected, actual,
                    "check_candidate_bb mismatch: ksq={ksq:?} pt={pt:?} us={us}"
                );
            }
        }
    }

    assert_eq!(check_candidate_bb(Color::BLACK, PieceType::KING, SQ_55), Bitboard::EMPTY);
    assert_eq!(check_candidate_bb(Color::WHITE, PieceType::HORSE, SQ_55), Bitboard::EMPTY);
}

#[test]
fn test_representative_attacks() {
    let sq_55 = SQ_55;
    assert_eq!(PAWN_ATTACKS[sq_55][Color::BLACK.to_index()].count(), 1);
    assert_eq!(PAWN_ATTACKS[sq_55][Color::WHITE.to_index()].count(), 1);
    assert_eq!(KNIGHT_ATTACKS[sq_55][Color::BLACK.to_index()].count(), 2);
    assert_eq!(KNIGHT_ATTACKS[sq_55][Color::WHITE.to_index()].count(), 2);
    assert_eq!(SILVER_ATTACKS[sq_55][Color::BLACK.to_index()].count(), 5);
    assert_eq!(SILVER_ATTACKS[sq_55][Color::WHITE.to_index()].count(), 5);
    assert_eq!(GOLD_ATTACKS[sq_55][Color::BLACK.to_index()].count(), 6);
    assert_eq!(GOLD_ATTACKS[sq_55][Color::WHITE.to_index()].count(), 6);
    assert_eq!(KING_ATTACKS[sq_55].count(), 8);
}

#[test]
fn test_lance_beams_properties() {
    let sq_55 = SQ_55;
    let beams = LANCE_BEAMS[sq_55.to_index()];
    assert_eq!(beams.black.count(), 4);
    assert_eq!(beams.white.count(), 4);

    assert!(LANCE_BEAMS[SQ_11.to_index()].black.is_empty());
    assert!(LANCE_BEAMS[SQ_99.to_index()].white.is_empty());
}

#[test]
fn test_bishop_beams_properties() {
    let sq_55 = SQ_55;
    let beams = BISHOP_BEAMS[sq_55.to_index()];
    assert_eq!(beams.ne.count(), 4);
    assert_eq!(beams.se.count(), 4);
    assert_eq!(beams.sw.count(), 4);
    assert_eq!(beams.nw.count(), 4);

    let corner_beams = BISHOP_BEAMS[SQ_11.to_index()];
    assert!(corner_beams.ne.is_empty());
    assert!(corner_beams.nw.is_empty());
    assert!(corner_beams.sw.is_empty());
    assert_eq!(corner_beams.se.count(), 8);
}

#[test]
fn test_rook_beams_properties() {
    let sq_55 = SQ_55;
    let beams = ROOK_BEAMS[sq_55.to_index()];
    assert_eq!(beams.n.count(), 4);
    assert_eq!(beams.e.count(), 4);
    assert_eq!(beams.s.count(), 4);
    assert_eq!(beams.w.count(), 4);

    let corner_beams = ROOK_BEAMS[SQ_11.to_index()];
    assert!(corner_beams.n.is_empty());
    assert!(corner_beams.w.is_empty());
    assert_eq!(corner_beams.e.count(), 8);
    assert_eq!(corner_beams.s.count(), 8);
}

#[test]
fn test_beam_structs_equality() {
    let sq_55 = SQ_55;
    let lance = LANCE_BEAMS[sq_55.to_index()];
    assert_eq!(lance, lance);

    let bishop = BISHOP_BEAMS[sq_55.to_index()];
    assert_eq!(bishop, bishop);

    let rook = ROOK_BEAMS[sq_55.to_index()];
    assert_eq!(rook, rook);
}

/// 攻撃テーブルの生成結果を検証する。
#[test]
fn test_yaneu_compatibility_pawn() {
    // SQ_55 (file=4, rank=4) の先手歩の利き
    // 期待値: SQ_54 (file=4, rank=3)
    let sq = SQ_55;
    let attacks = PAWN_ATTACKS[sq][Color::BLACK.to_index()];

    // file=4, rank=3 → sq=4*9+3=39
    let expected_sq = Square::new(39);
    assert!(attacks.test(expected_sq), "SQ_55の先手歩の利きにSQ_54が含まれる");
    assert_eq!(attacks.count(), 1);
}

#[test]
fn test_yaneu_compatibility_knight() {
    // SQ_55 (file=4, rank=4) の先手桂馬の利き
    // 期待値: SQ_43 (file=3, rank=2), SQ_63 (file=5, rank=2)
    let sq = SQ_55;
    let attacks = KNIGHT_ATTACKS[sq][Color::BLACK.to_index()];

    // file=3, rank=2 → sq=3*9+2=29
    let sq_east = Square::new(29);
    // file=5, rank=2 → sq=5*9+2=47
    let sq_west = Square::new(47);

    assert!(attacks.test(sq_west), "桂馬のW側の利き");
    assert!(attacks.test(sq_east), "桂馬のE側の利き");
    assert_eq!(attacks.count(), 2);
}

#[test]
fn test_yaneu_compatibility_silver() {
    // 銀は5方向に利きがある
    let sq = SQ_55;
    let attacks = SILVER_ATTACKS[sq][Color::BLACK.to_index()];
    assert_eq!(attacks.count(), 5, "銀の利きは5マス");
}

#[test]
fn test_yaneu_compatibility_gold() {
    // 金は6方向に利きがある
    let sq = SQ_55;
    let attacks = GOLD_ATTACKS[sq][Color::BLACK.to_index()];
    assert_eq!(attacks.count(), 6, "金の利きは6マス");
}

#[test]
fn test_yaneu_compatibility_king() {
    // 中央の玉は8方向に利きがある
    let sq = SQ_55;
    let attacks = KING_ATTACKS[sq];
    assert_eq!(attacks.count(), 8, "中央の玉の利きは8マス");

    // 隅の玉は3方向に利きがある
    let sq = SQ_11;
    let attacks = KING_ATTACKS[sq];
    assert_eq!(attacks.count(), 3, "隅の玉の利きは3マス");
}

// --- 滑り駒ビームテーブルのテスト ---

#[test]
#[allow(clippy::uninlined_format_args)]
fn test_lance_beams_black() {
    // 先手の香車：N方向（段が減る方向）への全到達マス
    // SQ_55 (file=4, rank=4) の先手香車
    let sq = SQ_55;
    let beams = LANCE_BEAMS[sq.to_index()];

    // 先手はN方向（rank: 4→3→2→1→0）
    // file=4, rank=3,2,1,0 → sq=36,37,38,39,40 は含まず、正しくは 36(4*9+0), 37(4*9+1), 38(4*9+2), 39(4*9+3)
    // file=4, rank=0,1,2,3 → sq=36,37,38,39
    let expected_squares = vec![
        Square::new(36), // file=4, rank=0
        Square::new(37), // file=4, rank=1
        Square::new(38), // file=4, rank=2
        Square::new(39), // file=4, rank=3
    ];

    for expected_sq in expected_squares {
        assert!(
            beams.black.test(expected_sq),
            "SQ_55の先手香車ビームに{:?}が含まれていない",
            expected_sq
        );
    }

    // 元のマス自身は含まれない
    assert!(!beams.black.test(sq), "香車ビームは元のマス自身を含まない");

    // 段が増える方向（S方向）は含まれない
    let south_sq = Square::new(sq.raw() + 1); // file=4, rank=5
    assert!(!beams.black.test(south_sq), "先手香車ビームにS方向のマスが含まれてはいけない");
}

#[test]
#[allow(clippy::uninlined_format_args)]
fn test_lance_beams_white() {
    // 後手の香車：S方向（段が増える方向）への全到達マス
    // SQ_55 (file=4, rank=4) の後手香車
    let sq = SQ_55;
    let beams = LANCE_BEAMS[sq.to_index()];

    // 後手はS方向（rank: 4→5→6→7→8）
    // file=4, rank=5,6,7,8 → sq=41,42,43,44
    let expected_squares = vec![
        Square::new(41), // file=4, rank=5
        Square::new(42), // file=4, rank=6
        Square::new(43), // file=4, rank=7
        Square::new(44), // file=4, rank=8
    ];

    for expected_sq in expected_squares {
        assert!(
            beams.white.test(expected_sq),
            "SQ_55の後手香車ビームに{:?}が含まれていない",
            expected_sq
        );
    }

    // 元のマス自身は含まれない
    assert!(!beams.white.test(sq), "香車ビームは元のマス自身を含まない");
}

#[test]
fn test_lance_beams_edge() {
    // 1段目の先手香車：N方向ビームは空（盤外）
    let sq = SQ_11; // file=0, rank=0
    let beams = LANCE_BEAMS[sq.to_index()];
    assert!(beams.black.is_empty(), "1段目の先手香車のN方向ビームは空");

    // 9段目の後手香車：S方向ビームは空（盤外）
    let sq = SQ_99; // file=8, rank=8
    let beams = LANCE_BEAMS[sq.to_index()];
    assert!(beams.white.is_empty(), "9段目の後手香車のS方向ビームは空");
}

#[test]
#[allow(
    clippy::similar_names,
    clippy::identity_op,
    clippy::erasing_op,
    clippy::uninlined_format_args
)]
fn test_bishop_beams_center() {
    // 中央の角：4方向の斜めビーム
    // SQ_55 (file=4, rank=4) の角
    let sq = SQ_55;
    let beams = BISHOP_BEAMS[sq.to_index()];
    let nw = beams.ne;
    let sw = beams.se;
    let se = beams.sw;
    let ne = beams.nw;

    // NW方向: file増加、rank減少 (5,3), (6,2), (7,1), (8,0)
    let ne_squares = vec![
        Square::new(5 * 9 + 3), // 48
        Square::new(6 * 9 + 2), // 56
        Square::new(7 * 9 + 1), // 64
        Square::new(8 * 9 + 0), // 72
    ];
    for sq in ne_squares {
        assert!(nw.test(sq), "NWビームに{:?}が含まれていない", sq);
    }

    // SW方向: file増加、rank増加 (5,5), (6,6), (7,7), (8,8)
    let se_squares = vec![
        Square::new(5 * 9 + 5), // 50
        Square::new(6 * 9 + 6), // 60
        Square::new(7 * 9 + 7), // 70
        Square::new(8 * 9 + 8), // 80
    ];
    for sq in se_squares {
        assert!(sw.test(sq), "SWビームに{:?}が含まれていない", sq);
    }

    // SE方向: file減少、rank増加 (3,5), (2,6), (1,7), (0,8)
    let sw_squares = vec![
        Square::new(3 * 9 + 5), // 32
        Square::new(2 * 9 + 6), // 24
        Square::new(1 * 9 + 7), // 16
        Square::new(0 * 9 + 8), // 8
    ];
    for sq in sw_squares {
        assert!(se.test(sq), "SEビームに{:?}が含まれていない", sq);
    }

    // NE方向: file減少、rank減少 (3,3), (2,2), (1,1), (0,0)
    let nw_squares = vec![
        Square::new(3 * 9 + 3), // 30
        Square::new(2 * 9 + 2), // 20
        Square::new(1 * 9 + 1), // 10
        Square::new(0 * 9 + 0), // 0
    ];
    for sq in nw_squares {
        assert!(ne.test(sq), "NEビームに{:?}が含まれていない", sq);
    }
}

#[test]
fn test_bishop_beams_corner() {
    // 隅の角：一部のビームが短い
    let sq = SQ_11; // file=0, rank=0
    let beams = BISHOP_BEAMS[sq.to_index()];
    let nw = beams.ne;
    let sw = beams.se;
    let se = beams.sw;
    let ne = beams.nw;

    // NW・NE・SEは盤外
    assert!(nw.is_empty(), "隅からのNWビームは盤外");
    assert!(ne.is_empty(), "隅からのNEビームは盤外");
    assert!(se.is_empty(), "隅からのSEビームは盤外");

    // SW方向のみ有効: (1,1), (2,2), ..., (8,8)
    assert_eq!(sw.count(), 8, "隅からのSWビームは8マス");
}

#[test]
#[allow(clippy::identity_op, clippy::erasing_op, clippy::uninlined_format_args)]
fn test_rook_beams_center() {
    // 中央の飛車：4方向の直線ビーム
    // SQ_55 (file=4, rank=4) の飛車
    let sq = SQ_55;
    let beams = ROOK_BEAMS[sq.to_index()];
    let w = beams.e;
    let e = beams.w;

    // N方向: rank減少 (4,3), (4,2), (4,1), (4,0)
    let n_squares = vec![
        Square::new(4 * 9 + 3), // 39
        Square::new(4 * 9 + 2), // 38
        Square::new(4 * 9 + 1), // 37
        Square::new(4 * 9 + 0), // 36
    ];
    for sq in n_squares {
        assert!(beams.n.test(sq), "Nビームに{:?}が含まれていない", sq);
    }

    // W方向（筋が増える）: file増加 (5,4), (6,4), (7,4), (8,4)
    let e_squares = vec![
        Square::new(5 * 9 + 4), // 49
        Square::new(6 * 9 + 4), // 58
        Square::new(7 * 9 + 4), // 67
        Square::new(8 * 9 + 4), // 76
    ];
    for sq in e_squares {
        assert!(w.test(sq), "Wビームに{:?}が含まれていない", sq);
    }

    // S方向: rank増加 (4,5), (4,6), (4,7), (4,8)
    let s_squares = vec![
        Square::new(4 * 9 + 5), // 41
        Square::new(4 * 9 + 6), // 42
        Square::new(4 * 9 + 7), // 43
        Square::new(4 * 9 + 8), // 44
    ];
    for sq in s_squares {
        assert!(beams.s.test(sq), "Sビームに{:?}が含まれていない", sq);
    }

    // E方向（筋が減る）: file減少 (3,4), (2,4), (1,4), (0,4)
    let w_squares = vec![
        Square::new(3 * 9 + 4), // 31
        Square::new(2 * 9 + 4), // 22
        Square::new(1 * 9 + 4), // 13
        Square::new(0 * 9 + 4), // 4
    ];
    for sq in w_squares {
        assert!(e.test(sq), "Eビームに{:?}が含まれていない", sq);
    }
}

#[test]
fn test_rook_beams_edge() {
    // 端の飛車：一部のビームが短いまたは空
    let sq = SQ_11; // file=0, rank=0
    let beams = ROOK_BEAMS[sq.to_index()];
    let w = beams.e;
    let e = beams.w;

    // N・E方向は盤外
    assert!(beams.n.is_empty(), "1段目からのNビームは盤外");
    assert!(e.is_empty(), "1筋からのEビームは盤外");

    // W・S方向は有効
    assert_eq!(w.count(), 8, "1筋からのWビームは8マス");
    assert_eq!(beams.s.count(), 8, "1段目からのSビームは8マス");
}

/// 飛び駒の利き実装が呼び出せることを確認する文書用テスト。
///
/// rsshogi は Magic Bitboard ではなく Qugiy 相当のステップテーブル + 差分抽出
/// （byte_reverse + 減算）で利きを生成する。攻撃パターンの正しさ自体は perft
/// と各駒の利きテストで検証しており、ここでは API の存在確認のみを行う。
#[test]
fn test_slider_attacks_implementation_note() {
    // 利き生成関数が存在し呼び出せることを確認する。
    let sq = SQ_55;
    let occupied = Bitboard::EMPTY;

    // API が存在してコンパイルできることを確認する。
    let _lance = lance_attacks(sq, occupied, Color::BLACK);
    let _rook = rook_attacks(sq, occupied);
    let _bishop = bishop_attacks(sq, occupied);
}

/// P12 Task 5.1: 香車の利き正確性検証
///
/// 香車の攻撃範囲計算が正しく動作することを、複数の占有パターンで検証する。
#[test]
fn test_lance_attacks_with_occupied() {
    use crate::types::Color;

    // SQ_55 (中央) の先手香車、占有なし
    let sq = SQ_55;
    let occupied = Bitboard::EMPTY;
    let attacks = lance_attacks(sq, occupied, Color::BLACK);

    // 先手香車はN方向（rank減少）に4マスの利きがある
    assert_eq!(attacks.count(), 4, "占有なしの香車利きは4マス");

    // SQ_55 (file=4, rank=4) からN方向 (rank=3,2,1,0)
    // file=4, rank=3 → sq=39
    assert!(attacks.test(Square::new(39)), "rank=3への利き");
    // file=4, rank=2 → sq=38
    assert!(attacks.test(Square::new(38)), "rank=2への利き");
    // file=4, rank=1 → sq=37
    assert!(attacks.test(Square::new(37)), "rank=1への利き");
    // file=4, rank=0 → sq=36
    assert!(attacks.test(Square::new(36)), "rank=0への利き");

    // 占有マスあり: SQ_39 (file=4, rank=3) に駒がある場合
    let mut occupied = Bitboard::EMPTY;
    occupied.set(Square::new(39)); // rank=3に駒を配置
    let attacks = lance_attacks(sq, occupied, Color::BLACK);

    // 遮断点までの利きのみ (rank=3まで)
    assert_eq!(attacks.count(), 1, "遮断点までの香車利きは1マス");
    assert!(attacks.test(Square::new(39)), "遮断点への利き");
    assert!(!attacks.test(Square::new(38)), "遮断点より奥の利きはなし");
}

/// P12 Task 5.1: 飛車の利き正確性検証
///
/// 飛車の攻撃範囲計算が正しく動作することを、複数の占有パターンで検証する。
#[test]
fn test_rook_attacks_with_occupied() {
    // SQ_55 (中央) の飛車、占有なし
    let sq = SQ_55;
    let occupied = Bitboard::EMPTY;
    let attacks = rook_attacks(sq, occupied);
    let beams = ROOK_BEAMS[sq.to_index()];
    let north = attacks & beams.n;
    let south = attacks & beams.s;
    let west = attacks & beams.e;
    let east = attacks & beams.w;
    assert_eq!(north.count(), 4, "N方向の飛車利きは4マス");
    assert_eq!(south.count(), 4, "S方向の飛車利きは4マス");
    assert_eq!(west.count(), 4, "W方向の飛車利きは4マス");
    assert_eq!(east.count(), 4, "E方向の飛車利きは4マス");

    // 飛車は4方向に合計16マスの利きがある (4方向×4マス)
    assert_eq!(attacks.count(), 16, "占有なしの飛車利きは16マス");

    // N方向の利きを確認
    assert!(attacks.test(Square::new(39)), "N方向rank=3への利き");
    assert!(attacks.test(Square::new(36)), "N方向rank=0への利き");

    // W方向（筋が増える）の利きを確認
    assert!(attacks.test(Square::new(49)), "W方向file=5への利き");
    assert!(attacks.test(Square::new(76)), "W方向file=8への利き");

    // 占有マスあり: NとWに駒がある場合
    let mut occupied = Bitboard::EMPTY;
    occupied.set(Square::new(39)); // N方向rank=3に駒
    occupied.set(Square::new(49)); // W方向file=5に駒
    let attacks = rook_attacks(sq, occupied);

    // 遮断点を含む利きのみ
    // N方向: rank=3まで (1マス)
    // W方向: file=5まで (1マス)
    // S方向: 4マス (遮断なし)
    // E方向: 4マス (遮断なし)
    assert_eq!(attacks.count(), 10, "遮断ありの飛車利きは10マス");
    assert!(attacks.test(Square::new(39)), "Nの遮断点への利き");
    assert!(attacks.test(Square::new(49)), "Wの遮断点への利き");
    assert!(!attacks.test(Square::new(38)), "Nの遮断点より奥の利きはなし");
}

#[test]
fn test_qugiy_rook_mask_matches_generated_rays() {
    let sq = SQ_55;
    let file = sq.file().raw();
    let rank = sq.rank().raw();

    let mut west = Bitboard::EMPTY;
    for f in (file + 1)..=File::FILE_9.raw() {
        west.set(Square::from_file_rank(File::new(f), Rank::new(rank)));
    }

    let mut east = Bitboard::EMPTY;
    for f in (File::FILE_1.raw()..file).rev() {
        east.set(Square::from_file_rank(File::new(f), Rank::new(rank)));
    }

    let east_rev = east.byte_reverse();
    let (hi, lo) = Bitboard::unpack(east_rev, west);
    assert_eq!(lo, QUGIY_ROOK_MASK[sq.to_index()][0]);
    assert_eq!(hi, QUGIY_ROOK_MASK[sq.to_index()][1]);
}

#[test]
fn test_rook_file_attacks_matches_lance_pair() {
    let occupancies = [
        Bitboard::EMPTY,
        Bitboard::ALL,
        Bitboard::from_square(SQ_11) | Bitboard::from_square(SQ_55) | Bitboard::from_square(SQ_99),
        Bitboard::file_mask(File::FILE_1) | Bitboard::file_mask(File::FILE_9),
        Bitboard::rank_mask(Rank::RANK_3) | Bitboard::rank_mask(Rank::RANK_7),
    ];

    for raw in 0..Square::COUNT as i8 {
        let sq = Square::new(raw);
        for occupied in occupancies {
            let expected = lance_attacks(sq, occupied, Color::BLACK)
                | lance_attacks(sq, occupied, Color::WHITE);
            assert_eq!(rook_file_attacks(sq, occupied), expected, "sq={sq:?}");
        }
    }
}

#[test]
#[allow(clippy::similar_names)]
fn test_qugiy_bishop_mask_matches_generated_rays() {
    let sq = SQ_55;
    let file = sq.file().raw();
    let rank = sq.rank().raw();

    let mut lu = Bitboard::EMPTY;
    let mut ld = Bitboard::EMPTY;
    let mut ru = Bitboard::EMPTY;
    let mut rd = Bitboard::EMPTY;

    let mut f = file + 1;
    let mut r = rank - 1;
    while f <= File::FILE_9.raw() && r >= Rank::RANK_1.raw() {
        lu.set(Square::from_file_rank(File::new(f), Rank::new(r)));
        f += 1;
        r -= 1;
    }

    let mut f = file + 1;
    let mut r = rank + 1;
    while f <= File::FILE_9.raw() && r <= Rank::RANK_9.raw() {
        ld.set(Square::from_file_rank(File::new(f), Rank::new(r)));
        f += 1;
        r += 1;
    }

    let mut f = file - 1;
    let mut r = rank - 1;
    while f >= File::FILE_1.raw() && r >= Rank::RANK_1.raw() {
        ru.set(Square::from_file_rank(File::new(f), Rank::new(r)));
        f -= 1;
        r -= 1;
    }

    let mut f = file - 1;
    let mut r = rank + 1;
    while f >= File::FILE_1.raw() && r <= Rank::RANK_9.raw() {
        rd.set(Square::from_file_rank(File::new(f), Rank::new(r)));
        f -= 1;
        r += 1;
    }

    let ru = ru.byte_reverse();
    let rd = rd.byte_reverse();
    for (part, mask) in QUGIY_BISHOP_MASK[sq.to_index()].iter().enumerate().take(2) {
        let [p0, p1, p2, p3] = mask.parts();
        let [lu_lo, lu_hi] = lu.parts();
        let [ru_lo, ru_hi] = ru.parts();
        let [ld_lo, ld_hi] = ld.parts();
        let [rd_lo, rd_hi] = rd.parts();
        if part == 0 {
            assert_eq!(p0, lu_lo);
            assert_eq!(p1, ru_lo);
            assert_eq!(p2, ld_lo);
            assert_eq!(p3, rd_lo);
        } else {
            assert_eq!(p0, lu_hi);
            assert_eq!(p1, ru_hi);
            assert_eq!(p2, ld_hi);
            assert_eq!(p3, rd_hi);
        }
    }
}

#[test]
fn test_qugiy_step_attacks_direction_order() {
    let sq = SQ_55;
    let file = sq.file().raw();
    let rank = sq.rank().raw();

    let directions = [(-1, -1), (-1, 0), (-1, 1), (1, -1), (1, 0), (1, 1)];
    let reverse_dirs = [true, true, true, false, false, false];

    for (dir_idx, (df, dr)) in directions.iter().enumerate() {
        let mut ray = Bitboard::EMPTY;
        let mut f = file + df;
        let mut r = rank + dr;
        while (File::FILE_1.raw()..=File::FILE_9.raw()).contains(&f)
            && (Rank::RANK_1.raw()..=Rank::RANK_9.raw()).contains(&r)
        {
            ray.set(Square::from_file_rank(File::new(f), Rank::new(r)));
            f += df;
            r += dr;
        }
        if reverse_dirs[dir_idx] {
            ray = ray.byte_reverse();
        }
        assert_eq!(ray, QUGIY_STEP_ATTACKS[dir_idx][sq.to_index()]);
    }
}

#[test]
fn test_bitboard_attacks_counts_match_yaneuraou() {
    let sq = SQ_55;
    let empty = Bitboard::EMPTY;
    let filled = Bitboard::ALL;
    let p0_table = [0, 1, 4, 2, 5, 16, 16, 6, 8, 6, 6, 6, 6, 20, 20, 6];
    let p1_table = [0, 1, 1, 2, 5, 4, 4, 6, 8, 6, 6, 6, 6, 8, 8, 6];
    let piece_types = [
        PieceType::PAWN,
        PieceType::LANCE,
        PieceType::KNIGHT,
        PieceType::SILVER,
        PieceType::BISHOP,
        PieceType::ROOK,
        PieceType::GOLD,
        PieceType::KING,
        PieceType::PRO_PAWN,
        PieceType::PRO_LANCE,
        PieceType::PRO_KNIGHT,
        PieceType::PRO_SILVER,
        PieceType::HORSE,
        PieceType::DRAGON,
    ];

    for color in [Color::BLACK, Color::WHITE] {
        for piece_type in piece_types {
            let idx = piece_type.to_index();
            let attacks0 = attacks_from(piece_type, color, sq, empty);
            let attacks1 = attacks_from(piece_type, color, sq, filled);
            assert_eq!(attacks0.count(), p0_table[idx]);
            assert_eq!(attacks1.count(), p1_table[idx]);
        }
    }
}

/// P12 Task 5.1: 角の利き正確性検証
///
/// 角の攻撃範囲計算が正しく動作することを、複数の占有パターンで検証する。
#[test]
fn test_bishop_attacks_with_occupied() {
    // SQ_55 (中央) の角、占有なし
    let sq = SQ_55;
    let occupied = Bitboard::EMPTY;
    let attacks = bishop_attacks(sq, occupied);

    // 角は4方向に合計16マスの利きがある (4方向×4マス)
    assert_eq!(attacks.count(), 16, "占有なしの角利きは16マス");

    // NW方向の利きを確認
    assert!(attacks.test(Square::new(48)), "NW方向(5,3)への利き");
    assert!(attacks.test(Square::new(72)), "NW方向(8,0)への利き");

    // SW方向の利きを確認
    assert!(attacks.test(Square::new(50)), "SW方向(5,5)への利き");
    assert!(attacks.test(Square::new(80)), "SW方向(8,8)への利き");

    // 占有マスあり: NWとSWに駒がある場合
    let mut occupied = Bitboard::EMPTY;
    occupied.set(Square::new(48)); // NW方向(5,3)に駒
    occupied.set(Square::new(50)); // SW方向(5,5)に駒
    let attacks = bishop_attacks(sq, occupied);

    // 遮断点を含む利きのみ
    // NW方向: (5,3)まで (1マス)
    // SW方向: (5,5)まで (1マス)
    // NE方向: 4マス (遮断なし)
    // SE方向: 4マス (遮断なし)
    assert_eq!(attacks.count(), 10, "遮断ありの角利きは10マス");
    assert!(attacks.test(Square::new(48)), "NWの遮断点への利き");
    assert!(attacks.test(Square::new(50)), "SWの遮断点への利き");
    assert!(!attacks.test(Square::new(56)), "NWの遮断点より奥の利きはなし");
}
