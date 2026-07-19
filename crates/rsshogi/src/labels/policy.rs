//! policy 学習向けの指し手ラベル変換。
//!
//! `MoveLabel` は「手番側から見た先手視点」に正規化した `27 * 81 = 2187` クラスのラベルである。
//! 後手番の指し手は盤を 180 度回転させてからラベル化する。
//!
//! `CompactMoveLabel` は 2187 クラスのうち、構造的に現れない 691 クラスを取り除いた
//! `1496` クラスの gapless なラベルである。これは「局面で合法」という意味ではなく、
//! 空盤面の移動パターン、成り、打ち制約から見て現れうるラベルのみを残したものである。
//!
//! ラベル配置自体は dlshogi で広く使われている 27x81 scheme と互換である。
//!
//! # Examples
//!
//! ```
//! use rsshogi::labels::policy::{CompactMoveLabel, MoveLabel, MoveLabelClass};
//! use rsshogi::types::{Color, Move, Square};
//!
//! let mv = Move::from_usi("7g7f").unwrap();
//! let label = MoveLabel::from_move(mv, Color::BLACK).unwrap();
//!
//! assert_eq!(label.class(), MoveLabelClass::Up);
//! assert_eq!(label.to_sq(), Square::from_usi("7f").unwrap());
//!
//! let compact = CompactMoveLabel::from_move(mv, Color::BLACK).unwrap();
//! assert_eq!(compact.expand(), label);
//! ```

use crate::types::square::flip;
use crate::types::{Color, Move, Move32, PieceType, Square};
use std::convert::TryFrom;

const LABELS_PER_CLASS: u16 = Square::COUNT as u16;
const MOVE_LABEL_COUNT_U16: u16 = 2187;
const COMPACT_MOVE_LABEL_COUNT_U16: u16 = 1496;

/// policy ラベルのクラス種別。
///
/// 通常移動 20 種と駒打ち 7 種を持つ。
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MoveLabelClass {
    Up = 0,
    UpLeft = 1,
    UpRight = 2,
    Left = 3,
    Right = 4,
    Down = 5,
    DownLeft = 6,
    DownRight = 7,
    Up2Left = 8,
    Up2Right = 9,
    UpPromote = 10,
    UpLeftPromote = 11,
    UpRightPromote = 12,
    LeftPromote = 13,
    RightPromote = 14,
    DownPromote = 15,
    DownLeftPromote = 16,
    DownRightPromote = 17,
    Up2LeftPromote = 18,
    Up2RightPromote = 19,
    DropPawn = 20,
    DropLance = 21,
    DropKnight = 22,
    DropSilver = 23,
    DropBishop = 24,
    DropRook = 25,
    DropGold = 26,
}

impl MoveLabelClass {
    /// ラベルクラス数。
    pub const COUNT: usize = 27;

    /// 生のクラス値を返す。
    #[inline]
    #[must_use]
    pub const fn raw(self) -> u8 {
        self as u8
    }

