use super::Position;
use crate::board::state_info::{PartialKeys, StateInfo};
use crate::board::zobrist::{Zobrist, ZobristKey};
use crate::types::{Color, Hand, HandPiece, Piece, PieceType};
// `key_after_move` 系（`book` データ機能の内部専用）でのみ使う型。
#[cfg(feature = "book")]
use crate::types::{Move, MoveBase};

const MATERIAL_VALUES: [i32; PieceType::COUNT] = [
    0,      // なし
    90,     // 歩
    315,    // 香
    405,    // 桂
    495,    // 銀
    855,    // 角
    990,    // 飛
    540,    // 金
    15_000, // 王
    540,    // と金
    540,    // 成香
    540,    // 成桂
    540,    // 成銀
    945,    // 馬
    1_395,  // 龍
    0,      // 金相当
];

#[allow(clippy::struct_field_names)]
pub(in crate::board) struct KeySet {
    pub(in crate::board) board_key: ZobristKey,
    pub(in crate::board) hand_key: ZobristKey,
}

impl Position {
    /// `StateInfo` / current state に載せる部分ハッシュ・駒割り情報を完全再計算する。
    #[must_use]
    pub(crate) fn recompute_partial_keys(&self) -> PartialKeys {
        let mut keys = PartialKeys::default();

        for (sq, piece) in self.board.iter() {
            Self::add_board_piece_to_partial_keys(&mut keys, sq, piece);
        }

        for color in [Color::BLACK, Color::WHITE] {
            let hand = self.hand(color);
            for hp in HandPiece::iter() {
                let count = Hand::count_of(hand, hp);
                if count == 0 {
                    continue;
                }
                Self::add_hand_piece_material_value(
                    &mut keys,
                    color,
                    hp.to_piece_type(),
                    i32::try_from(count).expect("hand count fits in i32"),
                );
            }
        }

        keys
    }

    pub(crate) fn write_partial_keys_to_state(&self, state: &mut StateInfo) {
        state.set_partial_keys(self.recompute_partial_keys());
    }

    /// 盤上配置 + 手番の Zobrist キー（持ち駒は含まない）。
    #[must_use]
    pub const fn board_key(&self) -> ZobristKey {
        self.board_key
    }

    /// 持ち駒の Zobrist キー。
    #[must_use]
    pub const fn hand_key(&self) -> ZobristKey {
        self.hand_key
    }

    /// Zobrist ハッシュを完全計算する。
    pub(in crate::board) fn compute_keys(&self) -> KeySet {
        let mut board_key = ZobristKey::default();
        let mut hand_key = ZobristKey::default();

        // 盤上の駒
        for (sq, piece) in self.board.iter() {
            if !piece.is_empty() {
                board_key ^= Zobrist::psq(sq, piece);
            }
        }

        // 持ち駒
        for color in [Color::BLACK, Color::WHITE] {
            let hand = self.hand(color);
            for hp in 0..7 {
                let hp = HandPiece::new(hp);
                let count = Hand::count_of(hand, hp);
                if count > 0 {
                    hand_key.add(Zobrist::hand(color, hp.to_piece_type(), count as usize));
                }
            }
        }

        // 手番
        if self.turn() == Color::WHITE {
            board_key ^= Zobrist::side();
        }

        KeySet { board_key, hand_key }
    }

    /// 部分ハッシュ・駒割りキャッシュをまとめて返す。
    #[must_use]
    pub fn partial_keys(&self) -> PartialKeys {
        self.current_hot().partial_keys()
    }

    #[cfg(feature = "book")]
    fn board_key_after_impl(&self, mv: impl MoveBase) -> ZobristKey {
        let m = mv.to_move();
        let us = self.turn();
        let mut board_key = self.board_key ^ Zobrist::side();
        let to = m.to_sq();

        if m.is_drop() {
            let piece_type =
                m.dropped_piece().expect("drop move must include a dropped piece type");
            let piece = Piece::from_parts(us, piece_type);
            board_key ^= Zobrist::psq(to, piece);
            return board_key;
        }

        let from = m.from_sq();
        let moved_piece = self.piece_on(from);
        debug_assert!(moved_piece != Piece::NONE, "move must originate from a piece");
        let moved_after = if m.is_promotion() { moved_piece.promote() } else { moved_piece };

        let captured = self.piece_on(to);
        if captured != Piece::NONE {
            board_key ^= Zobrist::psq(to, captured);
        }

        board_key ^= Zobrist::psq(from, moved_piece);
        board_key ^= Zobrist::psq(to, moved_after);
        board_key
    }

