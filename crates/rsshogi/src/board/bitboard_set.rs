//! 局面管理用ビットボード集合
//!
//! [`BitboardSet`] は [`Position`](crate::board::Position) 内部で使用される
//! ビットボード群で、駒種別、先後別、全体占有の 3 種類のビットボードを
//! まとめて管理する。

#![allow(clippy::inline_always)]

use crate::types::Bitboard;
use crate::types::{Color, Piece, PieceType, Square};

// ANCHOR: bitboard_set_struct
/// 駒種別と先後別のビットボード集合
#[derive(Clone, Copy, Debug, Default)]
pub struct BitboardSet {
    /// 駒種別のビットボード（先後の区別なし）
    by_piece: [Bitboard; PieceType::COUNT],

    /// 先後別の全駒ビットボード
    by_color: [Bitboard; Color::COUNT],

    /// 全占有マス（両者の駒すべて）
    occupied: Bitboard,

    /// 金相当の駒（GOLD | PRO_PAWN | PRO_LANCE | PRO_KNIGHT | PRO_SILVER）
    golds_cache: Bitboard,

    /// 馬・龍・玉（HORSE | DRAGON | KING）
    hdk_cache: Bitboard,

    /// 角・馬（BISHOP | HORSE）
    bishop_horse_cache: Bitboard,

    /// 飛・龍（ROOK | DRAGON）
    rook_dragon_cache: Bitboard,

    /// 銀 + 馬/龍/玉（SILVER_HDK）
    silver_hdk_cache: Bitboard,

    /// 金相当 + 馬/龍/玉（GOLDS_HDK）
    golds_hdk_cache: Bitboard,
}
// ANCHOR_END: bitboard_set_struct