    /// 生のクラス値から生成する。
    #[inline]
    #[must_use]
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::Up),
            1 => Some(Self::UpLeft),
            2 => Some(Self::UpRight),
            3 => Some(Self::Left),
            4 => Some(Self::Right),
            5 => Some(Self::Down),
            6 => Some(Self::DownLeft),
            7 => Some(Self::DownRight),
            8 => Some(Self::Up2Left),
            9 => Some(Self::Up2Right),
            10 => Some(Self::UpPromote),
            11 => Some(Self::UpLeftPromote),
            12 => Some(Self::UpRightPromote),
            13 => Some(Self::LeftPromote),
            14 => Some(Self::RightPromote),
            15 => Some(Self::DownPromote),
            16 => Some(Self::DownLeftPromote),
            17 => Some(Self::DownRightPromote),
            18 => Some(Self::Up2LeftPromote),
            19 => Some(Self::Up2RightPromote),
            20 => Some(Self::DropPawn),
            21 => Some(Self::DropLance),
            22 => Some(Self::DropKnight),
            23 => Some(Self::DropSilver),
            24 => Some(Self::DropBishop),
            25 => Some(Self::DropRook),
            26 => Some(Self::DropGold),
            _ => None,
        }
    }

    /// 駒打ちクラスかどうかを返す。
    #[inline]
    #[must_use]
    pub const fn is_drop(self) -> bool {
        self.raw() >= Self::DropPawn.raw()
    }

    /// 成り移動クラスかどうかを返す。
    #[inline]
    #[must_use]
    pub const fn is_promotion(self) -> bool {
        let raw = self.raw();
        raw >= Self::UpPromote.raw() && raw <= Self::Up2RightPromote.raw()
    }

    /// 駒打ちクラスに対応する駒種を返す。
    #[inline]
    #[must_use]
    pub const fn drop_piece_type(self) -> Option<PieceType> {
        match self {
            Self::DropPawn => Some(PieceType::PAWN),
            Self::DropLance => Some(PieceType::LANCE),
            Self::DropKnight => Some(PieceType::KNIGHT),
            Self::DropSilver => Some(PieceType::SILVER),
            Self::DropBishop => Some(PieceType::BISHOP),
            Self::DropRook => Some(PieceType::ROOK),
            Self::DropGold => Some(PieceType::GOLD),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    const fn promoted(self) -> Self {
        match self {
            Self::Up => Self::UpPromote,
            Self::UpLeft => Self::UpLeftPromote,
            Self::UpRight => Self::UpRightPromote,
            Self::Left => Self::LeftPromote,
            Self::Right => Self::RightPromote,
            Self::Down => Self::DownPromote,
            Self::DownLeft => Self::DownLeftPromote,
            Self::DownRight => Self::DownRightPromote,
            Self::Up2Left => Self::Up2LeftPromote,
            Self::Up2Right => Self::Up2RightPromote,
            _ => self,
        }
    }

    #[inline]
    #[must_use]
    const fn mirror_file(self) -> Self {
        match self {
            Self::UpLeft => Self::UpRight,
            Self::UpRight => Self::UpLeft,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            Self::DownLeft => Self::DownRight,
            Self::DownRight => Self::DownLeft,
            Self::Up2Left => Self::Up2Right,
            Self::Up2Right => Self::Up2Left,
            Self::UpLeftPromote => Self::UpRightPromote,
            Self::UpRightPromote => Self::UpLeftPromote,
            Self::LeftPromote => Self::RightPromote,
            Self::RightPromote => Self::LeftPromote,
            Self::DownLeftPromote => Self::DownRightPromote,
            Self::DownRightPromote => Self::DownLeftPromote,
            Self::Up2LeftPromote => Self::Up2RightPromote,
            Self::Up2RightPromote => Self::Up2LeftPromote,
            other => other,
        }
    }

    #[inline]
    #[must_use]
    fn from_drop_piece_type(piece_type: PieceType) -> Option<Self> {
        match piece_type {
            PieceType::PAWN => Some(Self::DropPawn),
            PieceType::LANCE => Some(Self::DropLance),
            PieceType::KNIGHT => Some(Self::DropKnight),
            PieceType::SILVER => Some(Self::DropSilver),
            PieceType::BISHOP => Some(Self::DropBishop),
            PieceType::ROOK => Some(Self::DropRook),
            PieceType::GOLD => Some(Self::DropGold),
            _ => None,
        }
    }
}

/// 2187 クラスの policy move label。
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct MoveLabel(u16);

/// 構造的に現れうるラベルだけに圧縮した 1496 クラス label。
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct CompactMoveLabel(u16);

include!(concat!(env!("OUT_DIR"), "/policy_label_tables.rs"));

impl MoveLabel {
    /// move label 総数。
    pub const COUNT: usize = MOVE_LABEL_COUNT_U16 as usize;

    /// `Move` を policy ラベルへ変換する。
    ///
    /// `side_to_move` はその指し手を指す側の手番であり、後手なら盤面を 180 度回転して
    /// 先手視点に正規化する。
    #[must_use]
    pub fn from_move(mv: Move, side_to_move: Color) -> Option<Self> {
        if !mv.is_normal() {
            return None;
        }

        let mut to_sq = mv.to_sq();
        if mv.is_drop() {
            if side_to_move == Color::WHITE {
                to_sq = flip(to_sq);
            }
            let class = MoveLabelClass::from_drop_piece_type(mv.dropped_piece()?)?;
            return Some(Self::from_parts(class, to_sq));
        }

        let mut from_sq = mv.from_sq();
        if side_to_move == Color::WHITE {
            to_sq = flip(to_sq);
            from_sq = flip(from_sq);
        }

        let mut class = classify_normal_move(from_sq, to_sq);
        if mv.is_promotion() {
            class = class.promoted();
        }

        Some(Self::from_parts(class, to_sq))
    }

    /// `Move32` を policy ラベルへ変換する。
    #[inline]
    #[must_use]
    pub fn from_move32(mv: Move32, side_to_move: Color) -> Option<Self> {
        Self::from_move(mv.to_move(), side_to_move)
    }

    /// 生値から生成する。
    #[inline]
    #[must_use]
    pub const fn from_raw(raw: u16) -> Option<Self> {
        if raw < MOVE_LABEL_COUNT_U16 { Some(Self(raw)) } else { None }
    }

    /// 生値を返す。
    #[inline]
    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }

    /// ラベルクラスを返す。
    #[inline]
    #[must_use]
    pub fn class(self) -> MoveLabelClass {
        MoveLabelClass::from_raw((self.0 / LABELS_PER_CLASS) as u8).expect("valid move label class")
    }

    /// 先手視点に正規化された移動先マスを返す。
    #[inline]
    #[must_use]
    pub const fn to_sq(self) -> Square {
        Square::new((self.0 % LABELS_PER_CLASS) as i8)
    }

    /// 構造的に現れうるラベルかどうかを返す。
    #[inline]
    #[must_use]
    pub fn is_structurally_valid(self) -> bool {
        MOVE_LABEL_TO_COMPACT[self.0 as usize].is_some()
    }

    /// 1496 クラスの compact label へ変換する。
    #[inline]
    #[must_use]
    pub fn compact(self) -> Option<CompactMoveLabel> {
        MOVE_LABEL_TO_COMPACT[self.0 as usize].map(CompactMoveLabel)
    }

    /// 盤面を左右反転した move label を返す。
    ///
    /// 移動先の file と左右方向を反転し、成り属性と駒打ちの駒種は維持する。
    #[inline]
    #[must_use]
    pub fn mirror_file(self) -> Self {
        Self::from_parts(self.class().mirror_file(), self.to_sq().mirror_file())
    }

    #[inline]
    #[must_use]
    const fn from_parts(class: MoveLabelClass, to_sq: Square) -> Self {
        Self((class.raw() as u16) * LABELS_PER_CLASS + to_sq.raw() as u16)
    }
}

