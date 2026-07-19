use super::Position;
use crate::types::{Bitboard, Color, PieceType, Square};

impl Position {
    /// 指定マス `sq` を攻撃している全駒の `Bitboard` を返す。
    ///
    /// `occupied` は飛び駒の利きを決める盤面占有状態を渡す。
    #[must_use]
    #[inline]
    pub fn attackers_to(&self, sq: Square, occupied: Bitboard) -> Bitboard {
        use crate::board::attack_tables::{
            GOLD_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS, SILVER_ATTACKS,
        };
        use crate::board::attack_tables::{bishop_attacks, lance_step_attacks, rook_attacks};
        if sq.is_none() {
            return Bitboard::EMPTY;
        }

        let sq_idx = sq.to_index();
        let bb = self.bitboards();
        let mut attackers = Bitboard::EMPTY;
        let pawns = bb.pieces(PieceType::PAWN);
        let knights = bb.pieces(PieceType::KNIGHT);
        let bishops_horse = bb.bishop_horse();
        let rooks_dragon = bb.rook_dragon();
        let lances = bb.pieces(PieceType::LANCE);
        let silver_hdk = bb.silver_hdk();
        let golds_hdk = bb.golds_hdk();

        for &color in &[Color::BLACK, Color::WHITE] {
            let them = color.flip();
            let color_mask = bb.color_pieces(color);

            let step_attacks = (PAWN_ATTACKS[sq_idx][them.to_index()] & pawns)
                | (KNIGHT_ATTACKS[sq_idx][them.to_index()] & knights)
                | (SILVER_ATTACKS[sq_idx][them.to_index()] & silver_hdk)
                | (GOLD_ATTACKS[sq_idx][them.to_index()] & golds_hdk);

            let bishop_atk = bishop_attacks(sq, occupied) & bishops_horse;
            let lance_line = lance_step_attacks(sq, them) & lances;
            let rook_atk = rook_attacks(sq, occupied) & (rooks_dragon | lance_line);

            attackers |= (step_attacks | bishop_atk | rook_atk) & color_mask;
        }

        attackers
    }

    /// 指定マスを攻撃している指定色の駒を返す（occupiedは指定の盤面）。
    ///
    /// `attackers_to()` は両色分をまとめて計算するため、片側だけ欲しい場合はこちらが高速。
    #[must_use]
    #[inline]
    pub(crate) fn attackers_to_color_fast(
        &self,
        color: Color,
        sq: Square,
        occupied: Bitboard,
    ) -> Bitboard {
        if sq.is_none() {
            return Bitboard::EMPTY;
        }

        match color {
            Color::BLACK => self.attackers_to_color_fast_for::<true>(sq, occupied),
            Color::WHITE => self.attackers_to_color_fast_for::<false>(sq, occupied),
        }
    }

    /// 指定マスが指定色に攻撃されているかを高速に判定する（occupiedは指定の盤面）。
    ///
    /// `attackers_to_color_fast()` と同等の判定だが、存在有無だけ返し、
    /// 途中で攻撃駒が見つかった時点で return する。
    #[must_use]
    #[inline]
    pub(crate) fn attacked_by_color_fast(
        &self,
        color: Color,
        sq: Square,
        occupied: Bitboard,
    ) -> bool {
        if sq.is_none() {
            return false;
        }
        match color {
            Color::BLACK => self.attacked_by_color_fast_for::<true>(sq, occupied),
            Color::WHITE => self.attacked_by_color_fast_for::<false>(sq, occupied),
        }
    }

    #[must_use]
    #[inline]
    fn attackers_to_color_fast_for<const BLACK: bool>(
        &self,
        sq: Square,
        occupied: Bitboard,
    ) -> Bitboard {
        use crate::board::attack_tables::{
            GOLD_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS, SILVER_ATTACKS,
        };
        use crate::board::attack_tables::{bishop_attacks, lance_step_attacks, rook_attacks};
        let us = if BLACK { Color::BLACK } else { Color::WHITE };
        let them = us.flip();
        let sq_idx = sq.to_index();
        let bb = self.bitboards();
        let ours = bb.color_pieces(us);
        let silver_hdk = bb.silver_hdk();
        let golds_hdk = bb.golds_hdk();

        let step_attackers = (PAWN_ATTACKS[sq_idx][them.to_index()] & bb.pieces(PieceType::PAWN))
            | (KNIGHT_ATTACKS[sq_idx][them.to_index()] & bb.pieces(PieceType::KNIGHT))
            | (SILVER_ATTACKS[sq_idx][them.to_index()] & silver_hdk)
            | (GOLD_ATTACKS[sq_idx][them.to_index()] & golds_hdk);
        let bishop_attackers = bishop_attacks(sq, occupied) & bb.bishop_horse();
        let lance_line = lance_step_attacks(sq, them) & bb.pieces(PieceType::LANCE);
        let rook_attackers = rook_attacks(sq, occupied) & (bb.rook_dragon() | lance_line);

        (step_attackers | bishop_attackers | rook_attackers) & ours
    }

