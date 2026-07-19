use crate::board::{Bitboard, Position};
use crate::types::{Piece, PieceType, Rank, SQ_D, SQ_U, Square};

use super::types::MoveGenType;
use super::{ColorMarker, MoveSink};

#[inline]
fn push_move(list: &mut impl MoveSink, from: Square, to: Square, piece: Piece) {
    list.push_normal(from, to, piece);
}

#[inline]
fn push_promote(list: &mut impl MoveSink, from: Square, to: Square, piece: Piece) {
    list.push_promotion(from, to, piece);
}

const GOLD_LIKE_TYPES: [PieceType; 5] = [
    PieceType::GOLD,
    PieceType::PRO_PAWN,
    PieceType::PRO_LANCE,
    PieceType::PRO_KNIGHT,
    PieceType::PRO_SILVER,
];

/// 歩の移動手を生成
fn pawn_bb_attacks<C: ColorMarker>(pawns: Bitboard) -> Bitboard {
    // 参照実装の pawnBbEffect 相当:
    // 1段目/9段目の歩を事前に落としてからビットシフトすることで筋またぎを防ぐ。
    let movable = if C::IS_BLACK {
        pawns.and_not(Bitboard::rank_mask(Rank::RANK_1))
    } else {
        pawns.and_not(Bitboard::rank_mask(Rank::RANK_9))
    };
    let shifted = if C::IS_BLACK { movable.packed_bits() >> 1 } else { movable.packed_bits() << 1 };
    Bitboard::from_packed_bits(shifted)
}

/// 歩の移動手を生成
#[allow(clippy::extra_unused_type_parameters)]
pub(super) fn generate_pawn_moves<T: MoveGenType, C: ColorMarker>(
    pos: &Position,
    list: &mut impl MoveSink,
    target: Bitboard,
) {
    let bb = pos.bitboards();
    let us = C::COLOR;
    let allow_underpromote = T::is_generate_all_legal();
    let pawns = bb.pieces_for(PieceType::PAWN, us);
    let mut destinations = pawn_bb_attacks::<C>(pawns).and(target);
    let piece = Piece::from_parts(us, PieceType::PAWN);

    if C::IS_BLACK {
        while let Some(to) = destinations.pop_lsb() {
            let from = Square::new(to.raw() + SQ_D);
            let to_rank = to.rank().raw();
            if to_rank == 0 {
                push_promote(list, from, to, piece);
                continue;
            }

            let can_promote = to_rank <= 2;
            if can_promote {
                push_promote(list, from, to, piece);
            }
            if allow_underpromote || !can_promote {
                push_move(list, from, to, piece);
            }
        }
    } else {
        while let Some(to) = destinations.pop_lsb() {
            let from = Square::new(to.raw() + SQ_U);
            let to_rank = to.rank().raw();
            if to_rank == 8 {
                push_promote(list, from, to, piece);
                continue;
            }

            let can_promote = to_rank >= 6;
            if can_promote {
                push_promote(list, from, to, piece);
            }
            if allow_underpromote || !can_promote {
                push_move(list, from, to, piece);
            }
        }
    }
}

/// 桂馬の移動手を生成
#[allow(clippy::extra_unused_type_parameters)]
pub(super) fn generate_knight_moves<T: MoveGenType, C: ColorMarker>(
    pos: &Position,
    list: &mut impl MoveSink,
    target: Bitboard,
) {
    use crate::board::attack_tables::knight_attacks_unchecked;

    let us = C::COLOR;
    let bb = pos.bitboards();
    let mut knights = bb.pieces_for(PieceType::KNIGHT, us);
    let piece = Piece::from_parts(us, PieceType::KNIGHT);

    while let Some(from) = knights.pop_lsb() {
        let from_rank = from.rank().raw();
        let attacks = knight_attacks_unchecked(from, us);
        let mut destinations = attacks.and(target);

        if C::IS_BLACK {
            while let Some(to) = destinations.pop_lsb() {
                let to_rank = to.rank().raw();
                if to_rank <= 1 {
                    push_promote(list, from, to, piece);
                    continue;
                }

                if from_rank <= 2 || to_rank <= 2 {
                    push_promote(list, from, to, piece);
                }
                push_move(list, from, to, piece);
            }
        } else {
            while let Some(to) = destinations.pop_lsb() {
                let to_rank = to.rank().raw();
                if to_rank >= 7 {
                    push_promote(list, from, to, piece);
                    continue;
                }

                if from_rank >= 6 || to_rank >= 6 {
                    push_promote(list, from, to, piece);
                }
                push_move(list, from, to, piece);
            }
        }
    }
}

