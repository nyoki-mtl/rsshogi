use super::Position;
use crate::board::state_info::{PartialKeys, StateInfo};
use crate::board::zobrist::{Zobrist, ZobristKey};
use crate::types::{Color, Hand, HandPiece, Move, Move32, MoveBase, Piece, PieceType};

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

pub(in crate::board) struct KeySet {
    pub(in crate::board) board_key: ZobristKey,
    pub(in crate::board) key: ZobristKey,
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
    #[inline]
    #[must_use]
    pub fn board_key(&self) -> ZobristKey {
        self.current_hot().board_key()
    }

    /// Zobrist ハッシュを完全計算する。
    ///
    /// 差分更新を検証する全再計算 oracle であり、局面の再構築時にのみ呼ぶ。
    pub(in crate::board) fn compute_keys(&self) -> KeySet {
        let mut board_key = ZobristKey::default();
        let mut hand_contribution = ZobristKey::default();

        // 盤上の駒
        for (sq, piece) in self.board.iter() {
            if !piece.is_empty() {
                board_key ^= Zobrist::psq(sq, piece);
            }
        }

        // 持ち駒
        for color in [Color::BLACK, Color::WHITE] {
            let hand = self.hand(color);
            for hp in HandPiece::iter() {
                // 枚数 0 のキーはゼロなので読み飛ばしてよい。
                let count = Hand::count_of(hand, hp);
                if count > 0 {
                    hand_contribution ^= Zobrist::hand(color, hp.to_piece_type(), count);
                }
            }
        }

        // 手番
        if self.turn() == Color::WHITE {
            board_key ^= Zobrist::side();
        }

        KeySet { board_key, key: board_key ^ hand_contribution }
    }

    /// 部分ハッシュ・駒割りキャッシュをまとめて返す。
    #[must_use]
    pub fn partial_keys(&self) -> PartialKeys {
        self.current_hot().partial_keys()
    }

    /// `mv` を適用した後の盤面キー（持ち駒を含まない）を、局面を変更せずに求める。
    ///
    /// 千日手判定用の第 2 キーを先読みしたい消費者向け。局面全体のキーは
    /// [`Position::key_after`] を使う。
    /// `mv` は現局面の合法手でなければならない。合法性は検査しない。
    #[inline]
    #[must_use]
    pub fn board_key_after(&self, mv: Move32) -> ZobristKey {
        self.board_key_after_impl(mv)
    }

    /// `Move` 版の [`Position::board_key_after`]。
    #[inline]
    #[must_use]
    pub fn board_key_after_move(&self, mv: Move) -> ZobristKey {
        self.board_key_after_impl(mv)
    }

    fn board_key_after_impl(&self, mv: impl MoveBase) -> ZobristKey {
        let m = mv.to_move();
        let us = self.turn();
        let mut board_key = self.board_key() ^ Zobrist::side();
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

    /// `mv` を適用した後の局面キーを、局面を変更せずに求める。
    ///
    /// 主な用途は探索エンジンの置換表 prefetch で、
    /// `key_after` でキーを確定 → engine 側で TT を prefetch → [`Position::apply_move32`]
    /// の順に使う。prefetch の発行 API は本 crate には置かない
    /// （置換表は engine の所有物であり、rsshogi は prefetch すべきアドレスを知り得ない）。
    ///
    /// 戻り値は `mv` を適用した後の [`Position::key`] と一致する。
    /// `mv` は現局面の合法手でなければならない。合法性は検査しないため、
    /// 非合法手を渡した場合の戻り値は意味を持たない。
    #[inline]
    #[must_use]
    pub fn key_after(&self, mv: Move32) -> ZobristKey {
        self.key_after_impl(mv)
    }

    /// `Move` 版の [`Position::key_after`]。
    #[inline]
    #[must_use]
    pub fn key_after_move(&self, mv: Move) -> ZobristKey {
        self.key_after_impl(mv)
    }

    /// null move（手番のみ反転）を適用した後の局面キー。
    ///
    /// 盤上の駒も持ち駒も動かないため、手番キーとの XOR だけで求まる。
    #[inline]
    #[must_use]
    pub fn key_after_null(&self) -> ZobristKey {
        self.key() ^ Zobrist::side()
    }

    fn key_after_impl(&self, mv: impl MoveBase) -> ZobristKey {
        let m = mv.to_move();
        let us = self.turn();
        // 盤面差分は board_key と key の両方に効く。board_key_after_impl が
        // 前者を返すので、その差分をそのまま合成キーへ移す。
        let board_key = self.board_key_after_impl(mv);
        let mut key = self.key() ^ self.board_key() ^ board_key;
        let to = m.to_sq();

        if m.is_drop() {
            let piece_type =
                m.dropped_piece().expect("drop move must include a dropped piece type");
            let hand_piece =
                HandPiece::from_piece_type(piece_type).expect("drop move must use a hand piece");
            let before = self.hand(us).count(hand_piece);
            debug_assert!(before > 0, "drop move must have a matching hand piece");
            key ^= Zobrist::hand_delta(us, piece_type, before, before - 1);
        } else {
            let captured = self.piece_on(to);
            if captured != Piece::NONE {
                let piece_type = captured.piece_type().demote();
                let hand_piece = HandPiece::from_piece_type(piece_type)
                    .expect("captured piece must map to hand");
                let before = self.hand(us).count(hand_piece);
                key ^= Zobrist::hand_delta(us, piece_type, before, before + 1);
            }
        }

        key
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
