use crate::board::attack_tables::{
    GOLD_ATTACKS, KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS, SILVER_ATTACKS, bishop_attacks,
    lance_attacks, rook_attacks,
};
use crate::board::{Bitboard, Position};
use crate::types::{Color, Move, Piece, PieceType, Rank, Square};

use super::promotions::{can_promote_move, must_promote};
use super::{MoveGenType, MoveSink};

#[derive(Clone, Copy)]
struct GenerationFacts {
    us: crate::types::Color,
    king_sq: Square,
    pinned: Bitboard,
    non_king_evasion_targets: Bitboard,
    in_check: bool,
}

#[inline(always)]
fn mode_accepts<T: MoveGenType>(
    pos: &Position,
    from: Square,
    to: Square,
    piece: Piece,
    is_capture: bool,
    promotion: bool,
) -> bool {
    if T::IS_RECAPTURES {
        return true;
    }
    if T::IS_CHECKS || T::QUIET_CHECKS {
        if T::QUIET_CHECKS && is_capture {
            return false;
        }
        let mv = if promotion { Move::promotion(from, to) } else { Move::normal(from, to) };
        return pos.gives_check_move(mv);
    }
    if T::IS_CAPTURE_PLUS_PRO {
        return is_capture
            || (piece.piece_type() == PieceType::PAWN
                && (promotion
                    || T::GENERATE_ALL_LEGAL && can_promote_move(from, to, piece.color())));
    }
    if T::IS_QUIETS_PRO_MINUS && promotion && piece.piece_type() == PieceType::PAWN {
        return false;
    }
    (is_capture && T::CAPTURES) || (!is_capture && T::QUIETS)
}

#[inline(always)]
fn legal_board_move(
    pos: &Position,
    from: Square,
    to: Square,
    piece: Piece,
    facts: GenerationFacts,
) -> bool {
    if piece.piece_type() == PieceType::KING {
        return !pos.is_attacked_by_color_with_king(facts.us.flip(), to, from);
    }
    if facts.in_check && !facts.non_king_evasion_targets.test(to) {
        return false;
    }
    !facts.pinned.test(from) || Bitboard::is_aligned(from, to, facts.king_sq)
}

#[inline(always)]
fn pseudo_ok<T: MoveGenType>(
    pos: &Position,
    from: Square,
    to: Square,
    piece: Piece,
    promotion: bool,
    facts: GenerationFacts,
) -> bool {
    if T::IS_LEGAL {
        return legal_board_move(pos, from, to, piece, facts);
    }
    if !T::EVASIONS && !T::IS_CHECKS && !T::QUIET_CHECKS && !facts.in_check {
        return true;
    }
    let mv = if promotion { Move::promotion(from, to) } else { Move::normal(from, to) };
    pos.is_pseudo_legal_move(mv, T::GENERATE_ALL_LEGAL)
}

fn king_evasion_leaves_checker_attack(pos: &Position, from: Square, to: Square) -> bool {
    let occupied = pos.bitboards().occupied().and_not(Bitboard::from_square(from));
    let mut checkers = pos.checkers();
    while let Some(checker) = checkers.pop_lsb() {
        let piece = pos.piece_on(checker);
        if Position::piece_attacks(piece.piece_type(), checker, piece.color(), occupied).test(to) {
            return true;
        }
    }
    false
}

#[inline(always)]
fn emit_variants<T: MoveGenType>(
    pos: &Position,
    from: Square,
    to: Square,
    piece: Piece,
    is_capture: bool,
    facts: GenerationFacts,
    list: &mut impl MoveSink,
) {
    let us = facts.us;
    let pt = piece.piece_type();
    // Pseudo evasions retain moves rejected by later pin/king-safety checks.
    // A king destination still attacked by the checking piece itself does not
    // evade at all, so exclude it before preserving those later distinctions.
    if T::EVASIONS && pt == PieceType::KING && king_evasion_leaves_checker_attack(pos, from, to) {
        return;
    }
    let promotable = matches!(
        pt,
        PieceType::PAWN
            | PieceType::LANCE
            | PieceType::KNIGHT
            | PieceType::SILVER
            | PieceType::BISHOP
            | PieceType::ROOK
    ) && can_promote_move(from, to, us);

    let legal_all_variants = T::IS_LEGAL
        && T::CAPTURES
        && T::QUIETS
        && T::GENERATE_ALL_LEGAL
        && !T::EVASIONS
        && !T::IS_CHECKS
        && !T::QUIET_CHECKS
        && !T::IS_CAPTURE_PLUS_PRO
        && !T::IS_RECAPTURES;
    if legal_all_variants {
        if !legal_board_move(pos, from, to, piece, facts) {
            return;
        }
        if promotable {
            list.push_promotion(from, to, piece);
        }
        if !must_promote(pt, to, us) {
            list.push_normal(from, to, piece);
        }
        return;
    }

    if promotable
        && mode_accepts::<T>(pos, from, to, piece, is_capture, true)
        && pseudo_ok::<T>(pos, from, to, piece, true, facts)
    {
        list.push_promotion(from, to, piece);
    }

    if must_promote(pt, to, us) {
        return;
    }

    let lance_second_rank = pt == PieceType::LANCE
        && ((us == Color::BLACK && to.rank() == Rank::RANK_2)
            || (us == Color::WHITE && to.rank() == Rank::RANK_8));
    let prefer_promotion = promotable
        && !T::GENERATE_ALL_LEGAL
        && (matches!(pt, PieceType::PAWN | PieceType::BISHOP | PieceType::ROOK)
            || lance_second_rank);
    if prefer_promotion {
        return;
    }

    if mode_accepts::<T>(pos, from, to, piece, is_capture, false)
        && pseudo_ok::<T>(pos, from, to, piece, false, facts)
    {
        list.push_normal(from, to, piece);
    }
}