impl BitboardSet {
    /// 空のビットボード集合を作成
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline(always)]
    pub(crate) fn refresh_derived(&mut self) {
        let king = self.by_piece[PieceType::KING.to_index()];
        let horse = self.by_piece[PieceType::HORSE.to_index()];
        let dragon = self.by_piece[PieceType::DRAGON.to_index()];

        self.hdk_cache = king | horse | dragon;
        self.golds_cache = self.by_piece[PieceType::GOLD.to_index()]
            | self.by_piece[PieceType::PRO_PAWN.to_index()]
            | self.by_piece[PieceType::PRO_LANCE.to_index()]
            | self.by_piece[PieceType::PRO_KNIGHT.to_index()]
            | self.by_piece[PieceType::PRO_SILVER.to_index()];
        self.bishop_horse_cache = self.by_piece[PieceType::BISHOP.to_index()] | horse;
        self.rook_dragon_cache = self.by_piece[PieceType::ROOK.to_index()] | dragon;
        self.silver_hdk_cache = self.by_piece[PieceType::SILVER.to_index()] | self.hdk_cache;
        self.golds_hdk_cache = self.golds_cache | self.hdk_cache;
    }

    #[inline(always)]
    pub(crate) fn set_piece_no_refresh(&mut self, sq: Square, piece_type: PieceType, color: Color) {
        if piece_type == PieceType::NONE {
            return;
        }
        self.by_piece[piece_type.to_index()].set(sq);
        self.by_color[color.to_index()].set(sq);
        self.occupied.set(sq);
    }

    #[inline(always)]
    pub(crate) fn clear_piece_no_refresh(
        &mut self,
        sq: Square,
        piece_type: PieceType,
        color: Color,
    ) {
        if piece_type == PieceType::NONE {
            return;
        }
        self.by_piece[piece_type.to_index()].clear(sq);
        self.by_color[color.to_index()].clear(sq);
        self.occupied.clear(sq);
    }

    #[inline(always)]
    pub(crate) unsafe fn set_piece_no_refresh_unchecked(
        &mut self,
        sq: Square,
        piece_type: PieceType,
        color: Color,
    ) {
        unsafe {
            // SAFETY: 呼び出し側が `piece_type != PieceType::NONE`、
            // `piece_type.to_index() < PieceType::COUNT`、`color.to_index() <
            // Color::COUNT`、および `sq` が盤面内であることを保証する。
            debug_assert!(piece_type != PieceType::NONE);
            debug_assert!(sq.is_on_board());
            self.by_piece.get_unchecked_mut(piece_type.to_index()).set(sq);
            self.by_color.get_unchecked_mut(color.to_index()).set(sq);
            self.occupied.set(sq);
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn clear_piece_no_refresh_unchecked(
        &mut self,
        sq: Square,
        piece_type: PieceType,
        color: Color,
    ) {
        unsafe {
            // SAFETY: 呼び出し側が `piece_type != PieceType::NONE`、
            // `piece_type.to_index() < PieceType::COUNT`、`color.to_index() <
            // Color::COUNT`、および `sq` が盤面内であることを保証する。
            debug_assert!(piece_type != PieceType::NONE);
            debug_assert!(sq.is_on_board());
            self.by_piece.get_unchecked_mut(piece_type.to_index()).clear(sq);
            self.by_color.get_unchecked_mut(color.to_index()).clear(sq);
            self.occupied.clear(sq);
        }
    }

    #[inline(always)]
    pub(crate) fn set_no_refresh(&mut self, sq: Square, piece: Piece) {
        if piece.is_empty() {
            return;
        }
        self.set_piece_no_refresh(sq, piece.piece_type(), piece.color());
    }

    #[inline(always)]
    pub(crate) fn clear_no_refresh(&mut self, sq: Square, piece: Piece) {
        if piece.is_empty() {
            return;
        }
        self.clear_piece_no_refresh(sq, piece.piece_type(), piece.color());
    }

    #[inline(always)]
    pub(crate) unsafe fn set_no_refresh_unchecked(&mut self, sq: Square, piece: Piece) {
        unsafe {
            // SAFETY: 呼び出し側が `piece` が空でなく `sq` が盤面内であることを保証する。
            // これにより派生する駒種・手番のインデックスが有効になる。
            debug_assert!(!piece.is_empty());
            self.set_piece_no_refresh_unchecked(sq, piece.piece_type(), piece.color());
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn clear_no_refresh_unchecked(&mut self, sq: Square, piece: Piece) {
        unsafe {
            // SAFETY: 呼び出し側が `piece` が空でなく `sq` が盤面内であることを保証する。
            // これにより派生する駒種・手番のインデックスが有効になる。
            debug_assert!(!piece.is_empty());
            self.clear_piece_no_refresh_unchecked(sq, piece.piece_type(), piece.color());
        }
    }

    /// 駒を配置（ビットボードを更新）
    pub fn set_piece(&mut self, sq: Square, piece_type: PieceType, color: Color) {
        self.set_piece_no_refresh(sq, piece_type, color);
        self.refresh_derived();
    }

    /// 駒を除去（ビットボードを更新）
    pub fn clear_piece(&mut self, sq: Square, piece_type: PieceType, color: Color) {
        self.clear_piece_no_refresh(sq, piece_type, color);
        self.refresh_derived();
    }

    // ANCHOR: bitboard_set_move_piece
    /// 駒を移動（fromからtoへ）
    pub fn move_piece(&mut self, from: Square, to: Square, piece_type: PieceType, color: Color) {
        let piece_bb = &mut self.by_piece[piece_type.to_index()];
        piece_bb.clear(from);
        piece_bb.set(to);

        let color_bb = &mut self.by_color[color.to_index()];
        color_bb.clear(from);
        color_bb.set(to);

        self.occupied.clear(from);
        self.occupied.set(to);
        self.refresh_derived();
    }
    // ANCHOR_END: bitboard_set_move_piece

    /// 指定した駒種のビットボードを取得
    #[inline]
    #[must_use]
    pub const fn pieces(&self, piece_type: PieceType) -> Bitboard {
        self.by_piece[piece_type.to_index()]
    }

    /// 指定した先後の全駒ビットボードを取得
    #[inline]
    #[must_use]
    pub const fn color_pieces(&self, color: Color) -> Bitboard {
        self.by_color[color.to_index()]
    }

    /// 指定した先後・駒種のビットボードを取得
    #[inline]
    #[must_use]
    pub fn pieces_for(&self, piece_type: PieceType, color: Color) -> Bitboard {
        self.pieces(piece_type).and(self.color_pieces(color))
    }

    /// 金相当の駒（歩成りなどを含む）
    #[inline]
    #[must_use]
    pub const fn golds(&self) -> Bitboard {
        self.golds_cache
    }

    /// 馬・龍・玉（Horse-Dragon-King）
    #[inline]
    #[must_use]
    pub const fn horse_dragon_king(&self) -> Bitboard {
        self.hdk_cache
    }

    /// 角・馬（BISHOP_HORSE）
    #[inline]
    #[must_use]
    pub const fn bishop_horse(&self) -> Bitboard {
        self.bishop_horse_cache
    }

    /// 飛・龍（ROOK_DRAGON）
    #[inline]
    #[must_use]
    pub const fn rook_dragon(&self) -> Bitboard {
        self.rook_dragon_cache
    }

    /// 銀 + 馬/龍/玉（SILVER_HDK）
    #[inline]
    #[must_use]
    pub const fn silver_hdk(&self) -> Bitboard {
        self.silver_hdk_cache
    }

    /// 金相当 + 馬/龍/玉（GOLDS_HDK）
    #[inline]
    #[must_use]
    pub const fn golds_hdk(&self) -> Bitboard {
        self.golds_hdk_cache
    }

    /// 全占有マスを取得
    #[inline]
    #[must_use]
    pub const fn occupied(&self) -> Bitboard {
        self.occupied
    }

    /// 駒を配置（簡易版）
    pub fn set(&mut self, sq: Square, piece: Piece) {
        self.set_no_refresh(sq, piece);
        self.refresh_derived();
    }

    /// 駒を除去（簡易版）
    pub fn clear(&mut self, sq: Square, piece: Piece) {
        self.clear_no_refresh(sq, piece);
        self.refresh_derived();
    }

    /// 空きマスを取得
    #[inline]
    #[must_use]
    pub const fn empties(&self) -> Bitboard {
        self.occupied.not()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Color, PieceType, SQ_16, SQ_17, SQ_82};

    #[test]
    fn test_bitboard_set_operations() {
        let mut bbs = BitboardSet::new();

        // 先手の歩を配置
        bbs.set_piece(SQ_17, PieceType::PAWN, Color::BLACK);
        assert!(bbs.pieces(PieceType::PAWN).test(SQ_17));
        assert!(bbs.color_pieces(Color::BLACK).test(SQ_17));
        assert!(bbs.occupied().test(SQ_17));

        // 後手の飛車を配置
        bbs.set_piece(SQ_82, PieceType::ROOK, Color::WHITE);
        assert!(bbs.pieces(PieceType::ROOK).test(SQ_82));
        assert!(bbs.color_pieces(Color::WHITE).test(SQ_82));

        // 先手の歩だけを取得
        let black_pawns = bbs.pieces_for(PieceType::PAWN, Color::BLACK);
        assert!(black_pawns.test(SQ_17));
        assert!(!black_pawns.test(SQ_82));

        // 駒を移動
        bbs.move_piece(SQ_17, SQ_16, PieceType::PAWN, Color::BLACK);
        assert!(!bbs.pieces(PieceType::PAWN).test(SQ_17));
        assert!(bbs.pieces(PieceType::PAWN).test(SQ_16));

        // 駒を除去
        bbs.clear_piece(SQ_82, PieceType::ROOK, Color::WHITE);
        assert!(!bbs.pieces(PieceType::ROOK).test(SQ_82));
        assert!(!bbs.occupied().test(SQ_82));
    }

    #[test]
    fn test_composite_caches() {
        let mut bbs = BitboardSet::new();

        // 金を配置 → golds キャッシュに反映
        bbs.set_piece(SQ_17, PieceType::GOLD, Color::BLACK);
        assert!(bbs.golds().test(SQ_17));

        // と金を配置 → golds キャッシュに反映
        bbs.set_piece(SQ_16, PieceType::PRO_PAWN, Color::BLACK);
        assert!(bbs.golds().test(SQ_16));

        // 馬を配置 → HDK + bishop_horse に反映
        bbs.set_piece(SQ_82, PieceType::HORSE, Color::WHITE);
        assert!(bbs.horse_dragon_king().test(SQ_82));
        assert!(bbs.bishop_horse().test(SQ_82));
        assert!(!bbs.rook_dragon().test(SQ_82));

        // 龍を配置 → HDK + rook_dragon に反映
        let sq_11 = Square::from_file_rank(crate::types::File::FILE_1, crate::types::Rank::RANK_1);
        bbs.set_piece(sq_11, PieceType::DRAGON, Color::WHITE);
        assert!(bbs.horse_dragon_king().test(sq_11));
        assert!(bbs.rook_dragon().test(sq_11));
        assert!(!bbs.bishop_horse().test(sq_11));

        // 馬を除去 → キャッシュから消える
        bbs.clear_piece(SQ_82, PieceType::HORSE, Color::WHITE);
        assert!(!bbs.horse_dragon_king().test(SQ_82));
        assert!(!bbs.bishop_horse().test(SQ_82));
    }
}
