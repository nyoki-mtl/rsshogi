use super::Position;
use super::types::{BoardArray, Ply};
use crate::board::bitboard_set::BitboardSet;
use crate::board::zobrist::ZobristKey;
use crate::types::{
    Bitboard, Color, File, Hand, Move, Move32, Move32Metadata, MoveBase, Piece, PieceType, Square,
};

impl Position {
    /// 指定マスのPieceを取得。
    #[inline]
    #[must_use]
    pub fn piece_on(&self, sq: Square) -> Piece {
        if sq.is_none() {
            return Piece::NONE;
        }
        self.board.get(sq)
    }

    /// 指定マスのPieceを取得（境界チェックなし）
    ///
    /// # Safety
    /// `sq` が盤面内であることを呼び出し側が保証すること。
    #[inline]
    #[must_use]
    pub(crate) unsafe fn piece_on_unchecked(&self, sq: Square) -> Piece {
        unsafe {
            // SAFETY: 呼び出し側が `sq` を盤面内に保つことを保証しており、
            // それが `BoardArray::get_unchecked` の事前条件と一致する。
            self.board.get_unchecked(sq)
        }
    }

    /// 指定マスが空かどうかを返す。
    #[inline]
    #[must_use]
    pub fn is_square_empty(&self, sq: Square) -> bool {
        self.piece_on(sq) == Piece::NONE
    }

    /// 駒がない升が1になっているBitboardを返す。
    #[inline]
    #[must_use]
    pub fn empties(&self) -> Bitboard {
        !self.bitboards().occupied()
    }

    /// 持ち駒を取得。
    #[inline]
    #[must_use]
    pub const fn hand(&self, color: Color) -> Hand {
        self.hands[color.to_index()]
    }

    /// 持ち駒への可変参照を取得
    #[inline]
    #[must_use]
    pub(crate) fn hand_mut(&mut self, color: Color) -> &mut Hand {
        &mut self.hands[color.to_index()]
    }

    /// 手番を取得
    #[inline]
    #[must_use]
    pub const fn turn(&self) -> Color {
        self.side_to_move
    }

    /// 手番を設定
    #[inline]
    pub(crate) fn set_side_to_move(&mut self, color: Color) {
        self.side_to_move = color;
    }

    /// 手番を反転
    #[inline]
    pub(crate) fn flip_side_to_move(&mut self) {
        self.side_to_move = self.side_to_move.flip();
    }

    /// 指定色の玉の位置を取得
    #[inline]
    #[must_use]
    pub const fn king_square(&self, color: Color) -> Square {
        self.king_square[color.to_index()]
    }

    /// 指定色の玉の位置を設定
    #[inline]
    pub(crate) fn set_king_square(&mut self, color: Color, sq: Square) {
        self.king_square[color.to_index()] = sq;
    }

    /// 指定色の玉の位置を返す。
    #[must_use]
    pub fn square(&self, color: Color, piece_type: PieceType) -> Square {
        debug_assert!(
            piece_type == PieceType::KING,
            "Position::square only supports KING in shogi mode"
        );
        self.king_square(color)
    }

    /// 指定筋に指定色の歩が存在するか判定（二歩判定用）
    #[must_use]
    pub fn has_pawn_on_file(&self, color: Color, file: File) -> bool {
        use crate::types::PieceType;

        // 指定色の歩のBitboardを取得
        let pawns = self.bitboards.pieces_for(PieceType::PAWN, color);

        // 指定筋のBitboardを作成
        let file_mask = Bitboard::file_mask(file);

        // 交差判定
        !(pawns & file_mask).is_empty()
    }

    /// 平手初期局面からの手数を取得。
    #[inline]
    #[must_use]
    pub const fn game_ply(&self) -> Ply {
        self.ply
    }

    /// 盤面配列を取得
    #[inline]
    #[must_use]
    pub fn board_array(&self) -> BoardArray {
        self.board
    }

    /// 持ち駒配列を取得
    #[inline]
    #[must_use]
    pub fn hands_array(&self) -> [Hand; Color::COUNT] {
        self.hands
    }

    /// 局面のキーを取得。
    #[inline]
    #[must_use]
    pub const fn key(&self) -> ZobristKey {
        self.zobrist
    }