/// 銀の移動手を生成
#[allow(clippy::extra_unused_type_parameters)]
pub(super) fn generate_silver_moves<T: MoveGenType, C: ColorMarker>(
    pos: &Position,
    list: &mut impl MoveSink,
    target: Bitboard,
) {
    use crate::board::attack_tables::silver_attacks_unchecked;

    let us = C::COLOR;
    let bb = pos.bitboards();
    let mut silvers = bb.pieces_for(PieceType::SILVER, us);
    let piece = Piece::from_parts(us, PieceType::SILVER);
    let enemy_territory = Bitboard::promotion_zone(us);

    while let Some(from) = silvers.pop_lsb() {
        let attacks = silver_attacks_unchecked(from, us);
        let destinations = attacks.and(target);
        if enemy_territory.test(from) {
            list.push_promotion_then_normals_with_piece(from, piece, destinations, true);
            continue;
        }

        let promotions = destinations.and(enemy_territory);
        list.push_promotion_then_normals_with_piece(from, piece, promotions, true);

        let non_promotions = destinations.and_not(enemy_territory);
        list.push_normals_with_piece(from, piece, non_promotions);
    }
}

/// 金相当 + 馬/龍 + 玉の移動手を生成
#[allow(clippy::extra_unused_type_parameters)]
pub(super) fn generate_gold_hdk_moves<T: MoveGenType, C: ColorMarker>(
    pos: &Position,
    list: &mut impl MoveSink,
    target: Bitboard,
    occupied: Bitboard,
) {
    use crate::board::attack_tables::{
        bishop_attacks, gold_attacks_unchecked, king_attacks_unchecked, rook_attacks,
    };

    let us = C::COLOR;
    let bb = pos.bitboards();
    let mut gold_like = Bitboard::EMPTY;
    for piece_type in GOLD_LIKE_TYPES {
        gold_like |= bb.pieces_for(piece_type, us);
    }
    // 参照実装の GPM_GHDK と同じく、金相当 + 王 + 馬 + 龍を同一Bitboardで走査する。
    let mut pieces = gold_like
        | bb.pieces_for(PieceType::KING, us)
        | bb.pieces_for(PieceType::HORSE, us)
        | bb.pieces_for(PieceType::DRAGON, us);
    while let Some(from) = pieces.pop_lsb() {
        // SAFETY: `pieces` は盤上駒 bitboard の反復であり、`from` は常に盤内。
        let piece = unsafe { pos.piece_on_unchecked(from) };
        let attacks = match piece.piece_type() {
            PieceType::GOLD
            | PieceType::PRO_PAWN
            | PieceType::PRO_LANCE
            | PieceType::PRO_KNIGHT
            | PieceType::PRO_SILVER => gold_attacks_unchecked(from, us),
            PieceType::KING => king_attacks_unchecked(from),
            PieceType::HORSE => bishop_attacks(from, occupied) | king_attacks_unchecked(from),
            PieceType::DRAGON => rook_attacks(from, occupied) | king_attacks_unchecked(from),
            _ => Bitboard::EMPTY,
        };
        list.push_normals_with_piece(from, piece, attacks.and(target));
    }
}

/// 金相当 + 馬/龍の移動手を生成（玉は除外）
#[allow(clippy::extra_unused_type_parameters)]
pub(super) fn generate_gold_hd_moves<T: MoveGenType, C: ColorMarker>(
    pos: &Position,
    list: &mut impl MoveSink,
    target: Bitboard,
    occupied: Bitboard,
) {
    use crate::board::attack_tables::{
        bishop_attacks, gold_attacks_unchecked, king_attacks_unchecked, rook_attacks,
    };

    let us = C::COLOR;
    let bb = pos.bitboards();
    let mut gold_like = Bitboard::EMPTY;
    for piece_type in GOLD_LIKE_TYPES {
        gold_like |= bb.pieces_for(piece_type, us);
    }
    // 参照実装の GPM_GHD と同じく、金相当 + 馬 + 龍を同一Bitboardで走査する。
    let mut pieces =
        gold_like | bb.pieces_for(PieceType::HORSE, us) | bb.pieces_for(PieceType::DRAGON, us);
    while let Some(from) = pieces.pop_lsb() {
        // SAFETY: `pieces` は盤上駒 bitboard の反復であり、`from` は常に盤内。
        let piece = unsafe { pos.piece_on_unchecked(from) };
        let attacks = match piece.piece_type() {
            PieceType::GOLD
            | PieceType::PRO_PAWN
            | PieceType::PRO_LANCE
            | PieceType::PRO_KNIGHT
            | PieceType::PRO_SILVER => gold_attacks_unchecked(from, us),
            PieceType::HORSE => bishop_attacks(from, occupied) | king_attacks_unchecked(from),
            PieceType::DRAGON => rook_attacks(from, occupied) | king_attacks_unchecked(from),
            _ => Bitboard::EMPTY,
        };
        list.push_normals_with_piece(from, piece, attacks.and(target));
    }
}

