use crate::board::{Move32List, Position};
use crate::types::{Color, Piece, PieceType};

use super::drops::generate_drops_for_target_color;
use super::pieces::{
    generate_br_moves, generate_gold_hd_moves, generate_knight_moves, generate_lance_moves,
    generate_pawn_moves, generate_silver_moves,
};
use super::types::{Evasions, MoveGenType};
use super::{Black, ColorMarker, Move32SinkAdapter, MoveSink, White};

#[inline]
fn push_move(
    list: &mut impl MoveSink,
    from: crate::types::Square,
    to: crate::types::Square,
    piece: Piece,
) {
    list.push_normal(from, to, piece);
}

/// 王手回避の pseudo-legal 手を生成する。
///
/// 現在王手がかかっている局面で、王手を回避しうる指し手を生成する。
/// ただし legal-only ではなく、ピン外しや玉の移動先が利かれている手など
/// split legality 後段で除外される手を含むことがある。
///
/// # 生成される手
/// - 両王手の場合: 玉の移動手のみ
/// - 単王手の場合: 玉の移動手 + 王手駒を取る手 + 合駒
pub fn generate_evasions(pos: &Position, list: &mut impl MoveSink) {
    generate_evasions_for::<Evasions>(pos, list);
}

/// 王手回避の pseudo-legal 手を `Move32` リストとして生成する。
///
/// legal-only が必要な場合は [`super::generate_legal_evasions_move32`] /
/// [`super::generate_legal_evasions_all_move32`] を使用する。
#[inline]
pub fn generate_evasions_move32(pos: &Position, list: &mut Move32List) {
    let mut adapter = Move32SinkAdapter { pos, sink: list };
    generate_evasions(pos, &mut adapter);
}

#[inline]
pub fn generate_evasions_for<T: MoveGenType + 'static>(pos: &Position, list: &mut impl MoveSink) {
    match pos.turn() {
        Color::BLACK => generate_evasions_for_color::<T, Black>(pos, list),
        Color::WHITE => generate_evasions_for_color::<T, White>(pos, list),
    }
}

pub fn generate_evasions_for_color<T: MoveGenType + 'static, C: ColorMarker>(
    pos: &Position,
    list: &mut impl MoveSink,
) {
    use crate::board::attack_tables::{
        bishop_attacks, gold_attacks_unchecked, king_attacks_unchecked, knight_attacks_unchecked,
        lance_attacks, pawn_attacks_unchecked, rook_attacks, silver_attacks_unchecked,
    };

    let us = C::COLOR;
    let checkers = pos.checkers();
    if checkers.is_empty() {
        return;
    }

    let king_sq_opt = pos.bitboards().pieces_for(PieceType::KING, us).lsb();
    debug_assert!(king_sq_opt.is_some(), "side to move must have a king");
    // SAFETY: 合法局面では手番側の玉は必ず 1 枚存在する。
    let king_sq = unsafe { king_sq_opt.unwrap_unchecked() };
    let occupied = pos.bitboards().occupied();
    let occupied_without_king = occupied.and_not(crate::board::Bitboard::from_square(king_sq));
    let mut slider_attacks = crate::board::Bitboard::EMPTY;
    let mut checkers_cnt = 0;
    let mut checkers_bb = checkers;
    while let Some(check_sq) = checkers_bb.pop_lsb() {
        checkers_cnt += 1;
        let piece = pos.piece_on(check_sq);
        let color = piece.color();
        let attacks = match piece.piece_type() {
            PieceType::PAWN => pawn_attacks_unchecked(check_sq, color),
            PieceType::LANCE => lance_attacks(check_sq, occupied_without_king, color),
            PieceType::KNIGHT => knight_attacks_unchecked(check_sq, color),
            PieceType::SILVER => silver_attacks_unchecked(check_sq, color),
            PieceType::GOLD
            | PieceType::PRO_PAWN
            | PieceType::PRO_LANCE
            | PieceType::PRO_KNIGHT
            | PieceType::PRO_SILVER => gold_attacks_unchecked(check_sq, color),
            PieceType::KING => king_attacks_unchecked(check_sq),
            PieceType::BISHOP => bishop_attacks(check_sq, occupied_without_king),
            PieceType::ROOK => rook_attacks(check_sq, occupied_without_king),
            PieceType::HORSE => {
                bishop_attacks(check_sq, occupied_without_king) | king_attacks_unchecked(check_sq)
            }
            PieceType::DRAGON => {
                rook_attacks(check_sq, occupied_without_king) | king_attacks_unchecked(check_sq)
            }
            _ => crate::board::Bitboard::EMPTY,
        };
        slider_attacks = slider_attacks.or(attacks);
    }

    let our_pieces = pos.bitboards().color_pieces(us);
    let king_piece = Piece::from_parts(us, PieceType::KING);
    let mut king_targets = king_attacks_unchecked(king_sq).and_not(our_pieces | slider_attacks);
    while let Some(to) = king_targets.pop_lsb() {
        push_move(list, king_sq, to, king_piece);
    }

    if checkers_cnt > 1 {
        return;
    }

    let checker_sq_opt = checkers.lsb();
    debug_assert!(checker_sq_opt.is_some(), "single-check branch requires one checker");
    // SAFETY: `checkers_cnt == 1` かつ `checkers` 非空が保証されている。
    let checker_sq = unsafe { checker_sq_opt.unwrap_unchecked() };
    let target1 = crate::board::Bitboard::between(king_sq, checker_sq);
    let target2 = target1.or(crate::board::Bitboard::from_square(checker_sq));

    generate_pawn_moves::<T, C>(pos, list, target2);
    generate_lance_moves::<T, C>(pos, list, target2, occupied);
    generate_knight_moves::<T, C>(pos, list, target2);
    generate_silver_moves::<T, C>(pos, list, target2);
    generate_br_moves::<T, C>(pos, list, target2, occupied);
    generate_gold_hd_moves::<T, C>(pos, list, target2, occupied);
    generate_drops_for_target_color::<T, C>(pos, list, target1);
}