    /// `Move` 版の `key_after`。
    ///
    /// `book` データ機能の内部でのみ使用するため `book` feature 有効時にのみコンパイルされる。
    #[cfg(feature = "book")]
    #[must_use]
    pub fn key_after_move(&self, mv: Move) -> ZobristKey {
        self.key_after_impl(mv)
    }

    #[cfg(feature = "book")]
    fn key_after_impl(&self, mv: impl MoveBase) -> ZobristKey {
        let m = mv.to_move();
        let us = self.turn();
        let board_key = self.board_key_after_impl(mv);
        let mut hand_key = self.hand_key;
        let to = m.to_sq();

        if m.is_drop() {
            let piece_type =
                m.dropped_piece().expect("drop move must include a dropped piece type");
            hand_key.sub(Zobrist::hand(us, piece_type, 1));
        } else {
            let captured = self.piece_on(to);
            if captured != Piece::NONE {
                hand_key.add(Zobrist::hand(us, captured.piece_type().demote(), 1));
            }
        }

        board_key ^ hand_key
    }

    #[inline]
    pub(super) fn add_board_piece_to_partial_keys(
        keys: &mut PartialKeys,
        sq: crate::types::Square,
        piece: Piece,
    ) {
        if piece == Piece::NONE {
            return;
        }

        Self::toggle_board_piece_partial_hashes(keys, sq, piece);
        keys.material.add(Zobrist::material(piece));
        keys.material_value += signed_material_value(piece);
    }

    #[inline]
    pub(super) fn remove_board_piece_from_partial_keys(
        keys: &mut PartialKeys,
        sq: crate::types::Square,
        piece: Piece,
    ) {
        if piece == Piece::NONE {
            return;
        }

        Self::toggle_board_piece_partial_hashes(keys, sq, piece);
        keys.material.sub(Zobrist::material(piece));
        keys.material_value -= signed_material_value(piece);
    }

    #[inline]
    pub(super) fn add_hand_piece_material_value(
        keys: &mut PartialKeys,
        color: Color,
        piece_type: PieceType,
        count: i32,
    ) {
        keys.material_value += color_sign(color) * piece_type_material_value(piece_type) * count;
    }

    #[inline]
    pub(super) fn sub_hand_piece_material_value(
        keys: &mut PartialKeys,
        color: Color,
        piece_type: PieceType,
        count: i32,
    ) {
        keys.material_value -= color_sign(color) * piece_type_material_value(piece_type) * count;
    }

    #[inline]
    pub(crate) fn debug_assert_partial_keys_consistent(&self) {
        #[cfg(debug_assertions)]
        {
            debug_assert_eq!(self.partial_keys(), self.recompute_partial_keys());
        }
    }
}

impl Position {
    #[inline]
    fn toggle_board_piece_partial_hashes(
        keys: &mut PartialKeys,
        sq: crate::types::Square,
        piece: Piece,
    ) {
        let pt = piece.piece_type();
        let psq = Zobrist::psq(sq, piece);

        if pt == PieceType::PAWN {
            keys.pawn ^= psq;
        }
        if pt.is_minor() {
            keys.minor ^= psq;
        }
        if pt != PieceType::PAWN {
            keys.non_pawn[piece.color().to_index()] ^= psq;
        }
    }
}

#[inline]
fn piece_type_material_value(piece_type: PieceType) -> i32 {
    MATERIAL_VALUES[piece_type.to_index()]
}

#[inline]
fn color_sign(color: Color) -> i32 {
    if color == Color::BLACK { 1 } else { -1 }
}

#[inline]
fn signed_material_value(piece: Piece) -> i32 {
    if piece == Piece::NONE {
        0
    } else {
        color_sign(piece.color()) * piece_type_material_value(piece.piece_type())
    }
}