    #[must_use]
    #[inline]
    pub(super) fn attacked_by_color_fast_for<const BLACK: bool>(
        &self,
        sq: Square,
        occupied: Bitboard,
    ) -> bool {
        use crate::board::attack_tables::{
            GOLD_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS, SILVER_ATTACKS,
        };
        use crate::board::attack_tables::{bishop_attacks, lance_step_attacks, rook_attacks};
        let us = if BLACK { Color::BLACK } else { Color::WHITE };
        let them = us.flip();
        let sq_idx = sq.to_index();
        let bb = self.bitboards();
        let ours = bb.color_pieces(us);
        let step_attackers = (PAWN_ATTACKS[sq_idx][them.to_index()] & bb.pieces(PieceType::PAWN))
            | (KNIGHT_ATTACKS[sq_idx][them.to_index()] & bb.pieces(PieceType::KNIGHT))
            | (SILVER_ATTACKS[sq_idx][them.to_index()] & bb.silver_hdk())
            | (GOLD_ATTACKS[sq_idx][them.to_index()] & bb.golds_hdk());
        if !(step_attackers & ours).is_empty() {
            return true;
        }

        if !(bishop_attacks(sq, occupied) & bb.bishop_horse() & ours).is_empty() {
            return true;
        }

        let lance_line = lance_step_attacks(sq, them) & bb.pieces(PieceType::LANCE);
        let rook_like = bb.rook_dragon() | lance_line;
        if !(rook_attacks(sq, occupied) & rook_like & ours).is_empty() {
            return true;
        }

        false
    }

    /// 指定マスを攻撃している指定色の駒を返す（occupiedは指定の盤面）。
    #[must_use]
    #[inline]
    pub fn attackers_to_color(&self, color: Color, sq: Square, occupied: Bitboard) -> Bitboard {
        self.attackers_to_color_fast(color, sq, occupied)
    }

    /// 指定マスを攻撃している指定色の駒を返す（現在の盤面）。
    #[must_use]
    #[inline]
    pub fn attackers_to_color_current(&self, color: Color, sq: Square) -> Bitboard {
        self.attackers_to_color_fast(color, sq, self.bitboards().occupied())
    }

    /// 指定マスが指定色に攻撃されているか判定（occupiedは指定の盤面）。
    ///
    /// 一時的な占有状態での利き確認が必要な SEE などで使う。
    #[must_use]
    #[inline]
    pub fn is_attacked_by_color(&self, by_color: Color, sq: Square, occupied: Bitboard) -> bool {
        self.attacked_by_color_fast(by_color, sq, occupied)
    }

    /// 指定マス `sq` が指定色 `by_color` に攻撃されていれば `true` を返す。
    #[must_use]
    #[inline]
    pub fn is_attacked_by(&self, sq: Square, by_color: Color) -> bool {
        let occupied = self.bitboards().occupied();
        self.attacked_by_color_fast(by_color, sq, occupied)
    }

    pub(in crate::board) fn attackers_to_pawn(&self, color: Color, sq: Square) -> Bitboard {
        use crate::board::attack_tables::{GOLD_ATTACKS, KNIGHT_ATTACKS, SILVER_ATTACKS};
        use crate::board::attack_tables::{bishop_attacks, rook_attacks};
        if sq.is_none() {
            return Bitboard::EMPTY;
        }

        let them = color.flip();
        let bb = self.bitboards();
        let occupied = bb.occupied();

        let bb_hd = bb.pieces(PieceType::HORSE) | bb.pieces(PieceType::DRAGON);
        let knights = KNIGHT_ATTACKS[sq][them.to_index()] & bb.pieces(PieceType::KNIGHT);
        let silvers = SILVER_ATTACKS[sq][them.to_index()] & (bb.pieces(PieceType::SILVER) | bb_hd);
        let golds = GOLD_ATTACKS[sq][them.to_index()]
            & (bb.pieces(PieceType::GOLD)
                | bb.pieces(PieceType::PRO_PAWN)
                | bb.pieces(PieceType::PRO_LANCE)
                | bb.pieces(PieceType::PRO_KNIGHT)
                | bb.pieces(PieceType::PRO_SILVER)
                | bb_hd);
        let bishops = bishop_attacks(sq, occupied) & bb.bishop_horse();
        let rooks = rook_attacks(sq, occupied) & bb.rook_dragon();

        (knights | silvers | golds | bishops | rooks) & bb.color_pieces(color)
    }