impl CompactMoveLabel {
    /// compact label 総数。
    pub const COUNT: usize = COMPACT_MOVE_LABEL_COUNT_U16 as usize;

    /// `Move` を compact label へ変換する。
    #[inline]
    #[must_use]
    pub fn from_move(mv: Move, side_to_move: Color) -> Option<Self> {
        MoveLabel::from_move(mv, side_to_move)?.compact()
    }

    /// `Move32` を compact label へ変換する。
    #[inline]
    #[must_use]
    pub fn from_move32(mv: Move32, side_to_move: Color) -> Option<Self> {
        MoveLabel::from_move32(mv, side_to_move)?.compact()
    }

    /// 生値から生成する。
    #[inline]
    #[must_use]
    pub const fn from_raw(raw: u16) -> Option<Self> {
        if raw < COMPACT_MOVE_LABEL_COUNT_U16 { Some(Self(raw)) } else { None }
    }

    /// 生値を返す。
    #[inline]
    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }

    /// 対応する 2187 クラスの move label へ戻す。
    #[inline]
    #[must_use]
    pub fn expand(self) -> MoveLabel {
        MoveLabel(COMPACT_TO_MOVE_LABEL[self.0 as usize])
    }

    /// 盤面を左右反転した compact move label を返す。
    ///
    /// compact policy 空間を保つ全単射であり、2 回適用すると元の label に戻る。
    #[inline]
    #[must_use]
    pub fn mirror_file(self) -> Self {
        self.expand().mirror_file().compact().expect("mirroring preserves structural validity")
    }
}

impl From<CompactMoveLabel> for MoveLabel {
    #[inline]
    fn from(value: CompactMoveLabel) -> Self {
        value.expand()
    }
}

impl TryFrom<MoveLabel> for CompactMoveLabel {
    type Error = MoveLabel;

    #[inline]
    fn try_from(value: MoveLabel) -> Result<Self, Self::Error> {
        value.compact().ok_or(value)
    }
}

