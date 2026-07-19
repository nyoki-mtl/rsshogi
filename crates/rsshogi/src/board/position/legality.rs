use super::Position;
use crate::types::{Bitboard, Color, HandPiece, Move, Move32, MoveBase, Piece, PieceType, Rank};

impl Position {
    /// 指し手がpseudo-legalかどうかを判定する（自殺手チェックは含まない）。
    ///
    /// `MoveBase` を実装する任意の指し手型に対応する内部実装。
    /// `Move32` の場合は `piece_after_hint()` による駒情報整合性チェックも行う。
    #[inline]
    #[allow(clippy::too_many_lines)]
    pub(crate) fn is_pseudo_legal_impl(
        &self,
        mv: impl MoveBase,
        generate_all_legal_moves: bool,
    ) -> bool {
        let m = mv.to_move();
        let piece_hint = mv.piece_after_hint();
        let us = self.turn();
        let to = m.to_sq();

        if m.is_drop() {
            let Some(piece_type) = m.dropped_piece() else {
                return false;
            };
            // Move32 の場合: 駒情報の整合性チェック
            if piece_hint != Piece::NONE && piece_hint != Piece::from_parts(us, piece_type) {
                return false;
            }
            let hand_piece = HandPiece::from_piece_type(piece_type)
                .expect("dropped piece must map to hand piece");
            if self.piece_on(to) != Piece::NONE {
                return false;
            }
            if self.hand(us).count(hand_piece) == 0 {
                return false;
            }

            let checkers = self.checkers();
            if !checkers.is_empty() {
                let mut checkers_bb = checkers;
                let checker_sq = checkers_bb.pop_lsb().expect("checker exists");
                if !checkers_bb.is_empty() {
                    return false;
                }
                let king_sq = self.king_square(us);
                if !Bitboard::between(checker_sq, king_sq).test(to) {
                    return false;
                }
            }

            if piece_type == PieceType::PAWN && !self.is_legal_pawn_drop(us, to) {
                return false;
            }

            return true;
        }

        let from = m.from_sq();
        let piece = self.piece_on(from);
        if piece == Piece::NONE || piece.color() != us {
            return false;
        }
        // Move32 の場合: 駒情報の整合性チェック
        if piece_hint != Piece::NONE {
            let expected = if m.is_promotion() { piece.promote() } else { piece };
            if piece_hint != expected {
                return false;
            }
        }

        let occupied = self.bitboards().occupied();
        if !Self::piece_attacks(piece.piece_type(), from, us, occupied).test(to) {
            return false;
        }

        if self.bitboards().color_pieces(us).test(to) {
            return false;
        }
        let pt = piece.piece_type();
        if m.is_promotion() {
            if !pt.is_promotable() {
                return false;
            }
        } else {
            if pt == PieceType::NONE {
                return false;
            }
            if generate_all_legal_moves {
                if (pt == PieceType::PAWN || pt == PieceType::LANCE)
                    && ((us == Color::BLACK && to.rank() == Rank::RANK_1)
                        || (us == Color::WHITE && to.rank() == Rank::RANK_9))
                {
                    return false;
                }
            } else {
                match pt {
                    PieceType::PAWN => {
                        let enemy_territory = Bitboard::promotion_zone(us);
                        if enemy_territory.test(to) {
                            return false;
                        }
                    }
                    PieceType::LANCE
                        if (us == Color::BLACK && to.rank() <= Rank::RANK_2)
                            || (us == Color::WHITE && to.rank() >= Rank::RANK_8) =>
                    {
                        return false;
                    }
                    PieceType::BISHOP | PieceType::ROOK
                        if crate::types::square::is_promotable_move(us, from, to) =>
                    {
                        return false;
                    }
                    _ => {}
                }
            }
        }

        let checkers = self.checkers();
        if !checkers.is_empty() && pt != PieceType::KING {
            if checkers.count() > 1 {
                return false;
            }
            let king_sq = self.king_square(us);
            let checker_sq = checkers.lsb().expect("single checker exists");
            let target =
                Bitboard::between(checker_sq, king_sq).or(Bitboard::from_square(checker_sq));
            if !target.test(to) {
                return false;
            }
        }

        true
    }

    /// `Move32` 版の pseudo-legal 判定。
    ///
    /// `piece_after_move()` による駒情報の整合性チェックを含む。
    #[must_use]
    #[inline]
    pub fn is_pseudo_legal_move32(&self, mv: Move32, generate_all_legal_moves: bool) -> bool {
        self.is_pseudo_legal_impl(mv, generate_all_legal_moves)
    }

    /// `Move` 版の pseudo-legal 判定。
    #[must_use]
    #[inline]
    pub fn is_pseudo_legal_move(&self, mv: Move, generate_all_legal_moves: bool) -> bool {
        self.is_pseudo_legal_impl(mv, generate_all_legal_moves)
    }
}
