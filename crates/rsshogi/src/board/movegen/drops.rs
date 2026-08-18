use crate::board::{Bitboard, Position};
use crate::types::{Color, HandPiece, Move, PieceType, Rank, Square};

use super::{MoveGenType, MoveSink};

const DROP_PIECES: [(PieceType, HandPiece); HandPiece::COUNT] = [
    (PieceType::PAWN, HandPiece::PAWN),
    (PieceType::LANCE, HandPiece::LANCE),
    (PieceType::KNIGHT, HandPiece::KNIGHT),
    (PieceType::SILVER, HandPiece::SILVER),
    (PieceType::GOLD, HandPiece::GOLD),
    (PieceType::BISHOP, HandPiece::BISHOP),
    (PieceType::ROOK, HandPiece::ROOK),
];

fn dead_drop(piece_type: PieceType, to: Square, us: Color) -> bool {
    match piece_type {
        PieceType::PAWN | PieceType::LANCE => {
            if us == Color::BLACK {
                to.rank() == Rank::RANK_1
            } else {
                to.rank() == Rank::RANK_9
            }
        }
        PieceType::KNIGHT => {
            if us == Color::BLACK {
                to.rank().raw() <= 1
            } else {
                to.rank().raw() >= 7
            }
        }
        _ => false,
    }
}

fn drop_evasion_target(pos: &Position, to: Square, us: Color) -> bool {
    let checkers = pos.checkers();
    if checkers.is_empty() {
        return true;
    }
    if checkers.more_than_one() {
        return false;
    }
    let checker_sq = checkers.lsb().expect("non-empty checkers");
    Bitboard::between(checker_sq, pos.king_square(us)).test(to)
}

fn drop_rank_targets(piece_type: PieceType, us: Color) -> Bitboard {
    let forbidden = match piece_type {
        PieceType::PAWN | PieceType::LANCE => {
            if us == Color::BLACK {
                Bitboard::rank_mask(Rank::RANK_1)
            } else {
                Bitboard::rank_mask(Rank::RANK_9)
            }
        }
        PieceType::KNIGHT => {
            if us == Color::BLACK {
                Bitboard::rank_mask(Rank::RANK_1) | Bitboard::rank_mask(Rank::RANK_2)
            } else {
                Bitboard::rank_mask(Rank::RANK_8) | Bitboard::rank_mask(Rank::RANK_9)
            }
        }
        _ => Bitboard::EMPTY,
    };
    Bitboard::ALL.and_not(forbidden)
}

fn pawn_drop_targets(pos: &Position, mut targets: Bitboard, us: Color) -> Bitboard {
    let mut occupied_files = Bitboard::EMPTY;
    let mut pawns = pos.bitboards().pieces_for(PieceType::PAWN, us);
    while let Some(square) = pawns.pop_lsb() {
        occupied_files |= Bitboard::file_mask(square.file());
    }
    targets = targets.and_not(occupied_files);

    let king = pos.king_square(us.flip());
    if !king.is_none() {
        let checking_rank = king.rank().raw() + if us == Color::BLACK { 1 } else { -1 };
        let checking_square = (0..9)
            .contains(&checking_rank)
            .then(|| Square::from_file_rank(king.file(), Rank::new(checking_rank)));
        if let Some(to) = checking_square
            && targets.test(to)
            && !pos.is_legal_drop(to)
        {
            targets.clear(to);
        }
    }
    targets
}

fn generate_common_drops<T: MoveGenType>(
    pos: &Position,
    list: &mut impl MoveSink,
    us: Color,
    hand: crate::types::Hand,
    mut targets: Bitboard,
) {
    if T::IS_LEGAL {
        let checkers = pos.checkers();
        targets = if checkers.is_empty() {
            targets
        } else if checkers.more_than_one() {
            Bitboard::EMPTY
        } else {
            let checker = checkers.lsb().expect("single checker");
            targets & Bitboard::between(checker, pos.king_square(us))
        };
    }

    for (piece_type, hand_piece) in DROP_PIECES {
        if list.stop() {
            return;
        }
        if hand.count(hand_piece) == 0 {
            continue;
        }
        let mut destinations = targets & drop_rank_targets(piece_type, us);
        if piece_type == PieceType::PAWN {
            destinations = pawn_drop_targets(pos, destinations, us);
        }
        list.push_drop_targets(piece_type, destinations, us);
    }
}

fn drop_ok<T: MoveGenType>(pos: &Position, piece_type: PieceType, to: Square, us: Color) -> bool {
    if T::IS_LEGAL {
        return drop_evasion_target(pos, to, us)
            && (piece_type != PieceType::PAWN || pos.is_legal_pawn_drop(us, to));
    }
    if !T::EVASIONS && !T::IS_CHECKS && !T::QUIET_CHECKS && pos.checkers().is_empty() {
        return piece_type != PieceType::PAWN || pos.is_legal_pawn_drop(us, to);
    }
    pos.is_pseudo_legal_move(Move::drop(piece_type, to), T::GENERATE_ALL_LEGAL)
}

pub(super) fn generate_drops<T: MoveGenType>(pos: &Position, list: &mut impl MoveSink) {
    if (!T::QUIETS && !T::QUIET_CHECKS) || T::IS_RECAPTURES || T::IS_CAPTURE_PLUS_PRO {
        return;
    }
    let us = pos.turn();
    let hand = pos.hand(us);
    if hand.bits() == 0 {
        return;
    }
    let empties = pos.empties();

    let ordinary = !T::IS_CHECKS && !T::QUIET_CHECKS && !T::EVASIONS;
    if T::IS_LEGAL || ordinary {
        generate_common_drops::<T>(pos, list, us, hand, empties);
        return;
    }

    for (piece_type, hand_piece) in DROP_PIECES {
        if list.stop() {
            return;
        }
        if hand.count(hand_piece) == 0 {
            continue;
        }
        let mut destinations = empties;
        while let Some(to) = destinations.pop_lsb() {
            if dead_drop(piece_type, to, us) {
                continue;
            }
            if !drop_ok::<T>(pos, piece_type, to, us) {
                continue;
            }
            if (T::IS_CHECKS || T::QUIET_CHECKS)
                && !pos.gives_check_move(Move::drop(piece_type, to))
            {
                continue;
            }
            list.push_drop(piece_type, to, us);
        }
    }
}