    /// 指定した駒種が指定位置から攻撃する範囲を返す
    #[must_use]
    pub(crate) fn piece_attacks(
        piece_type: PieceType,
        sq: Square,
        c: Color,
        occupied: Bitboard,
    ) -> Bitboard {
        use crate::board::attack_tables::{
            GOLD_ATTACKS, KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS, SILVER_ATTACKS,
        };
        use crate::board::attack_tables::{bishop_attacks, lance_attacks, rook_attacks};

        match piece_type {
            PieceType::PAWN => PAWN_ATTACKS[sq][c.to_index()],
            PieceType::LANCE => lance_attacks(sq, occupied, c),
            PieceType::KNIGHT => KNIGHT_ATTACKS[sq][c.to_index()],
            PieceType::SILVER => SILVER_ATTACKS[sq][c.to_index()],
            PieceType::GOLD
            | PieceType::PRO_PAWN
            | PieceType::PRO_LANCE
            | PieceType::PRO_KNIGHT
            | PieceType::PRO_SILVER => GOLD_ATTACKS[sq][c.to_index()],
            PieceType::BISHOP => bishop_attacks(sq, occupied),
            PieceType::ROOK => rook_attacks(sq, occupied),
            PieceType::HORSE => {
                // 角の動き + 縦横1マス
                let bishop_moves = bishop_attacks(sq, occupied);
                let king_moves = KING_ATTACKS[sq];
                bishop_moves | king_moves
            }
            PieceType::DRAGON => {
                // 飛車の動き + 斜め1マス
                let rook_moves = rook_attacks(sq, occupied);
                let king_moves = KING_ATTACKS[sq];
                rook_moves | king_moves
            }
            PieceType::KING => KING_ATTACKS[sq],
            _ => Bitboard::EMPTY,
        }
    }

    /// 指定色の駒種が攻撃しているマス集合を返す。
    #[must_use]
    pub fn attacks_by(&self, color: Color, piece_type: PieceType) -> Bitboard {
        use crate::board::attack_tables::PAWN_ATTACKS;

        let occupied = self.bitboards().occupied();
        let mut attacks = Bitboard::EMPTY;
        let mut pieces = self.bitboards().pieces_for(piece_type, color);

        if piece_type == PieceType::PAWN {
            while let Some(sq) = pieces.pop_lsb() {
                attacks |= PAWN_ATTACKS[sq][color.to_index()];
            }
            return attacks;
        }

        while let Some(sq) = pieces.pop_lsb() {
            attacks |= Self::piece_attacks(piece_type, sq, color, occupied);
        }

        attacks
    }

    // --- プライベートヘルパー ---

    /// 指定色の玉に対するブロッカーとピンしている敵大駒を算出（色定数化版）。
    #[inline]
    pub(in crate::board) fn compute_slider_info_for<const BLACK: bool>(
        &self,
    ) -> (Bitboard, Bitboard) {
        use crate::board::attack_tables::{
            bishop_step_attacks, lance_step_attacks, rook_step_attacks,
        };
        let color = if BLACK { Color::BLACK } else { Color::WHITE };
        let them = if BLACK { Color::WHITE } else { Color::BLACK };

        let king_sq = self.king_square(color);
        if king_sq.is_none() {
            return (Bitboard::EMPTY, Bitboard::EMPTY);
        }

        let bb = self.bitboards();
        let enemy = bb.color_pieces(them);
        let mut snipers = ((bb.rook_dragon() & rook_step_attacks(king_sq))
            | (bb.bishop_horse() & bishop_step_attacks(king_sq))
            | (bb.pieces(PieceType::LANCE) & lance_step_attacks(king_sq, color)))
            & enemy;
        if snipers.is_empty() {
            return (Bitboard::EMPTY, Bitboard::EMPTY);
        }

        let occupancy = bb.occupied() ^ snipers;
        let ours = bb.color_pieces(color);

        let mut blockers = Bitboard::EMPTY;
        let mut pinners = Bitboard::EMPTY;

        while !snipers.is_empty() {
            // SAFETY: ループ条件により `snipers` が非空であることが保証される。
            let sniper_sq = unsafe { snipers.pop_lsb_unchecked() };
            let between = Bitboard::between(sniper_sq, king_sq) & occupancy;

            if !between.is_empty() && !between.more_than_one() {
                blockers |= between;
                if between.intersects(ours) {
                    pinners.set(sniper_sq);
                }
            }
        }

        (blockers, pinners)
    }
}