/// 香車の移動手を生成
#[allow(clippy::extra_unused_type_parameters)]
pub(super) fn generate_lance_moves<T: MoveGenType, C: ColorMarker>(
    pos: &Position,
    list: &mut impl MoveSink,
    target: Bitboard,
    occupied: Bitboard,
) {
    use crate::board::attack_tables::lance_attacks;
    use crate::types::Rank;

    let us = C::COLOR;
    let bb = pos.bitboards();
    let mut lances = bb.pieces_for(PieceType::LANCE, us);
    let allow_underpromote = T::is_generate_all_legal();
    let enemy_territory = Bitboard::promotion_zone(us);
    let non_promote_mask = if allow_underpromote {
        if C::IS_BLACK {
            Bitboard::ALL.and_not(Bitboard::rank_mask(Rank::RANK_1))
        } else {
            Bitboard::ALL.and_not(Bitboard::rank_mask(Rank::RANK_9))
        }
    } else if C::IS_BLACK {
        Bitboard::ALL
            .and_not(Bitboard::rank_mask(Rank::RANK_1))
            .and_not(Bitboard::rank_mask(Rank::RANK_2))
    } else {
        Bitboard::ALL
            .and_not(Bitboard::rank_mask(Rank::RANK_8))
            .and_not(Bitboard::rank_mask(Rank::RANK_9))
    };

    let piece = Piece::from_parts(us, PieceType::LANCE);

    while let Some(from) = lances.pop_lsb() {
        let attacks = lance_attacks(from, occupied, us);
        let destinations = attacks.and(target);

        let promotions = destinations.and(enemy_territory);
        list.push_promotions_with_piece(from, piece, promotions);

        let non_promotions = destinations.and(non_promote_mask);
        list.push_normals_with_piece(from, piece, non_promotions);
    }
}

/// 角と飛車の移動手を生成
#[allow(clippy::extra_unused_type_parameters)]
pub(super) fn generate_br_moves<T: MoveGenType + 'static, C: ColorMarker>(
    pos: &Position,
    list: &mut impl MoveSink,
    target: Bitboard,
    occupied: Bitboard,
) {
    use crate::board::attack_tables::{bishop_attacks, rook_attacks};

    let us = C::COLOR;
    let bb = pos.bitboards();
    let allow_underpromote = T::is_generate_all_legal();
    let enemy_territory = Bitboard::promotion_zone(us);
    // 参照実装の GPM_BR と同じく、角+飛を同一Bitboardで走査する。
    let mut pieces = bb.pieces_for(PieceType::BISHOP, us) | bb.pieces_for(PieceType::ROOK, us);
    while let Some(from) = pieces.pop_lsb() {
        // SAFETY: `pieces` は盤上駒 bitboard の反復であり、`from` は常に盤内。
        let piece = unsafe { pos.piece_on_unchecked(from) };
        let attacks = match piece.piece_type() {
            PieceType::BISHOP => bishop_attacks(from, occupied),
            PieceType::ROOK => rook_attacks(from, occupied),
            _ => Bitboard::EMPTY,
        };
        let destinations = attacks.and(target);
        let from_in_enemy = enemy_territory.test(from);

        if from_in_enemy {
            list.push_promotion_then_normals_with_piece(
                from,
                piece,
                destinations,
                allow_underpromote,
            );
            continue;
        }

        let promotions = destinations.and(enemy_territory);
        list.push_promotion_then_normals_with_piece(from, piece, promotions, allow_underpromote);

        let non_promotions = destinations.and_not(enemy_territory);
        list.push_normals_with_piece(from, piece, non_promotions);
    }
}