    /// 指し手で移動させる駒（移動前）を返す。
    #[must_use]
    #[inline]
    pub fn moved_piece_before(&self, mv: Move32) -> Piece {
        self.moved_piece_before_impl(mv)
    }

    /// `Move` 版の `moved_piece_before`。
    #[must_use]
    #[inline]
    pub fn moved_piece_before_move(&self, mv: Move) -> Piece {
        self.moved_piece_before_impl(mv)
    }

    #[inline]
    fn moved_piece_before_impl(&self, mv: impl MoveBase) -> Piece {
        let m = mv.to_move();
        if m.is_drop() {
            let Some(pt) = m.dropped_piece() else {
                return Piece::NONE;
            };
            return Piece::from_parts(self.turn(), pt);
        }

        let from = m.from_sq();
        if from.is_none() {
            return Piece::NONE;
        }
        self.piece_on(from)
    }

    /// 指し手で移動させる駒（移動後）を返す。
    #[must_use]
    #[inline]
    pub fn moved_piece_after(&self, mv: Move32) -> Piece {
        self.moved_piece_after_impl(mv)
    }

    /// `Move` 版の `moved_piece_after`。
    #[must_use]
    #[inline]
    pub fn moved_piece_after_move(&self, mv: Move) -> Piece {
        self.moved_piece_after_impl(mv)
    }

    #[inline]
    fn moved_piece_after_impl(&self, mv: impl MoveBase) -> Piece {
        let m = mv.to_move();
        if m.is_drop() {
            let Some(pt) = m.dropped_piece() else {
                return Piece::NONE;
            };
            return Piece::from_parts(self.turn(), pt);
        }
        let from = m.from_sq();
        if from.is_none() {
            return Piece::NONE;
        }
        let piece = self.piece_on(from);
        if m.is_promotion() { piece.promote() } else { piece }
    }

    /// 指し手で移動させる駒（移動後）を返す。
    #[must_use]
    #[inline]
    pub fn moved_piece(&self, mv: Move32) -> Piece {
        self.moved_piece_after(mv)
    }

    /// `Move32` の軽量メタデータを返す。
    ///
    /// engine 側で頻出する「移動後の駒 / 捕獲有無 / 成り / 打ち」の分類を
    /// 1 回で取りたいときに使う。
    #[must_use]
    #[inline]
    pub fn move32_metadata(&self, mv: Move32) -> Move32Metadata {
        let piece_after =
            if mv.has_piece_info() { mv.piece_after_move() } else { self.moved_piece_after(mv) };
        let captured_piece = if mv.is_drop() {
            Piece::NONE
        } else {
            let piece = self.piece_on(mv.to_sq());
            if piece == Piece::NONE || piece.color() == self.turn() { Piece::NONE } else { piece }
        };

        Move32Metadata::with_from_to_index(
            piece_after,
            captured_piece,
            captured_piece != Piece::NONE,
            mv.is_promotion(),
            mv.is_drop(),
            mv.from_to_index(),
        )
    }

    /// `Move32` を探索向けに分類したメタデータを返す。
    #[must_use]
    #[inline]
    pub fn classify_move32(&self, mv: Move32) -> Move32Metadata {
        self.move32_metadata(mv)
    }

    /// 直前の指し手で捕獲された駒を取得。
    #[must_use]
    #[inline]
    pub fn captured_piece(&self) -> Piece {
        self.current_cold().captured_piece()
    }

    /// 直前の指し手を取得。
    #[must_use]
    #[inline]
    pub fn last_move(&self) -> Move32 {
        self.current_cold().last_move()
    }

    /// 直前に動かした駒種を取得。
    #[must_use]
    #[inline]
    pub fn last_moved_piece_type(&self) -> PieceType {
        self.current_cold().last_moved_piece_type()
    }

    /// 現局面の繰り返し回数カウンタを返す。
    #[must_use]
    pub fn repetition_counter(&self) -> i32 {
        self.current_hot().repetition_counter
    }

    /// 現局面の繰り返し検出距離を返す。
    #[must_use]
    pub fn repetition_distance(&self) -> i32 {
        self.current_hot().repetition_distance
    }