fn classify_normal_move(from_sq: Square, to_sq: Square) -> MoveLabelClass {
    let from_x = from_sq.file().raw();
    let from_y = from_sq.rank().raw();
    let to_x = to_sq.file().raw();
    let to_y = to_sq.rank().raw();

    let dir_x = from_x - to_x;
    let dir_y = to_y - from_y;

    if dir_y < 0 && dir_x == 0 {
        MoveLabelClass::Up
    } else if dir_y == -2 && dir_x == -1 {
        MoveLabelClass::Up2Left
    } else if dir_y == -2 && dir_x == 1 {
        MoveLabelClass::Up2Right
    } else if dir_y < 0 && dir_x < 0 {
        MoveLabelClass::UpLeft
    } else if dir_y < 0 && dir_x > 0 {
        MoveLabelClass::UpRight
    } else if dir_y == 0 && dir_x < 0 {
        MoveLabelClass::Left
    } else if dir_y == 0 && dir_x > 0 {
        MoveLabelClass::Right
    } else if dir_y > 0 && dir_x == 0 {
        MoveLabelClass::Down
    } else if dir_y > 0 && dir_x < 0 {
        MoveLabelClass::DownLeft
    } else {
        MoveLabelClass::DownRight
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{self, MoveList};
    use crate::movegen::generate_legal_all;

    fn reference_make_move_label(mv: Move, side_to_move: Color) -> u16 {
        let mut to_sq = mv.to_sq();

        if mv.is_drop() {
            if side_to_move == Color::WHITE {
                to_sq = flip(to_sq);
            }
            let class = MoveLabelClass::from_drop_piece_type(
                mv.dropped_piece().expect("drop move should have dropped piece"),
            )
            .expect("drop piece should map to a label class");
            return u16::from(class.raw()) * LABELS_PER_CLASS + to_sq.raw() as u16;
        }

        let mut from_sq = mv.from_sq();
        if side_to_move == Color::WHITE {
            to_sq = flip(to_sq);
            from_sq = flip(from_sq);
        }

        let mut class = classify_normal_move(from_sq, to_sq);
        if mv.is_promotion() {
            class = class.promoted();
        }

        u16::from(class.raw()) * LABELS_PER_CLASS + to_sq.raw() as u16
    }

    fn assert_labels_match_reference_for_all_legal_moves(sfen: &str) {
        board::init();
        let pos = board::position_from_sfen(sfen).expect("sfen should parse");
        let mut moves = MoveList::new();
        generate_legal_all(&pos, &mut moves);
        assert!(!moves.is_empty(), "position should have legal moves");

        for mv in moves.iter().copied() {
            let label = MoveLabel::from_move(mv, pos.turn()).expect("legal move should encode");
            assert_eq!(
                label.raw(),
                reference_make_move_label(mv, pos.turn()),
                "reference mismatch for {} in {}",
                mv.to_usi(),
                sfen
            );
            assert!(label.is_structurally_valid(), "legal move label should be structurally valid");

            let compact = label.compact().expect("legal move label should compact");
            assert_eq!(compact.expand(), label);
            assert_eq!(CompactMoveLabel::from_move(mv, pos.turn()), Some(compact));

            let mv32 = pos.move32_from_move(mv);
            assert_eq!(MoveLabel::from_move32(mv32, pos.turn()), Some(label));
            assert_eq!(CompactMoveLabel::from_move32(mv32, pos.turn()), Some(compact));
        }
    }

    #[test]
    fn structural_label_tables_are_consistent() {
        assert_eq!(MoveLabel::COUNT, 2187);
        assert_eq!(CompactMoveLabel::COUNT, 1496);
        assert_eq!(COMPACT_TO_MOVE_LABEL.len(), CompactMoveLabel::COUNT);
        assert_eq!(MOVE_LABEL_TO_COMPACT.len(), MoveLabel::COUNT);
        assert_eq!(MOVE_LABEL_TO_COMPACT.iter().flatten().count(), CompactMoveLabel::COUNT);
        assert_eq!(MOVE_LABEL_TO_COMPACT.iter().filter(|value| value.is_none()).count(), 691);

        for (compact_raw, &move_label_raw) in COMPACT_TO_MOVE_LABEL.iter().enumerate() {
            let move_label = MoveLabel::from_raw(move_label_raw).expect("generated move label");
            assert!(move_label.is_structurally_valid());
            assert_eq!(
                MOVE_LABEL_TO_COMPACT[usize::from(move_label_raw)],
                Some(u16::try_from(compact_raw).expect("compact label index fits in u16"))
            );
            assert_eq!(
                move_label.compact().expect("generated move label should compact").raw(),
                u16::try_from(compact_raw).expect("compact label index fits in u16")
            );
        }
    }

    #[test]
    fn move_label_class_helpers_work() {
        assert!(MoveLabelClass::DropKnight.is_drop());
        assert_eq!(MoveLabelClass::DropKnight.drop_piece_type(), Some(PieceType::KNIGHT));
        assert!(MoveLabelClass::UpLeftPromote.is_promotion());
        assert!(!MoveLabelClass::Left.is_promotion());
        assert_eq!(MoveLabelClass::from_raw(26), Some(MoveLabelClass::DropGold));
        assert_eq!(MoveLabelClass::from_raw(27), None);
    }

    #[test]
    fn move_label_file_mirror_is_total_structural_and_involutive() {
        for raw in 0..MoveLabel::COUNT {
            let label = MoveLabel::from_raw(u16::try_from(raw).expect("label fits in u16"))
                .expect("raw label is in range");
            let mirrored = label.mirror_file();

            assert_eq!(mirrored.mirror_file(), label, "involution failed for {raw}");
            assert_eq!(
                mirrored.is_structurally_valid(),
                label.is_structurally_valid(),
                "structural validity changed for {raw}"
            );
            assert_eq!(mirrored.class().is_drop(), label.class().is_drop());
            assert_eq!(mirrored.class().is_promotion(), label.class().is_promotion());

            if let Some(compact) = label.compact() {
                assert_eq!(mirrored.compact(), Some(compact.mirror_file()));
            }
        }
    }

    #[test]
    fn compact_move_label_file_mirror_is_total_and_involutive() {
        for raw in 0..CompactMoveLabel::COUNT {
            let label = CompactMoveLabel::from_raw(u16::try_from(raw).expect("label fits in u16"))
                .expect("raw compact label is in range");
            let mirrored = label.mirror_file();

            assert_eq!(mirrored.mirror_file(), label, "involution failed for {raw}");
            assert_eq!(mirrored.expand(), label.expand().mirror_file());
            assert!(mirrored.raw() < CompactMoveLabel::COUNT as u16);
        }
    }

    #[test]
    fn file_mirror_preserves_drop_piece_and_promotion() {
        let pawn_drop =
            MoveLabel::from_move(Move::from_usi("P*3d").expect("valid drop"), Color::BLACK)
                .expect("drop should encode");
        let mirrored_pawn_drop =
            MoveLabel::from_move(Move::from_usi("P*7d").expect("valid drop"), Color::BLACK)
                .expect("drop should encode");
        assert_eq!(pawn_drop.mirror_file(), mirrored_pawn_drop);
        assert_eq!(pawn_drop.mirror_file().class(), MoveLabelClass::DropPawn);

        let promotion =
            MoveLabel::from_move(Move::from_usi("8h2b+").expect("valid promotion"), Color::BLACK)
                .expect("promotion should encode");
        let mirrored_promotion =
            MoveLabel::from_move(Move::from_usi("2h8b+").expect("valid promotion"), Color::BLACK)
                .expect("promotion should encode");
        assert_eq!(promotion.mirror_file(), mirrored_promotion);
        assert!(promotion.mirror_file().class().is_promotion());
    }

    #[test]
    fn invalid_move_patterns_do_not_compact() {
        let pawn_drop_last_rank =
            MoveLabel::from_move(Move::from_usi("P*5a").expect("valid usi"), Color::BLACK)
                .expect("2187 label should exist");
        assert_eq!(pawn_drop_last_rank.class(), MoveLabelClass::DropPawn);
        assert!(!pawn_drop_last_rank.is_structurally_valid());
        assert_eq!(pawn_drop_last_rank.compact(), None);

        let unpromoted_knight_first_rank =
            MoveLabel::from_move(Move::from_usi("5c4a").expect("valid usi"), Color::BLACK)
                .expect("2187 label should exist");
        assert_eq!(unpromoted_knight_first_rank.class(), MoveLabelClass::Up2Right);
        assert!(!unpromoted_knight_first_rank.is_structurally_valid());
        assert_eq!(unpromoted_knight_first_rank.compact(), None);
    }

    #[test]
    fn white_moves_are_rotated_to_black_perspective() {
        let label =
            MoveLabel::from_move(Move::from_usi("3c3d").expect("valid usi"), Color::WHITE).unwrap();
        assert_eq!(label.class(), MoveLabelClass::Up);
        assert_eq!(label.to_sq(), flip(Square::from_usi("3d").expect("valid square")));
    }

    #[test]
    fn special_moves_do_not_encode() {
        assert_eq!(MoveLabel::from_move(Move::MOVE_NULL, Color::BLACK), None);
        assert_eq!(MoveLabel::from_move(Move::MOVE_RESIGN, Color::BLACK), None);
    }

    #[test]
    fn legal_moves_match_reference_label_encoding() {
        assert_labels_match_reference_for_all_legal_moves(
            "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        );
        assert_labels_match_reference_for_all_legal_moves(
            "l6nl/5+P1gk/2np1S3/p1p4Pp/3P2Sp1/1PPb2P1P/P5GS1/R8/LN4bKL w GR5pnsg 1",
        );
        assert_labels_match_reference_for_all_legal_moves("4k4/4P4/9/9/9/9/9/9/4K4 b - 1");
        assert_labels_match_reference_for_all_legal_moves("4k4/9/9/9/9/9/9/4p4/3K5 w - 1");
    }
}