#[inline(always)]
fn generate_destinations<T: MoveGenType>(
    pos: &Position,
    from: Square,
    piece: Piece,
    mut targets: Bitboard,
    enemy: Bitboard,
    facts: GenerationFacts,
    list: &mut impl MoveSink,
) {
    if T::QUIET_CHECKS || (!T::CAPTURES && T::QUIETS && !T::IS_CHECKS) {
        targets = targets.and_not(enemy);
    } else if T::CAPTURES && !T::QUIETS && !T::IS_CAPTURE_PLUS_PRO && !T::IS_RECAPTURES {
        targets &= enemy;
    }
    while let Some(to) = targets.pop_lsb() {
        emit_variants::<T>(pos, from, to, piece, enemy.test(to), facts, list);
    }
}

#[inline]
fn generate_board_moves_with_facts<T: MoveGenType, const APPLY_EVASION_MASK: bool>(
    pos: &Position,
    only_to: Option<Square>,
    list: &mut impl MoveSink,
    facts: GenerationFacts,
) {
    let us = facts.us;
    let occupied = pos.bitboards().occupied();
    let ours = pos.bitboards().color_pieces(us);
    let enemy = occupied.and_not(ours);
    let destination_mask = only_to.map_or(Bitboard::ALL, Bitboard::from_square).and_not(ours);

    macro_rules! generate_piece_type {
        ($piece_type:expr, |$from:ident| $attacks:expr) => {{
            if list.stop() {
                return;
            }
            let piece = Piece::from_parts(us, $piece_type);
            let mut sources = pos.bitboards().pieces_for($piece_type, us);
            while let Some($from) = sources.pop_lsb() {
                let mut targets = ($attacks) & destination_mask;
                if APPLY_EVASION_MASK && $piece_type != PieceType::KING {
                    targets &= facts.non_king_evasion_targets;
                }
                generate_destinations::<T>(pos, $from, piece, targets, enemy, facts, list);
                if list.stop() {
                    return;
                }
            }
        }};
    }

    generate_piece_type!(PieceType::PAWN, |from| PAWN_ATTACKS[from][us.to_index()]);
    generate_piece_type!(PieceType::LANCE, |from| lance_attacks(from, occupied, us));
    generate_piece_type!(PieceType::KNIGHT, |from| KNIGHT_ATTACKS[from][us.to_index()]);
    generate_piece_type!(PieceType::SILVER, |from| SILVER_ATTACKS[from][us.to_index()]);
    generate_piece_type!(PieceType::BISHOP, |from| bishop_attacks(from, occupied));
    generate_piece_type!(PieceType::ROOK, |from| rook_attacks(from, occupied));
    generate_piece_type!(PieceType::GOLD, |from| GOLD_ATTACKS[from][us.to_index()]);
    generate_piece_type!(PieceType::KING, |from| KING_ATTACKS[from]);
    generate_piece_type!(PieceType::PRO_PAWN, |from| GOLD_ATTACKS[from][us.to_index()]);
    generate_piece_type!(PieceType::PRO_LANCE, |from| GOLD_ATTACKS[from][us.to_index()]);
    generate_piece_type!(PieceType::PRO_KNIGHT, |from| GOLD_ATTACKS[from][us.to_index()]);
    generate_piece_type!(PieceType::PRO_SILVER, |from| GOLD_ATTACKS[from][us.to_index()]);
    generate_piece_type!(PieceType::HORSE, |from| bishop_attacks(from, occupied)
        | KING_ATTACKS[from]);
    generate_piece_type!(PieceType::DRAGON, |from| rook_attacks(from, occupied)
        | KING_ATTACKS[from]);
}

pub(super) fn generate_board_moves<T: MoveGenType>(
    pos: &Position,
    only_to: Option<Square>,
    list: &mut impl MoveSink,
) {
    let us = pos.turn();
    let checkers = pos.checkers();
    let ordinary_mode = !T::IS_LEGAL && !T::EVASIONS && !T::IS_CHECKS && !T::QUIET_CHECKS;

    if ordinary_mode {
        let facts = GenerationFacts {
            us,
            king_sq: Square::NONE,
            pinned: Bitboard::EMPTY,
            non_king_evasion_targets: Bitboard::ALL,
            in_check: false,
        };
        generate_board_moves_with_facts::<T, false>(pos, only_to, list, facts);
        return;
    }

    let king_sq = pos.king_square(us);
    let non_king_evasion_targets = if checkers.is_empty() {
        Bitboard::ALL
    } else if checkers.more_than_one() {
        Bitboard::EMPTY
    } else {
        let checker_sq = checkers.lsb().expect("single checker");
        Bitboard::between(checker_sq, king_sq).or(Bitboard::from_square(checker_sq))
    };
    let facts = GenerationFacts {
        us,
        king_sq,
        pinned: pos.blockers_for_king(us),
        non_king_evasion_targets,
        in_check: !checkers.is_empty(),
    };
    generate_board_moves_with_facts::<T, true>(pos, only_to, list, facts);
}