    /// 現局面の繰り返し回数（- 1）を返す。
    #[must_use]
    pub fn repetition_times(&self) -> i32 {
        self.current_hot().repetition_times()
    }

    /// 現局面の千日手の状態（キャッシュ値）を返す。
    ///
    /// 千日手判定そのものは [`Position::repetition_state`] を参照すること。
    #[must_use]
    pub fn repetition_type(&self) -> crate::types::RepetitionState {
        self.current_hot().repetition_type()
    }

    /// null move からの手数を返す。
    #[must_use]
    pub fn plies_from_null(&self) -> Ply {
        self.current_hot().plies_from_null()
    }

    /// 連続王手カウンタ（`[先手, 後手]`）を返す。
    #[must_use]
    pub fn continuous_checks(&self) -> [u16; Color::COUNT] {
        self.current_hot().continuous_checks()
    }

    /// ビットボードを取得
    #[inline]
    #[must_use]
    pub const fn bitboards(&self) -> &BitboardSet {
        &self.bitboards
    }

    /// 盤上にある全駒のビットボードを取得。
    #[inline]
    #[must_use]
    pub const fn pieces(&self) -> Bitboard {
        self.bitboards().occupied()
    }

    /// 指定した駒種のビットボードを取得。
    #[inline]
    #[must_use]
    pub const fn pieces_by_type(&self, piece_type: PieceType) -> Bitboard {
        self.bitboards().pieces(piece_type)
    }

    /// 指定した複数の駒種のビットボードを取得。
    #[must_use]
    pub fn pieces_by_types(&self, piece_types: &[PieceType]) -> Bitboard {
        let mut out = Bitboard::EMPTY;
        for &piece_type in piece_types {
            out |= self.bitboards().pieces(piece_type);
        }
        out
    }

    /// 指定した複数の駒種のビットボードを取得（配列版）。
    #[must_use]
    pub fn pieces_by_types_array<const N: usize>(&self, piece_types: [PieceType; N]) -> Bitboard {
        let mut out = Bitboard::EMPTY;
        for &piece_type in &piece_types {
            out |= self.bitboards().pieces(piece_type);
        }
        out
    }

    /// 指定色の駒全体のビットボードを取得。
    #[inline]
    #[must_use]
    pub const fn pieces_by_color(&self, color: Color) -> Bitboard {
        self.bitboards().color_pieces(color)
    }

    /// 指定色・駒種のビットボードを取得。
    #[inline]
    #[must_use]
    pub fn pieces_for(&self, piece_type: PieceType, color: Color) -> Bitboard {
        self.bitboards().pieces_for(piece_type, color)
    }

    /// 金相当の駒（歩成りなどを含む）。
    #[inline]
    #[must_use]
    pub fn golds(&self) -> Bitboard {
        self.bitboards().golds()
    }

    /// 馬・龍・玉（Horse-Dragon-King）。
    #[inline]
    #[must_use]
    pub fn horse_dragon_king(&self) -> Bitboard {
        self.bitboards().horse_dragon_king()
    }

    /// 角・馬（BISHOP_HORSE）。
    #[inline]
    #[must_use]
    pub fn bishop_horse(&self) -> Bitboard {
        self.bitboards().bishop_horse()
    }

    /// 飛・龍（ROOK_DRAGON）。
    #[inline]
    #[must_use]
    pub fn rook_dragon(&self) -> Bitboard {
        self.bitboards().rook_dragon()
    }

    /// 指定色・複数駒種のビットボードを取得。
    #[must_use]
    pub fn pieces_for_types(&self, piece_types: &[PieceType], color: Color) -> Bitboard {
        let mut out = Bitboard::EMPTY;
        for &piece_type in piece_types {
            out |= self.bitboards().pieces_for(piece_type, color);
        }
        out
    }

    /// 指定色・複数駒種のビットボードを取得（配列版）。
    #[must_use]
    pub fn pieces_for_types_array<const N: usize>(
        &self,
        piece_types: [PieceType; N],
        color: Color,
    ) -> Bitboard {
        let mut out = Bitboard::EMPTY;
        for &piece_type in &piece_types {
            out |= self.bitboards().pieces_for(piece_type, color);
        }
        out
    }
}
