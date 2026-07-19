//! 指し手の型定義
//!
//! このモジュールは将棋の指し手を表現する型を提供する。
//!
//! # 指し手の種類
//!
//! - [`Move32`] - 32bit 指し手（完全な情報を保持）
//! - [`Move`] - 16bit 指し手（コンパクト版）
//!
//! # Move32 vs Move
//!
//! | 特徴 | Move32 (32bit) | Move (16bit) |
//! |------|:----------:|:--------------:|
//! | 移動元 | ○ | ○ |
//! | 移動先 | ○ | ○ |
//! | 成り | ○ | ○ |
//! | 駒打ち | ○ | ○ |
//! | 取った駒 | ○ | - |
//! | 動かした駒 | ○ | - |
//!
//! `Move` は探索中の手のリストなど、メモリ効率が重要な場面で使用する。
//! `Move32` は取った駒の情報が必要な場面（undo など）で使用する。
//!
//! # USI 形式
//!
//! USI プロトコルの指し手文字列との相互変換をサポートする。
//!
//! ```
//! use rsshogi::types::Move32;
//!
//! // USI 文字列から Move32 を作成
//! let m = Move32::from_usi("7g7f").unwrap();
//!
//! // Move32 から USI 文字列を取得
//! assert_eq!(m.to_usi(), "7g7f");
//! ```
//!
//! # 駒打ち
//!
//! 駒打ちは `P*5e` のような形式で表現する。
//!
//! ```
//! use rsshogi::types::Move32;
//!
//! let drop = Move32::from_usi("P*5e").unwrap();
//! assert!(drop.is_drop());
//! ```
//!
//! # 特殊な指し手
//!
//! | 定数 | 説明 |
//! |------|------|
//! | `MOVE_NONE` | 無効な指し手 |
//! | `MOVE_NULL` | パス（null move） |
//! | `MOVE_RESIGN` | 投了 |
//! | `MOVE_WIN` | 宣言勝ち |

#![allow(clippy::inline_always)]

use super::{Color, Piece, PieceType, Square};
use crate::board::Position;
use std::convert::TryFrom;
use std::fmt;

// --- sealed MoveBase trait ---

mod sealed {
    pub trait Sealed {}
}

/// `Move` と `Move32` の共通インターフェース。
///
/// 内部の共通ロジックをジェネリック化するためのトレイト。
/// sealed パターンにより外部クレートでの実装を防止する。
pub(crate) trait MoveBase: Copy + sealed::Sealed {
    /// 下位16bit（`Move` 相当）を取得する。
    fn to_move(self) -> Move;

    /// 移動後の駒情報のヒントを返す。
    ///
    /// - `Move`: 常に `Piece::NONE`（駒情報なし）
    /// - `Move32`: `piece_after_move()` の値（完全 Move32 なら有効な駒、部分 Move32 なら `Piece::NONE`）
    fn piece_after_hint(self) -> Piece;
}

impl sealed::Sealed for Move {}

impl MoveBase for Move {
    #[inline]
    fn to_move(self) -> Move {
        self
    }

    #[inline]
    fn piece_after_hint(self) -> Piece {
        Piece::NONE
    }
}

// ANCHOR: move_struct
/// 16bit指し手表現
/// ビットフィールド構成:
/// - bit0..6: 移動先 (to)
/// - bit7..13: 移動元 (from) または打つ駒種
/// - bit14: 駒打ちフラグ (`MOVE_DROP`)
/// - bit15: 成りフラグ (`MOVE_PROMOTE`)
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct Move(u16);

const _: () = {
    assert!(core::mem::size_of::<Move>() == 2);
    assert!(core::mem::align_of::<Move>() == 2);
};
// ANCHOR_END: move_struct

// ANCHOR: move_special_constants
// 定数定義
// Move32用
pub const MOVE_NONE: Move32 = Move32(0);
pub const MOVE_NULL: Move32 = Move32(((1 << 7) + 1) as u32);
pub const MOVE_RESIGN: Move32 = Move32(((2 << 7) + 2) as u32);
pub const MOVE_WIN: Move32 = Move32(((3 << 7) + 3) as u32);
pub const MOVE_END: Move32 = Move32(((4 << 7) + 4) as u32);
// ANCHOR_END: move_special_constants

// ANCHOR: move_type_enum
/// 指し手種別
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MoveType {
    Normal,
    Promotion,
    Drop,
}
// ANCHOR_END: move_type_enum

/// `Move32` に付随する軽量メタデータ。
///
/// `Move32` 自身が持つ情報（移動後駒、成り、打ち）に加え、
/// 局面依存の捕獲情報をまとめて扱うために使う。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Move32Metadata {
    piece_after: Piece,
    captured_piece: Piece,
    is_capture: bool,
    is_promotion: bool,
    is_drop: bool,
    from_to_index_plus_one: u16,
}

const _: () = {
    assert!(core::mem::size_of::<Move32Metadata>() == 8);
    assert!(core::mem::align_of::<Move32Metadata>() == 2);
};

impl Move32Metadata {
    const NO_FROM_TO_INDEX: u16 = 0;

    #[must_use]
    pub const fn new(
        piece_after: Piece,
        captured_piece: Piece,
        is_capture: bool,
        is_promotion: bool,
        is_drop: bool,
    ) -> Self {
        Self {
            piece_after,
            captured_piece,
            is_capture,
            is_promotion,
            is_drop,
            from_to_index_plus_one: Self::NO_FROM_TO_INDEX,
        }
    }

    #[must_use]
    pub const fn with_from_to_index(
        piece_after: Piece,
        captured_piece: Piece,
        is_capture: bool,
        is_promotion: bool,
        is_drop: bool,
        from_to_index: Option<usize>,
    ) -> Self {
        Self {
            piece_after,
            captured_piece,
            is_capture,
            is_promotion,
            is_drop,
            from_to_index_plus_one: Self::encode_from_to_index(from_to_index),
        }
    }

    #[must_use]
    pub const fn piece_after_move(self) -> Piece {
        self.piece_after
    }

    #[must_use]
    pub const fn captured_piece(self) -> Piece {
        self.captured_piece
    }

    #[must_use]
    pub const fn captured_piece_type(self) -> PieceType {
        self.captured_piece.piece_type()
    }

    #[must_use]
    pub const fn is_capture(self) -> bool {
        self.is_capture
    }

    #[must_use]
    pub const fn is_promotion(self) -> bool {
        self.is_promotion
    }

    #[must_use]
    pub const fn is_drop(self) -> bool {
        self.is_drop
    }

    /// history 系テーブル向けの from/to index を返す。
    ///
    /// [`Position::move32_metadata()`] / [`Position::classify_move32()`] で
    /// 作られた metadata では、元の `Move32` と同じ index が取得できる。
    /// [`Move32Metadata::new()`] で直接構築した場合は `None`。
    #[must_use]
    pub const fn from_to_index(self) -> Option<usize> {
        Self::decode_from_to_index(self.from_to_index_plus_one)
    }

    #[must_use]
    const fn encode_from_to_index(from_to_index: Option<usize>) -> u16 {
        match from_to_index {
            Some(index) if index < u16::MAX as usize => index as u16 + 1,
            _ => Self::NO_FROM_TO_INDEX,
        }
    }

    #[must_use]
    const fn decode_from_to_index(from_to_index_plus_one: u16) -> Option<usize> {
        if from_to_index_plus_one == Self::NO_FROM_TO_INDEX {
            None
        } else {
            Some((from_to_index_plus_one - 1) as usize)
        }
    }
}

/// cshogi / Apery 互換の 16bit 指し手表現。
///
/// ビットフィールド構成:
/// - bit0..6: 移動先 (to)
/// - bit7..13: 移動元 (from) または打つ駒種 + 80
/// - bit14: 成りフラグ
/// - 駒打ちは `from >= 81` で判定する
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct AperyMove(u16);

const _: () = {
    assert!(core::mem::size_of::<AperyMove>() == 2);
    assert!(core::mem::align_of::<AperyMove>() == 2);
};

/// cshogi / Apery 互換の 32bit 指し手表現。
///
/// - 下位16bit: [`AperyMove`]
/// - bit16..19: 移動前の `PieceType`
/// - bit20..23: 取得駒の `PieceType`
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct AperyMove32(u32);

const _: () = {
    assert!(core::mem::size_of::<AperyMove32>() == 4);
    assert!(core::mem::align_of::<AperyMove32>() == 4);
};

impl MoveType {
    #[inline]
    #[must_use]
    pub const fn from_bits(bits: u16) -> Self {
        match bits {
            Move::MOVE_PROMOTE => Self::Promotion,
            Move::MOVE_DROP => Self::Drop,
            _ => Self::Normal,
        }
    }
}

impl AperyMove {
    const TO_MASK: u16 = 0x7f;
    const FROM_MASK: u16 = 0x7f;
    const FROM_SHIFT: u16 = 7;
    const PROMOTE: u16 = 1 << 14;
    const DROP_FROM_BASE: u16 = 80;

    #[inline]
    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }

    #[inline]
    #[must_use]
    pub const fn from_raw(raw: u16) -> Self {
        Self(raw)
    }

    #[inline]
    #[must_use]
    pub const fn from_move(mv: Move) -> Self {
        let raw = mv.raw();
        if raw == Move::MOVE_NONE.raw()
            || raw == Move::MOVE_NULL.raw()
            || raw == Move::MOVE_RESIGN.raw()
            || raw == Move::MOVE_WIN.raw()
            || raw == Move::MOVE_END.raw()
        {
            // rsshogi のセンチネル値（`MOVE_NONE`、`MOVE_NULL` 等）はそのまま通す。
            return Self(raw);
        }
        let to = raw & Self::TO_MASK;
        let mut from = (raw >> Self::FROM_SHIFT) & Self::FROM_MASK;
        if (raw & Move::MOVE_DROP) != 0 {
            from += Self::DROP_FROM_BASE;
        }
        let promote = if (raw & Move::MOVE_PROMOTE) != 0 { Self::PROMOTE } else { 0 };
        Self(to | (from << Self::FROM_SHIFT) | promote)
    }

    #[inline]
    #[must_use]
    pub const fn to_move(self) -> Move {
        let to = self.0 & Self::TO_MASK;
        let from = (self.0 >> Self::FROM_SHIFT) & Self::FROM_MASK;
        let drop = from >= 81;
        let move_from = if drop { from - Self::DROP_FROM_BASE } else { from };
        let promote = if (self.0 & Self::PROMOTE) != 0 { Move::MOVE_PROMOTE } else { 0 };
        let drop_flag = if drop { Move::MOVE_DROP } else { 0 };
        Move::from_raw(to | (move_from << Self::FROM_SHIFT) | promote | drop_flag)
    }

    #[inline]
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn from_sq(self) -> Square {
        if self.is_drop() {
            Square::NONE
        } else {
            Square::new(((self.0 >> Self::FROM_SHIFT) & Self::FROM_MASK) as i8)
        }
    }

    #[inline]
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn to_sq(self) -> Square {
        Square::new((self.0 & Self::TO_MASK) as i8)
    }

    #[inline]
    #[must_use]
    pub const fn is_drop(self) -> bool {
        ((self.0 >> Self::FROM_SHIFT) & Self::FROM_MASK) >= 81
    }

    #[inline]
    #[must_use]
    pub const fn is_promotion(self) -> bool {
        (self.0 & Self::PROMOTE) != 0
    }

    #[must_use]
    pub const fn move_type(self) -> MoveType {
        if self.is_drop() {
            MoveType::Drop
        } else if self.is_promotion() {
            MoveType::Promotion
        } else {
            MoveType::Normal
        }
    }

    #[must_use]
    pub const fn dropped_piece(self) -> Option<PieceType> {
        if self.is_drop() {
            let from = ((self.0 >> Self::FROM_SHIFT) & Self::FROM_MASK) as i8;
            Some(PieceType::new(from - Self::DROP_FROM_BASE as i8))
        } else {
            None
        }
    }

    #[must_use]
    pub fn is_normal(self) -> bool {
        self.to_move().is_normal()
    }

    #[must_use]
    pub fn from_usi(s: &str) -> Option<Self> {
        Move::from_usi(s).map(Self::from_move)
    }

    #[must_use]
    pub fn to_usi(self) -> String {
        self.to_move().to_usi()
    }
}

impl Default for AperyMove {
    fn default() -> Self {
        Self::from_raw(0)
    }
}

impl fmt::Display for AperyMove {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_usi())
    }
}

impl AperyMove32 {
    const APERY_MOVE_MASK: u32 = 0xffff;
    const PIECE_TYPE_MASK: u32 = 0xf;
    const PIECE_TYPE_SHIFT: u32 = 16;
    const CAPTURED_SHIFT: u32 = 20;

    #[inline]
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }

    #[inline]
    #[must_use]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    #[inline]
    #[must_use]
    pub const fn to_apery_move(self) -> AperyMove {
        AperyMove::from_raw((self.0 & Self::APERY_MOVE_MASK) as u16)
    }

    #[inline]
    #[must_use]
    pub const fn to_move(self) -> Move {
        self.to_apery_move().to_move()
    }

    #[inline]
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn piece_type_before(self) -> PieceType {
        PieceType::new(((self.0 >> Self::PIECE_TYPE_SHIFT) & Self::PIECE_TYPE_MASK) as i8)
    }

    #[inline]
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn captured_piece_type(self) -> PieceType {
        PieceType::new(((self.0 >> Self::CAPTURED_SHIFT) & Self::PIECE_TYPE_MASK) as i8)
    }

    #[inline]
    #[must_use]
    pub fn is_capture(self) -> bool {
        self.captured_piece_type() != PieceType::NONE
    }

    #[inline]
    #[must_use]
    pub const fn is_drop(self) -> bool {
        self.to_apery_move().is_drop()
    }

    #[inline]
    #[must_use]
    pub const fn is_promotion(self) -> bool {
        self.to_apery_move().is_promotion()
    }

    #[inline]
    #[must_use]
    pub const fn move_type(self) -> MoveType {
        self.to_apery_move().move_type()
    }

    #[inline]
    #[must_use]
    pub const fn from_sq(self) -> Square {
        self.to_apery_move().from_sq()
    }

    #[inline]
    #[must_use]
    pub const fn to_sq(self) -> Square {
        self.to_apery_move().to_sq()
    }

    #[inline]
    #[must_use]
    pub const fn dropped_piece(self) -> Option<PieceType> {
        self.to_apery_move().dropped_piece()
    }

    #[must_use]
    pub const fn piece_type_after(self) -> PieceType {
        if let Some(piece_type) = self.dropped_piece() {
            return piece_type;
        }
        let piece_type = self.piece_type_before();
        if self.is_promotion() { piece_type.promote() } else { piece_type }
    }

    #[must_use]
    pub fn is_normal(self) -> bool {
        self.to_move().is_normal()
    }

    #[must_use]
    pub fn to_usi(self) -> String {
        self.to_move().to_usi()
    }
}

impl Default for AperyMove32 {
    fn default() -> Self {
        Self::from_raw(0)
    }
}

impl fmt::Display for AperyMove32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_usi())
    }
}

fn piece_type_to_csa(piece_type: PieceType) -> Option<&'static str> {
    match piece_type {
        PieceType::PAWN => Some("FU"),
        PieceType::LANCE => Some("KY"),
        PieceType::KNIGHT => Some("KE"),
        PieceType::SILVER => Some("GI"),
        PieceType::GOLD => Some("KI"),
        PieceType::BISHOP => Some("KA"),
        PieceType::ROOK => Some("HI"),
        PieceType::KING => Some("OU"),
        PieceType::PRO_PAWN => Some("TO"),
        PieceType::PRO_LANCE => Some("NY"),
        PieceType::PRO_KNIGHT => Some("NK"),
        PieceType::PRO_SILVER => Some("NG"),
        PieceType::HORSE => Some("UM"),
        PieceType::DRAGON => Some("RY"),
        _ => None,
    }
}

fn square_to_csa(sq: Square) -> Option<String> {
    if !sq.is_on_board() {
        return None;
    }
    let file = sq.file();
    let rank = sq.rank();
    if !file.is_valid() || !rank.is_valid() {
        return None;
    }
    let file_char = file.to_usi();
    let rank_char = char::from(b'1' + u8::try_from(rank.raw()).ok()?);
    Some(format!("{file_char}{rank_char}"))
}

const KI2_KANJI_RANKS: [char; 9] = ['一', '二', '三', '四', '五', '六', '七', '八', '九'];
const KI2_WIDE_DIGITS: [char; 9] = ['１', '２', '３', '４', '５', '６', '７', '８', '９'];

fn piece_type_to_ki2(piece_type: PieceType) -> Option<&'static str> {
    match piece_type {
        PieceType::PAWN => Some("歩"),
        PieceType::LANCE => Some("香"),
        PieceType::KNIGHT => Some("桂"),
        PieceType::SILVER => Some("銀"),
        PieceType::GOLD => Some("金"),
        PieceType::BISHOP => Some("角"),
        PieceType::ROOK => Some("飛"),
        PieceType::KING => Some("玉"),
        PieceType::PRO_PAWN => Some("と"),
        PieceType::PRO_LANCE => Some("成香"),
        PieceType::PRO_KNIGHT => Some("成桂"),
        PieceType::PRO_SILVER => Some("成銀"),
        PieceType::HORSE => Some("馬"),
        PieceType::DRAGON => Some("龍"),
        _ => None,
    }
}

fn ki2_destination_string(to: Square, last_move: Move32) -> String {
    if last_move.is_normal() && last_move.to_sq() == to {
        return "同".to_string();
    }
    let file = to.file().raw() + 1;
    let rank = to.rank().raw() + 1;
    let file_char = KI2_WIDE_DIGITS[(file - 1) as usize];
    let rank_char = KI2_KANJI_RANKS[(rank - 1) as usize];
    format!("{file_char}{rank_char}")
}

fn ki2_perspective_coords(sq: Square, color: Color) -> (i8, i8) {
    let sq = if color == Color::BLACK { sq } else { super::square::flip(sq) };
    (sq.file().raw(), sq.rank().raw())
}

fn ki2_promotion_option_count(
    pos: &Position,
    piece_type: PieceType,
    from: Square,
    to: Square,
) -> u8 {
    if !piece_type.is_promotable() || piece_type.is_promoted() {
        return 1;
    }
    if !super::square::is_promotable_move(pos.turn(), from, to) {
        return 1;
    }
    let to_rank = to.rank().raw();
    let us = pos.turn();
    let mandatory = match piece_type {
        PieceType::PAWN | PieceType::LANCE => {
            (us == Color::BLACK && to_rank == 0) || (us == Color::WHITE && to_rank == 8)
        }
        PieceType::KNIGHT => {
            (us == Color::BLACK && to_rank <= 1) || (us == Color::WHITE && to_rank >= 7)
        }
        _ => false,
    };
    if mandatory { 1 } else { 2 }
}

impl Move {
    // ANCHOR: move_bit_constants
    // 特殊手定数
    pub const MOVE_NONE: Self = Self(0);
    pub const MOVE_NULL: Self = Self((1 << 7) + 1);
    pub const MOVE_RESIGN: Self = Self((2 << 7) + 2);
    pub const MOVE_WIN: Self = Self((3 << 7) + 3);
    pub const MOVE_END: Self = Self((4 << 7) + 4);

    // フラグ定数
    pub const MOVE_DROP: u16 = 1 << 14;
    pub const MOVE_PROMOTE: u16 = 1 << 15;
    // ANCHOR_END: move_bit_constants

    /// 内部表現（16bit）を取得
    #[inline]
    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }

    /// 内部表現（16bit）から生成
    #[inline]
    #[must_use]
    pub const fn from_raw(raw: u16) -> Self {
        Self(raw)
    }

    /// cshogi / Apery 互換の 16bit 指し手に変換する。
    #[inline]
    #[must_use]
    pub const fn to_apery(self) -> AperyMove {
        AperyMove::from_move(self)
    }

    // ビットマスク
    const TO_MASK: u16 = 0x7f; // 下位7bit
    const FROM_MASK: u16 = 0x7f; // 7bit分
    const FROM_SHIFT: u16 = 7;

    /// 通常の移動手を生成
    #[must_use]
    pub fn normal(from: Square, to: Square) -> Self {
        let from_idx = u16::try_from(from.to_board_index()).expect("from square index");
        let to_idx = u16::try_from(to.to_board_index()).expect("to square index");
        Self((from_idx << Self::FROM_SHIFT) | to_idx)
    }

    /// 通常の移動手を生成（高速と内部用）。
    ///
    /// - `Square` の内部値（0..=80）を直接詰めるため、`TryFrom` を回避する。
    /// - `Square::SQ_NONE` や負値が入ると壊れるので、**内部のホットパス専用**。
    #[inline]
    #[must_use]
    pub(crate) fn normal_fast(from: Square, to: Square) -> Self {
        debug_assert!(from.is_on_board(), "from square must be on board");
        debug_assert!(to.is_on_board(), "to square must be on board");
        // SAFETY: 呼び出し側が from/to の値を 0..=80 の範囲に保つことを保証する。
        let from_idx = from.raw() as u16;
        let to_idx = to.raw() as u16;
        Self((from_idx << Self::FROM_SHIFT) | to_idx)
    }

    /// 駒打ち手を生成
    #[must_use]
    pub fn drop(piece_type: PieceType, to: Square) -> Self {
        let piece_idx = u16::try_from(piece_type.to_index()).expect("piece type index");
        let to_idx = u16::try_from(to.to_board_index()).expect("drop target index");
        Self(Self::MOVE_DROP | (piece_idx << Self::FROM_SHIFT) | to_idx)
    }

    /// 駒打ち手を生成（高速と内部用）。
    #[inline]
    #[must_use]
    pub(crate) fn drop_fast(piece_type: PieceType, to: Square) -> Self {
        debug_assert!(to.is_on_board(), "drop target must be on board");
        // 0..=6 が前提（PAWN..=GOLD）
        let piece_idx = piece_type.to_index() as u16;
        let to_idx = to.raw() as u16;
        Self(Self::MOVE_DROP | (piece_idx << Self::FROM_SHIFT) | to_idx)
    }

    /// 成り手を生成
    #[must_use]
    pub fn promotion(from: Square, to: Square) -> Self {
        let from_idx = u16::try_from(from.to_board_index()).expect("from square index");
        let to_idx = u16::try_from(to.to_board_index()).expect("to square index");
        Self(Self::MOVE_PROMOTE | (from_idx << Self::FROM_SHIFT) | to_idx)
    }

    /// 成り手を生成（高速と内部用）。
    #[inline]
    #[must_use]
    pub(crate) fn promotion_fast(from: Square, to: Square) -> Self {
        debug_assert!(from.is_on_board(), "from square must be on board");
        debug_assert!(to.is_on_board(), "to square must be on board");
        let from_idx = from.raw() as u16;
        let to_idx = to.raw() as u16;
        Self(Self::MOVE_PROMOTE | (from_idx << Self::FROM_SHIFT) | to_idx)
    }

    // ANCHOR: move_accessors
    /// 移動元の座標を取得
    #[inline]
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn from_sq(self) -> Square {
        Square::new(((self.0 >> Self::FROM_SHIFT) & Self::FROM_MASK) as i8)
    }

    /// 移動先の座標を取得
    #[inline]
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn to_sq(self) -> Square {
        Square::new((self.0 & Self::TO_MASK) as i8)
    }

    /// 駒打ちかどうか判定
    #[inline]
    #[must_use]
    pub const fn is_drop(self) -> bool {
        (self.0 & Self::MOVE_DROP) != 0
    }

    /// 成りかどうか判定
    #[inline]
    #[must_use]
    pub const fn is_promotion(self) -> bool {
        (self.0 & Self::MOVE_PROMOTE) != 0
    }

    /// 指し手種別を取得
    #[must_use]
    pub const fn move_type(self) -> MoveType {
        MoveType::from_bits(self.0 & (Self::MOVE_PROMOTE | Self::MOVE_DROP))
    }

    /// 打つ駒種を取得（駒打ちの場合）
    #[must_use]
    pub fn dropped_piece(self) -> Option<PieceType> {
        if self.is_drop() {
            let raw = (self.0 >> Self::FROM_SHIFT) & Self::FROM_MASK;
            let piece_raw = i8::try_from(raw).expect("piece type nibble");
            Some(PieceType::new(piece_raw))
        } else {
            None
        }
    }
    // ANCHOR_END: move_accessors

    /// 有効な手かどうか判定（特殊手を除外）
    #[must_use]
    #[inline]
    pub fn is_normal(self) -> bool {
        let raw = self.0;
        if raw == Self::MOVE_NONE.0
            || raw == Self::MOVE_NULL.0
            || raw == Self::MOVE_RESIGN.0
            || raw == Self::MOVE_WIN.0
            || raw == Self::MOVE_END.0
        {
            return false;
        }

        let to = raw & Self::TO_MASK;
        if to > 80 {
            return false;
        }

        if (raw & Self::MOVE_DROP) != 0 {
            // 駒打ちの場合は駒種をチェック
            let pt = (raw >> Self::FROM_SHIFT) & Self::FROM_MASK;
            pt >= PieceType::PAWN.raw() as u16 && pt <= PieceType::GOLD.raw() as u16
        } else {
            // 通常移動の場合は移動元をチェック
            let from = (raw >> Self::FROM_SHIFT) & Self::FROM_MASK;
            from <= 80
        }
    }

    /// USI形式の文字列から生成
    #[must_use]
    pub fn from_usi(s: &str) -> Option<Self> {
        if s == "null" || s == "0000" {
            return Some(Self::MOVE_NULL);
        }
        if s == "none" {
            return Some(Self::MOVE_NONE);
        }
        if s == "resign" {
            return Some(Self::MOVE_RESIGN);
        }
        if s == "win" {
            return Some(Self::MOVE_WIN);
        }

        let chars: Vec<char> = s.chars().collect();

        // 駒打ち（例: "P*5e"）
        if chars.len() >= 4 && chars[1] == '*' {
            let piece_str = chars[0].to_string();
            let pt = match piece_str.as_str() {
                "P" => PieceType::PAWN,
                "L" => PieceType::LANCE,
                "N" => PieceType::KNIGHT,
                "S" => PieceType::SILVER,
                "B" => PieceType::BISHOP,
                "R" => PieceType::ROOK,
                "G" => PieceType::GOLD,
                _ => return None,
            };

            let to_str: String = chars[2..4].iter().collect();
            let to = Square::from_usi(&to_str)?;

            return Some(Self::drop(pt, to));
        }

        // 通常移動（例: "7g7f" または "2b3c+"）
        if chars.len() >= 4 {
            let from_str: String = chars[0..2].iter().collect();
            let to_str: String = chars[2..4].iter().collect();

            let from = Square::from_usi(&from_str)?;
            let to = Square::from_usi(&to_str)?;

            let is_promotion = chars.len() >= 5 && chars[4] == '+';

            if is_promotion {
                Some(Self::promotion(from, to))
            } else {
                Some(Self::normal(from, to))
            }
        } else {
            None
        }
    }

    /// USI形式の文字列に変換
    #[must_use]
    pub fn to_usi(self) -> String {
        if !self.is_normal() {
            return if self == Self::MOVE_RESIGN {
                "resign".to_string()
            } else if self == Self::MOVE_WIN {
                "win".to_string()
            } else if self == Self::MOVE_NULL {
                "null".to_string()
            } else if self == Self::MOVE_NONE {
                "none".to_string()
            } else {
                String::new()
            };
        }

        if self.is_drop() {
            // 駒打ち
            let Some(pt) = self.dropped_piece() else {
                return String::new();
            };
            let piece_char = match pt {
                PieceType::PAWN => "P",
                PieceType::LANCE => "L",
                PieceType::KNIGHT => "N",
                PieceType::SILVER => "S",
                PieceType::BISHOP => "B",
                PieceType::ROOK => "R",
                PieceType::GOLD => "G",
                _ => return String::new(),
            };
            format!("{piece_char}*{}", self.to_sq())
        } else {
            // 通常移動
            let mut result = format!("{}{}", self.from_sq(), self.to_sq());
            if self.is_promotion() {
                result.push('+');
            }
            result
        }
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", (*self).to_usi())
    }
}

/// 32bit指し手表現（駒情報付き）
/// - 下位16bit: Move
/// - 上位16bit: 移動する駒（Piece, 下位5bitのみ使用）
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct Move32(u32);

const _: () = {
    assert!(core::mem::size_of::<Move32>() == 4);
    assert!(core::mem::align_of::<Move32>() == 4);
};

impl Move32 {
    const TO_MASK: u32 = 0x7f;
    const FROM_MASK: u32 = 0x7f;
    const FROM_SHIFT: u32 = 7;
    const MOVE_MASK: u32 = 0xffff;
    const MOVE_DROP_MASK: u32 = Move::MOVE_DROP as u32;
    const MOVE_PROMOTE_MASK: u32 = Move::MOVE_PROMOTE as u32;
    const BOARD_SQUARE_COUNT: usize = Square::COUNT;
    const DROP_KIND_COUNT: usize = 7;
    /// from/to index 空間のサイズ。
    pub const FROM_TO_TABLE_SIZE: usize =
        (Self::BOARD_SQUARE_COUNT + Self::DROP_KIND_COUNT) * Self::BOARD_SQUARE_COUNT;

    /// 内部表現（32bit）を取得
    #[inline(always)]
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// 内部表現（32bit）から生成
    #[inline(always)]
    #[must_use]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// 通常移動の生成
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn normal(from: Square, to: Square, piece: Piece) -> Self {
        let m16 = Move::normal(from, to);
        let piece_raw = u8::try_from(piece.raw()).expect("piece index fits in u8") & 0x1f;
        Self((u32::from(piece_raw) << 16) | u32::from(m16.raw()))
    }

    /// 駒打ちの生成
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn drop(piece_type: PieceType, to: Square, color: Color) -> Self {
        let m16 = Move::drop(piece_type, to);
        let piece = Piece::from_parts(color, piece_type);
        let piece_raw = u8::try_from(piece.raw()).expect("piece index fits in u8") & 0x1f;
        Self((u32::from(piece_raw) << 16) | u32::from(m16.raw()))
    }

    /// 成り移動の生成
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn promotion(from: Square, to: Square, piece: Piece) -> Self {
        let m16 = Move::promotion(from, to);
        let promoted = piece.promote();
        let piece_raw = u8::try_from(promoted.raw()).expect("piece index fits in u8") & 0x1f;
        Self((u32::from(piece_raw) << 16) | u32::from(m16.raw()))
    }

    /// USI文字列から `Move32` を生成（部分 `Move32`: 駒情報は未設定）
    ///
    /// 完全な `Move32` が必要な場合は `Position::move32_from_move()` を使用する。
    #[must_use]
    pub fn from_usi(s: &str) -> Option<Self> {
        Move::from_usi(s).map(|mv| Self(u32::from(mv.raw())))
    }

    /// Move部分を取得
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    #[inline(always)]
    pub const fn to_move(self) -> Move {
        Move::from_raw((self.0 & Self::MOVE_MASK) as u16)
    }

    /// 上位16bitに駒情報が設定されているか判定
    ///
    /// - `true`: 完全 `Move32`（`Position::move32_from_move` 等で生成）
    /// - `false`: 部分 `Move32`（`from_usi` 等で生成、駒情報なし）
    #[must_use]
    #[inline(always)]
    pub const fn has_piece_info(self) -> bool {
        (self.0 >> 16) != 0
    }

    /// 移動後の駒を取得
    ///
    /// 部分 `Move32`（`from_usi()` 等で生成）の場合は `Piece::NONE` を返す。
    /// 駒情報が必要な場合は事前に `has_piece_info()` で確認するか、
    /// `Position::move32_from_move()` で完全な `Move32` を生成する。
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    #[inline(always)]
    pub const fn piece_after_move(self) -> Piece {
        Piece::new(((self.0 >> 16) & 0x1f) as i8)
    }

    // Moveのメソッドを委譲
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    #[inline(always)]
    pub const fn from_sq(self) -> Square {
        Square::new(((self.0 >> Self::FROM_SHIFT) & Self::FROM_MASK) as i8)
    }

    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    #[inline(always)]
    pub const fn to_sq(self) -> Square {
        Square::new((self.0 & Self::TO_MASK) as i8)
    }

    #[must_use]
    #[inline(always)]
    pub const fn is_drop(self) -> bool {
        (self.0 & Self::MOVE_DROP_MASK) != 0
    }

    #[must_use]
    #[inline(always)]
    pub const fn is_promotion(self) -> bool {
        (self.0 & Self::MOVE_PROMOTE_MASK) != 0
    }

    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    #[inline(always)]
    pub const fn dropped_piece(self) -> Option<PieceType> {
        if self.is_drop() {
            Some(PieceType::new(((self.0 >> Self::FROM_SHIFT) & Self::FROM_MASK) as i8))
        } else {
            None
        }
    }

    /// 指し手種別を取得
    #[must_use]
    #[inline(always)]
    pub const fn move_type(self) -> MoveType {
        MoveType::from_bits((self.0 as u16) & (Move::MOVE_PROMOTE | Move::MOVE_DROP))
    }

    /// from/to index を返す。
    ///
    /// 通常手、成り、駒打ちを、history 系テーブル向けの一意な index に変換する。
    /// 無効手や特殊手 (`MOVE_NONE` / `MOVE_NULL` など) の場合は `None` を返す。
    ///
    /// - 通常手/成り: `from * 81 + to`
    /// - 駒打ち: `(81 + drop_kind - 1) * 81 + to`
    ///
    /// 通常手と駒打ちを単一の index 空間に配置する。
    #[must_use]
    #[inline(always)]
    #[allow(clippy::cast_sign_loss)]
    pub const fn from_to_index(self) -> Option<usize> {
        let raw = self.0;

        if raw == MOVE_NONE.0
            || raw == MOVE_NULL.0
            || raw == MOVE_RESIGN.0
            || raw == MOVE_WIN.0
            || raw == MOVE_END.0
        {
            return None;
        }

        let to = raw & Self::TO_MASK;
        if to > 80 {
            return None;
        }

        let from_like = if (raw & Self::MOVE_DROP_MASK) != 0 {
            let piece_type = (raw >> Self::FROM_SHIFT) & Self::FROM_MASK;
            if piece_type < PieceType::PAWN.raw() as u32
                || piece_type > PieceType::GOLD.raw() as u32
            {
                return None;
            }
            Self::BOARD_SQUARE_COUNT + (piece_type as usize - PieceType::PAWN.to_index())
        } else {
            let from = (raw >> Self::FROM_SHIFT) & Self::FROM_MASK;
            if from > 80 {
                return None;
            }
            from as usize
        };

        Some(from_like * Self::BOARD_SQUARE_COUNT + to as usize)
    }

    #[must_use]
    pub fn is_normal(self) -> bool {
        self.to_move().is_normal()
    }

    #[must_use]
    pub fn to_usi(self) -> String {
        self.to_move().to_usi()
    }

    /// CSA形式の文字列に変換（手番記号は含めない）
    #[must_use]
    pub fn to_csa(self) -> Option<String> {
        if !self.is_normal() {
            return None;
        }
        let to = square_to_csa(self.to_sq())?;
        let from = if self.is_drop() { "00".to_string() } else { square_to_csa(self.from_sq())? };
        let piece_type = if self.is_drop() {
            self.dropped_piece()?
        } else {
            self.piece_after_move().piece_type()
        };
        let piece_code = piece_type_to_csa(piece_type)?;
        Some(format!("{from}{to}{piece_code}"))
    }

    /// KI2形式の文字列に変換
    #[must_use]
    pub fn to_ki2(self, pos: &Position) -> Option<String> {
        if !self.is_normal() {
            return None;
        }
        let turn = if pos.turn() == Color::BLACK { "▲" } else { "△" };
        let to = self.to_sq();
        let dest = ki2_destination_string(to, pos.last_move());

        if self.is_drop() {
            let piece_type = self.dropped_piece()?;
            let piece_str = piece_type_to_ki2(piece_type)?;
            let attackers = pos.attackers_to_color(pos.turn(), to, pos.pieces());
            let candidates = attackers.and(pos.pieces_for(piece_type, pos.turn()));
            let drop_suffix = if !candidates.is_empty() { "打" } else { "" };
            let mut out = format!("{turn}{dest}{piece_str}{drop_suffix}");
            if dest == "同" && out.chars().count() == 3 {
                let mut chars: Vec<char> = out.chars().collect();
                chars.insert(2, '　');
                out = chars.into_iter().collect();
            }
            return Some(out);
        }

        let moved = self.piece_after_move();
        let piece_type =
            if self.is_promotion() { moved.piece_type().demote() } else { moved.piece_type() };
        let piece_str = piece_type_to_ki2(piece_type)?;

        let attackers = pos.attackers_to_color(pos.turn(), to, pos.pieces());
        let mut candidate_bb = attackers.and(pos.pieces_for(piece_type, pos.turn()));
        let mut candidates: Vec<Square> = Vec::new();
        while let Some(sq) = candidate_bb.pop_lsb() {
            candidates.push(sq);
        }
        let mut promotion_count = [0u8; 82];
        for sq in &candidates {
            let idx = sq.to_index_with_none();
            if idx < promotion_count.len() {
                promotion_count[idx] = ki2_promotion_option_count(pos, piece_type, *sq, to);
            }
        }

        let mut relative = String::new();
        let mut motion = String::new();
        if candidates.len() > 1 {
            let (to_file, to_rank) = ki2_perspective_coords(to, pos.turn());
            let (from_file, from_rank) = ki2_perspective_coords(self.from_sq(), pos.turn());

            let mut candidates_left = 0u32;
            let mut candidates_right = 0u32;
            let mut candidates_up = 0u32;
            let mut candidates_down = 0u32;
            let mut candidates_side = 0u32;
            let mut same_file_count = 0u32;
            let mut other_left = false;
            let mut other_right = false;
            let mut left_up = 0u32;
            let mut left_down = 0u32;
            let mut left_side = 0u32;
            let mut right_up = 0u32;
            let mut right_down = 0u32;
            let mut right_side = 0u32;
            let mut same_up = 0u32;
            let mut same_down = 0u32;
            let mut same_side = 0u32;

            for sq in &candidates {
                let (cf, cr) = ki2_perspective_coords(*sq, pos.turn());
                let rel = match cf.cmp(&to_file) {
                    std::cmp::Ordering::Greater => {
                        candidates_left += 1;
                        if *sq != self.from_sq() {
                            other_left = true;
                        }
                        "left"
                    }
                    std::cmp::Ordering::Less => {
                        candidates_right += 1;
                        if *sq != self.from_sq() {
                            other_right = true;
                        }
                        "right"
                    }
                    std::cmp::Ordering::Equal => {
                        same_file_count += 1;
                        "same"
                    }
                };

                let direction = match cr.cmp(&to_rank) {
                    std::cmp::Ordering::Greater => {
                        candidates_up += 1;
                        "up"
                    }
                    std::cmp::Ordering::Less => {
                        candidates_down += 1;
                        "down"
                    }
                    std::cmp::Ordering::Equal => {
                        candidates_side += 1;
                        "side"
                    }
                };

                match (rel, direction) {
                    ("left", "up") => left_up += 1,
                    ("left", "down") => left_down += 1,
                    ("left", "side") => left_side += 1,
                    ("right", "up") => right_up += 1,
                    ("right", "down") => right_down += 1,
                    ("right", "side") => right_side += 1,
                    ("same", "up") => same_up += 1,
                    ("same", "down") => same_down += 1,
                    ("same", "side") => same_side += 1,
                    _ => {}
                }
            }

            let horse_or_dragon = piece_type == PieceType::HORSE || piece_type == PieceType::DRAGON;
            match from_rank.cmp(&to_rank) {
                std::cmp::Ordering::Greater => {
                    let direction_count = candidates_up;
                    let branch_motion = '上';
                    if direction_count == 1 {
                        motion.push(branch_motion);
                    } else {
                        let (relative_count_total, relative_direction_count) =
                            match from_file.cmp(&to_file) {
                                std::cmp::Ordering::Equal => {
                                    if !horse_or_dragon {
                                        relative.push('直');
                                        (same_file_count, same_up)
                                    } else if other_right && !other_left {
                                        relative.push('左');
                                        (candidates_left, left_up)
                                    } else if other_left && !other_right {
                                        relative.push('右');
                                        (candidates_right, right_up)
                                    } else {
                                        (0, 0)
                                    }
                                }
                                std::cmp::Ordering::Greater => {
                                    relative.push('左');
                                    (candidates_left, left_up)
                                }
                                std::cmp::Ordering::Less => {
                                    relative.push('右');
                                    (candidates_right, right_up)
                                }
                            };

                        if relative.is_empty() || relative_count_total > 0 {
                            let other_same_relative =
                                relative_count_total.saturating_sub(relative_direction_count);
                            let mut need_motion =
                                relative.is_empty() || relative_direction_count > 1;
                            if !need_motion && other_same_relative > 0 {
                                if relative == "左" || relative == "右" {
                                    need_motion = true;
                                } else {
                                    if same_down > 0 && candidates_down > 1 {
                                        need_motion = true;
                                    }
                                    if same_side > 0 && candidates_side > 1 {
                                        need_motion = true;
                                    }
                                }
                            }
                            if need_motion {
                                motion.push(branch_motion);
                            }
                        }
                    }
                }
                std::cmp::Ordering::Less => {
                    let direction_count = candidates_down;
                    let branch_motion = '引';
                    if direction_count == 1 {
                        motion.push(branch_motion);
                    } else {
                        let (relative_count_total, relative_direction_count) =
                            match from_file.cmp(&to_file) {
                                std::cmp::Ordering::Equal => {
                                    if !horse_or_dragon {
                                        relative.push('直');
                                        (same_file_count, same_down)
                                    } else if other_right && !other_left {
                                        relative.push('左');
                                        (candidates_left, left_down)
                                    } else if other_left && !other_right {
                                        relative.push('右');
                                        (candidates_right, right_down)
                                    } else {
                                        (0, 0)
                                    }
                                }
                                std::cmp::Ordering::Greater => {
                                    relative.push('左');
                                    (candidates_left, left_down)
                                }
                                std::cmp::Ordering::Less => {
                                    relative.push('右');
                                    (candidates_right, right_down)
                                }
                            };

                        if relative.is_empty() || relative_count_total > 0 {
                            let other_same_relative =
                                relative_count_total.saturating_sub(relative_direction_count);
                            let mut need_motion =
                                relative.is_empty() || relative_direction_count > 1;
                            if !need_motion && other_same_relative > 0 {
                                if relative == "左" {
                                    if left_up > 0 && candidates_up > 1 {
                                        need_motion = true;
                                    }
                                    if left_side > 0 && candidates_side > 1 {
                                        need_motion = true;
                                    }
                                } else if relative == "右" {
                                    if right_up > 0 && candidates_up > 1 {
                                        need_motion = true;
                                    }
                                    if right_side > 0 && candidates_side > 1 {
                                        need_motion = true;
                                    }
                                } else {
                                    if same_up > 0 && candidates_up > 1 {
                                        need_motion = true;
                                    }
                                    if same_side > 0 && candidates_side > 1 {
                                        need_motion = true;
                                    }
                                }
                            }
                            if need_motion {
                                motion.push(branch_motion);
                            }
                        }
                    }
                }
                std::cmp::Ordering::Equal => {
                    let direction_count = candidates_side;
                    let branch_motion = '寄';
                    if direction_count == 1 {
                        motion.push(branch_motion);
                    } else {
                        let (relative_count_total, relative_direction_count) =
                            match from_file.cmp(&to_file) {
                                std::cmp::Ordering::Greater => {
                                    relative.push('左');
                                    (candidates_left, left_side)
                                }
                                std::cmp::Ordering::Less => {
                                    relative.push('右');
                                    (candidates_right, right_side)
                                }
                                std::cmp::Ordering::Equal => (same_file_count, same_side),
                            };

                        if relative.is_empty() || relative_count_total > 0 {
                            let other_same_relative =
                                relative_count_total.saturating_sub(relative_direction_count);
                            let mut need_motion =
                                relative.is_empty() || relative_direction_count > 1;
                            if !need_motion && other_same_relative > 0 {
                                if relative == "左" {
                                    if left_up > 0 && candidates_up > 1 {
                                        need_motion = true;
                                    }
                                    if left_down > 0 && candidates_down > 1 {
                                        need_motion = true;
                                    }
                                } else if relative == "右" {
                                    if right_up > 0 && candidates_up > 1 {
                                        need_motion = true;
                                    }
                                    if right_down > 0 && candidates_down > 1 {
                                        need_motion = true;
                                    }
                                } else {
                                    if same_up > 0 && candidates_up > 1 {
                                        need_motion = true;
                                    }
                                    if same_down > 0 && candidates_down > 1 {
                                        need_motion = true;
                                    }
                                }
                            }
                            if need_motion {
                                motion.push(branch_motion);
                            }
                        }
                    }
                }
            }
        }

        let from_idx = self.from_sq().to_index_with_none();
        let promotable = promotion_count.get(from_idx).copied().unwrap_or(1) == 2;
        let promote_suffix = if self.is_promotion() {
            "成"
        } else if promotable {
            "不成"
        } else {
            ""
        };
        let mut out = format!("{turn}{dest}{piece_str}{relative}{motion}{promote_suffix}");
        if dest == "同" && out.chars().count() == 3 {
            let mut chars: Vec<char> = out.chars().collect();
            chars.insert(2, '　');
            out = chars.into_iter().collect();
        }
        Some(out)
    }

    /// 7bit の移動元フィールドをインデックスとして返す。
    ///
    /// 通常手と成り手では盤面の移動元マスインデックスを返す。
    /// 駒打ちおよびセンチネル値の場合は、エンコードされた生フィールドの値を返すため、
    /// 盤面上の移動元マスを表すものではない。
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    #[inline(always)]
    pub const fn from_index(self) -> usize {
        ((self.0 >> Self::FROM_SHIFT) & Self::FROM_MASK) as usize
    }

    /// 7bit の移動先フィールドをインデックスとして返す。
    ///
    /// 通常手、成り手、駒打ちでは目標マスのインデックスを返す。
    /// センチネル値の場合はエンコードされた生の移動先フィールドを返す。
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    #[inline(always)]
    pub const fn to_index(self) -> usize {
        (self.0 & Self::TO_MASK) as usize
    }

    #[must_use]
    pub fn to_apery(self, pos: &Position) -> AperyMove32 {
        pos.apery_move32_from_move32(self)
    }
}

impl sealed::Sealed for Move32 {}

impl MoveBase for Move32 {
    #[inline]
    fn to_move(self) -> Move {
        Move32::to_move(self)
    }

    #[inline]
    fn piece_after_hint(self) -> Piece {
        self.piece_after_move()
    }
}

impl Default for Move32 {
    fn default() -> Self {
        MOVE_NONE
    }
}

impl fmt::Display for Move32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_usi())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Position;
    use crate::types::PieceType;

    #[test]
    fn test_move_constants() {
        assert_eq!(Move::MOVE_NONE.raw(), 0);
        assert_eq!(Move::MOVE_NULL.raw(), (1 << 7) + 1);
        assert_eq!(Move::MOVE_RESIGN.raw(), (2 << 7) + 2);
        assert_eq!(Move::MOVE_WIN.raw(), (3 << 7) + 3);
        assert_eq!(Move::MOVE_END.raw(), (4 << 7) + 4);
        assert_eq!(Move::MOVE_DROP, 1 << 14);
        assert_eq!(Move::MOVE_PROMOTE, 1 << 15);
    }

    #[test]
    fn test_move_bit_encoding() {
        // 通常移動: 7g → 7f
        let from = "7g".parse::<Square>().unwrap();
        let to = "7f".parse::<Square>().unwrap();
        let m = Move::normal(from, to);
        assert_eq!(m.to_sq(), to);
        assert_eq!(m.from_sq(), from);
        assert!(!m.is_drop());
        assert!(!m.is_promotion());

        // 駒打ち: P*5e(40)
        let drop_to = "5e".parse::<Square>().unwrap();
        let m = Move::drop(PieceType::PAWN, drop_to);
        assert_eq!(m.to_sq(), drop_to);
        assert!(m.is_drop());
        assert_eq!(m.dropped_piece(), Some(PieceType::PAWN));
        assert!(!m.is_promotion());

        // 成り移動: 2b(10) → 3c(20)+
        let promote_from = "2b".parse::<Square>().unwrap();
        let promote_to = "3c".parse::<Square>().unwrap();
        let m = Move::promotion(promote_from, promote_to);
        assert_eq!(m.to_sq(), promote_to);
        assert_eq!(m.from_sq(), promote_from);
        assert!(!m.is_drop());
        assert!(m.is_promotion());
    }

    #[test]
    fn test_move_from_usi() {
        // 通常移動
        let m = Move::from_usi("7g7f").unwrap();
        assert_eq!(m.from_sq(), "7g".parse::<Square>().unwrap());
        assert_eq!(m.to_sq(), "7f".parse::<Square>().unwrap());
        assert!(!m.is_drop());
        assert!(!m.is_promotion());

        // 駒打ち
        let m = Move::from_usi("P*5e").unwrap();
        assert!(m.is_drop());
        assert_eq!(m.dropped_piece(), Some(PieceType::PAWN));
        assert_eq!(m.to_sq(), "5e".parse::<Square>().unwrap());

        // 成り移動
        let m = Move::from_usi("2b3c+").unwrap();
        let from_sq_expected = "2b".parse::<Square>().unwrap();
        let to_sq_expected = "3c".parse::<Square>().unwrap();
        assert_eq!(m.from_sq(), from_sq_expected);
        assert_eq!(m.to_sq(), to_sq_expected);
        assert!(m.is_promotion());

        // 特殊な手
        assert_eq!(Move::from_usi("resign"), Some(Move::MOVE_RESIGN));
        assert_eq!(Move::from_usi("win"), Some(Move::MOVE_WIN));
        assert_eq!(Move::from_usi("null"), Some(Move::MOVE_NULL));
        assert_eq!(Move::from_usi("0000"), Some(Move::MOVE_NULL));
        assert_eq!(Move::from_usi("none"), Some(Move::MOVE_NONE));
    }

    fn ki2_from_sfen_move(sfen: &str, usi: &str, pre_moves: &[&str]) -> String {
        let mut pos = Position::empty();
        pos.set_sfen(sfen).expect("valid sfen");
        for mv in pre_moves {
            let mv16 = Move::from_usi(mv).expect("valid usi");
            let mv = pos.move32_from_move(mv16);
            pos.apply_move32(mv);
        }
        let mv16 = Move::from_usi(usi).expect("valid usi");
        let mv = pos.move32_from_move(mv16);
        mv.to_ki2(&pos).expect("ki2 output")
    }

    #[test]
    fn test_move_to_ki2_issue37() {
        let sfen = "ln1gk3l/1r4g2/p1ppssnpp/4p1p2/1p3P3/2P1SgPRP/PPBPP1N2/2SK5/LN1G4L b B2P 1";
        assert_eq!(ki2_from_sfen_move(sfen, "5c4d", &["4e4d"]), "△同銀右");
        assert_eq!(ki2_from_sfen_move(sfen, "4c4d", &["4e4d"]), "△同銀直");
    }

    #[test]
    fn test_move_to_ki2_dragon_vertical_relative() {
        let sfen = "ln1g4l/1ks6/1pp2pn1p/p2p1+R3/6+R2/P1s3P1P/1P1P2N2/2K6/LN2S3L b B2G3Pbgs4p 1";
        assert_eq!(ki2_from_sfen_move(sfen, "4d4f", &[]), "▲４六龍左");
        assert_eq!(ki2_from_sfen_move(sfen, "3e4f", &[]), "▲４六龍右");
    }

    #[test]
    fn test_move_to_ki2_dragon_pull_motion() {
        let sfen = "ln1g3+Rl/1ks1g4/1pp3n1p/p5p2/3p5/P1P3P1P/1PSPSPN2/2K6/LN2Sr2L b BG2Pbg3p 1";
        assert_eq!(ki2_from_sfen_move(sfen, "5g6h", &[]), "▲６八銀右");
    }

    #[test]
    fn test_move_to_ki2_horse_left_vertical() {
        let sfen = "ln1g1R2l/1ks6/1pp1p1n1p/p2p+B+B3/9/P5P1P/1P1P2N2/2K6/LN2S2rL b 2G3Pg2s4p 1";
        assert_eq!(ki2_from_sfen_move(sfen, "5d5c", &[]), "▲５三馬左");
    }

    #[test]
    fn test_move_to_ki2_drop_with_existing_piece() {
        let sfen = "ln1g4l/1ks6/1pp3n1p/p2pS4/9/P1P3P1P/1P1P2N2/2K1p4/LN1GP2+rL b RB2SPb2g4p 1";
        assert_eq!(ki2_from_sfen_move(sfen, "S*6c", &[]), "▲６三銀打");
    }

    #[test]
    fn test_move_to_ki2_unique_motion_without_relative() {
        let sfen = "l+R5nl/3p1bgk1/2+B2p1sp/p3p4/2p3G2/P4P1R1/3gS2pP/4g1S2/L5KPL w N5Ps2n2p 1";
        assert_eq!(ki2_from_sfen_move(sfen, "6g5g", &[]), "△５七金寄");
    }

    #[test]
    fn test_move_to_ki2_horse_left_right_branch() {
        let sfen = "+B+B3g1nl/1p4sk1/3p1g3/p1p3ppp/5p3/P4PPPP/3S1SN2/4G1K2/L1+r1P3L b RNL2Pgsn2p 1";
        assert_eq!(ki2_from_sfen_move(sfen, "9a8b", &[]), "▲８二馬左");
        assert_eq!(ki2_from_sfen_move(sfen, "8a8b", &[]), "▲８二馬右");
    }

    #[test]
    fn test_move_to_ki2_horse_left_right_down_variation() {
        let sfen = "3+R3nk/6ggl/7pp/p2bppp2/6lNP/P3P4/5PSPS/6G+nL/1+r4SSK b GL3Pbn4p 1";
        let cases = [
            ("3g2h", "▲２八銀左引"),
            ("1g2h", "▲２八銀右"),
            ("3i2h", "▲２八銀左上"),
            ("2i2h", "▲２八銀直"),
        ];
        for (usi, expected) in cases {
            assert_eq!(ki2_from_sfen_move(sfen, usi, &[]), expected);
        }
    }

    #[test]
    fn test_move_to_ki2_horse_motion_vs_relative() {
        let sfen = "6gnk/7sl/p2+B1g1p1/6p1p/+Bn3p3/P4PPPP/3+r2S2/6SK1/L4G1NL b RL3Pgsn5p 1";
        assert_eq!(ki2_from_sfen_move(sfen, "9e8e", &[]), "▲８五馬寄");
        assert_eq!(ki2_from_sfen_move(sfen, "6c8e", &[]), "▲８五馬引");
    }

    #[test]
    fn test_move_to_ki2_horse_vertical_motion_selection() {
        let sfen = "+B6nl/l5gk1/5g1s1/p1+B2ppnp/7p1/P5P1P/4+p2P1/6GSL/L2+r2GNK b RN3P2s5p 1";
        assert_eq!(ki2_from_sfen_move(sfen, "9a9b", &[]), "▲９二馬引");
        assert_eq!(ki2_from_sfen_move(sfen, "7d9b", &[]), "▲９二馬上");
    }

    #[test]
    fn test_move_to_ki2_gold_like_straight_and_relative() {
        let sfen = "+R6nk/5gggl/6Ppp/p4pp2/2+b1p3P/P8/6SP1/7SL/1+r3PGNK w S2NL2Pbsl5p 1";
        let cases = [("4b3c", "△３三金右"), ("3b3c", "△３三金直"), ("2b3c", "△３三金左")];
        for (usi, expected) in cases {
            assert_eq!(ki2_from_sfen_move(sfen, usi, &[]), expected);
        }
    }

    #[test]
    fn test_move_to_ki2_dragon_left_right_branch() {
        let sfen = "l2p2gkl/2+R2sgs1/3+R5/p4ppp1/7np/P3pPP1P/7P1/4P1SK1/L4G1NL b BSN3Pbgn2p 1";
        assert_eq!(ki2_from_sfen_move(sfen, "7b6a", &[]), "▲６一龍左");
        assert_eq!(ki2_from_sfen_move(sfen, "6c6a", &[]), "▲６一龍右");
    }

    #[test]
    fn test_move_to_ki2_dragon_motion_distinction() {
        let sfen = "l+R4gnk/2s1+R1gsl/7pp/p4pp2/8P/P3PPPP1/5GNS1/4+p1GK1/L4s2L b 2N3P2b3p 1";
        assert_eq!(ki2_from_sfen_move(sfen, "8a7b", &[]), "▲７二龍引");
        assert_eq!(ki2_from_sfen_move(sfen, "5b7b", &[]), "▲７二龍寄");
    }

    #[test]
    fn test_move_to_ki2_promoted_pawn_relative_and_motion() {
        let sfen =
            "2+R4K1/3+P2G+B1/4+N+P1+L1/2p5p/1p7/p7P/+p+p+p6/2+p5L/k+p1+r5 w G2S3N3Pb2g2s2l3p 1";
        let cases = [
            ("9g8h", "△８八と右"),
            ("8g8h", "△８八と直"),
            ("7g8h", "△８八と左上"),
            ("8i8h", "△８八と引"),
            ("7h8h", "△８八と寄"),
        ];
        for (usi, expected) in cases {
            assert_eq!(ki2_from_sfen_move(sfen, usi, &[]), expected);
        }
    }

    #[test]
    fn test_move_to_usi() {
        // 通常移動
        let m = Move::normal("7g".parse().unwrap(), "7f".parse().unwrap());
        assert_eq!(m.to_usi(), "7g7f");

        // 駒打ち
        let m = Move::drop(PieceType::PAWN, "5e".parse::<Square>().unwrap());
        assert_eq!(m.to_usi(), "P*5e");

        // 成り移動
        let from_sq_expected = "2b".parse::<Square>().unwrap();
        let to_sq_expected = "3c".parse::<Square>().unwrap();
        let m = Move::promotion(from_sq_expected, to_sq_expected);
        assert_eq!(m.to_usi(), "2b3c+");

        // 特殊な手
        assert_eq!(Move::MOVE_NONE.to_usi(), "none");
        assert_eq!(Move::MOVE_RESIGN.to_usi(), "resign");
        assert_eq!(Move::MOVE_WIN.to_usi(), "win");
    }

    #[test]
    fn test_move_usi_round_trip() {
        let test_cases = vec!["7g7f", "2g2f", "8h2b+", "P*5e", "G*3c", "1a1b+"];

        for usi in test_cases {
            let m = Move::from_usi(usi).unwrap();
            assert_eq!(m.to_usi(), usi);
        }
    }

    #[test]
    fn test_move_is_ok() {
        // 有効な手
        assert!(Move::normal("8g".parse().unwrap(), "8f".parse().unwrap()).is_normal());
        assert!(Move::drop(PieceType::PAWN, "5e".parse().unwrap()).is_normal());
        assert!(Move::promotion("2b".parse().unwrap(), "3c".parse().unwrap()).is_normal());

        // 特殊手は無効扱い
        assert!(!Move::MOVE_NONE.is_normal());
        assert!(!Move::MOVE_NULL.is_normal());
        assert!(!Move::MOVE_RESIGN.is_normal());
        assert!(!Move::MOVE_WIN.is_normal());
        assert!(!Move::MOVE_END.is_normal());

        // 不正な駒打ち（KING）
        let invalid_drop =
            Move::from_raw(Move::MOVE_DROP | ((PieceType::KING.raw() as u16) << 7) | 0x28);
        assert!(!invalid_drop.is_normal());
    }

    #[test]
    fn test_move32_structure() {
        // 通常移動
        let normal_from = "7g".parse::<Square>().unwrap();
        let normal_to = "7f".parse::<Square>().unwrap();
        let m = Move32::normal(normal_from, normal_to, Piece::B_PAWN);
        assert_eq!(m.to_move(), Move::normal(normal_from, normal_to));
        assert_eq!(m.piece_after_move(), Piece::B_PAWN);

        // 駒打ち
        let drop_sq = "5e".parse::<Square>().unwrap();
        let m = Move32::drop(PieceType::GOLD, drop_sq, Color::WHITE);
        assert_eq!(m.to_move(), Move::drop(PieceType::GOLD, drop_sq));
        assert_eq!(m.piece_after_move(), Piece::W_GOLD);

        // 成り移動
        let promote_from = "2b".parse::<Square>().unwrap();
        let promote_to = "3c".parse::<Square>().unwrap();
        let m = Move32::promotion(promote_from, promote_to, Piece::B_BISHOP);
        assert_eq!(m.to_move(), Move::promotion(promote_from, promote_to));
        assert_eq!(m.piece_after_move(), Piece::B_HORSE); // BISHOPが成ってHORSEに
    }

    #[test]
    fn test_move32_delegation() {
        let from = "7g".parse::<Square>().unwrap();
        let to = "7f".parse::<Square>().unwrap();
        let m = Move32::normal(from, to, Piece::B_PAWN);
        assert_eq!(m.from_sq(), from);
        assert_eq!(m.to_sq(), to);
        assert!(!m.is_drop());
        assert!(!m.is_promotion());
        assert_eq!(m.dropped_piece(), None);
        assert!(m.is_normal());
        assert_eq!(m.to_usi(), "7g7f");
    }

    #[test]
    fn test_move32_raw_index_accessors() {
        let normal_from = Square::from_usi("7g").unwrap();
        let normal_to = Square::from_usi("7f").unwrap();
        let normal = Move32::normal(normal_from, normal_to, Piece::B_PAWN);
        assert_eq!(normal.from_index(), normal_from.to_board_index());
        assert_eq!(normal.to_index(), normal_to.to_board_index());

        let promote_from = Square::from_usi("2b").unwrap();
        let promote_to = Square::from_usi("3c").unwrap();
        let promote = Move32::promotion(promote_from, promote_to, Piece::B_BISHOP);
        assert_eq!(promote.from_index(), promote_from.to_board_index());
        assert_eq!(promote.to_index(), promote_to.to_board_index());

        let drop_to = Square::from_usi("5e").unwrap();
        let drop = Move32::drop(PieceType::PAWN, drop_to, Color::BLACK);
        assert_eq!(drop.from_index(), PieceType::PAWN.raw() as usize);
        assert_eq!(drop.to_index(), drop_to.to_board_index());

        assert_eq!(MOVE_NONE.from_index(), 0);
        assert_eq!(MOVE_NONE.to_index(), 0);
        assert_eq!(MOVE_NULL.from_index(), 1);
        assert_eq!(MOVE_NULL.to_index(), 1);
    }

    fn yane_from_to_index(mv: Move32) -> usize {
        let to = mv.to_sq().to_board_index();
        if mv.is_drop() {
            let piece_type = mv.dropped_piece().expect("drop must have piece type");
            (Square::COUNT + piece_type.to_index() - PieceType::PAWN.to_index()) * Square::COUNT
                + to
        } else {
            mv.from_sq().to_board_index() * Square::COUNT + to
        }
    }

    #[test]
    fn test_move32_from_to_index_matches_yane_for_all_normal_and_promotion_moves() {
        let piece = Piece::B_SILVER;
        for from_idx in 0..Square::COUNT {
            for to_idx in 0..Square::COUNT {
                if from_idx == to_idx {
                    continue;
                }

                let from = Square::from_index(from_idx);
                let to = Square::from_index(to_idx);

                let normal = Move32::normal(from, to, piece);
                assert_eq!(normal.from_to_index(), Some(yane_from_to_index(normal)));

                let promotion = Move32::promotion(from, to, piece);
                assert_eq!(promotion.from_to_index(), Some(yane_from_to_index(promotion)));
            }
        }
    }

    #[test]
    fn test_move32_from_to_index_matches_yane_for_all_drop_moves() {
        let drop_types = [
            PieceType::PAWN,
            PieceType::LANCE,
            PieceType::KNIGHT,
            PieceType::SILVER,
            PieceType::BISHOP,
            PieceType::ROOK,
            PieceType::GOLD,
        ];

        for piece_type in drop_types {
            for to_idx in 0..Square::COUNT {
                let to = Square::from_index(to_idx);
                let black = Move32::drop(piece_type, to, Color::BLACK);
                let white = Move32::drop(piece_type, to, Color::WHITE);

                assert_eq!(black.from_to_index(), Some(yane_from_to_index(black)));
                assert_eq!(white.from_to_index(), Some(yane_from_to_index(white)));
                assert_eq!(black.from_to_index(), white.from_to_index());
            }
        }
    }

    #[test]
    fn test_move32_from_to_index_rejects_special_and_invalid_moves() {
        assert_eq!(MOVE_NONE.from_to_index(), None);
        assert_eq!(MOVE_NULL.from_to_index(), None);
        assert_eq!(MOVE_RESIGN.from_to_index(), None);
        assert_eq!(MOVE_WIN.from_to_index(), None);
        assert_eq!(MOVE_END.from_to_index(), None);

        let invalid_drop = Move32::from_raw(u32::from(
            Move::MOVE_DROP | ((PieceType::KING.raw() as u16) << 7) | 0x28,
        ));
        assert_eq!(invalid_drop.from_to_index(), None);
    }

    #[test]
    fn test_move32_to_csa() {
        let from = "7g".parse::<Square>().unwrap();
        let to = "7f".parse::<Square>().unwrap();
        let m = Move32::normal(from, to, Piece::B_PAWN);
        assert_eq!(m.to_csa(), Some("7776FU".to_string()));

        let from = "2b".parse::<Square>().unwrap();
        let to = "3c".parse::<Square>().unwrap();
        let m = Move32::promotion(from, to, Piece::B_BISHOP);
        assert_eq!(m.to_csa(), Some("2233UM".to_string()));

        let drop_to = "5e".parse::<Square>().unwrap();
        let m = Move32::drop(PieceType::PAWN, drop_to, Color::BLACK);
        assert_eq!(m.to_csa(), Some("0055FU".to_string()));
    }

    #[test]
    fn test_move_to_apery_roundtrip() {
        let cases = ["7g7f", "2b3c+", "P*5e"];
        for usi in cases {
            let mv = Move::from_usi(usi).unwrap();
            let apery = mv.to_apery();
            assert_eq!(apery.to_move(), mv);
            assert_eq!(apery.to_usi(), usi);
        }
    }

    #[test]
    fn test_move_to_apery_roundtrip_special_moves() {
        let cases =
            [Move::MOVE_NONE, Move::MOVE_NULL, Move::MOVE_RESIGN, Move::MOVE_WIN, Move::MOVE_END];
        for mv in cases {
            assert_eq!(AperyMove::from_move(mv).to_move(), mv);
        }
    }

    #[test]
    fn test_apery_move_bit_layout_matches_cshogi_examples() {
        let normal = Move::from_usi("7g7f").unwrap().to_apery();
        assert_eq!(
            normal.raw(),
            (u16::try_from(Square::from_usi("7g").unwrap().raw()).unwrap() << 7)
                | u16::try_from(Square::from_usi("7f").unwrap().raw()).unwrap()
        );

        let promote = Move::from_usi("2b3c+").unwrap().to_apery();
        assert_eq!(
            promote.raw(),
            (1 << 14)
                | (u16::try_from(Square::from_usi("2b").unwrap().raw()).unwrap() << 7)
                | u16::try_from(Square::from_usi("3c").unwrap().raw()).unwrap()
        );

        let drop = Move::from_usi("P*5e").unwrap().to_apery();
        assert_eq!(
            drop.raw(),
            ((80 + PieceType::PAWN.raw() as u16) << 7)
                | u16::try_from(Square::from_usi("5e").unwrap().raw()).unwrap()
        );
        assert_eq!(drop.from_sq(), Square::NONE);
        assert_eq!(drop.dropped_piece(), Some(PieceType::PAWN));
    }

    #[test]
    fn test_position_apery_move32_conversion() {
        let mut pos = Position::empty();
        pos.set_hirate();

        let quiet = Move::from_usi("7g7f").unwrap();
        let quiet_apery = pos.apery_move32_from_move(quiet);
        assert_eq!(quiet_apery.piece_type_before(), PieceType::PAWN);
        assert_eq!(quiet_apery.captured_piece_type(), PieceType::NONE);
        assert_eq!(quiet_apery.to_move(), quiet);

        pos.apply_move32(pos.move32_from_move(quiet));
        pos.apply_move32(pos.move32_from_move(Move::from_usi("3c3d").unwrap()));
        let capture = pos.move32_from_move(Move::from_usi("8h2b+").unwrap());
        let apery_capture = pos.apery_move32_from_move32(capture);
        assert_eq!(apery_capture.piece_type_before(), PieceType::BISHOP);
        assert_eq!(apery_capture.captured_piece_type(), PieceType::BISHOP);

        let restored = pos.move32_from_apery_move32(apery_capture);
        assert_eq!(restored.to_move(), capture.to_move());
        assert_eq!(restored.piece_after_move(), capture.piece_after_move());

        let drop = Move::from_usi("P*5e").unwrap();
        let apery_drop = pos.apery_move32_from_move(drop);
        assert_eq!(apery_drop.from_sq(), Square::NONE);
        assert_eq!(apery_drop.piece_type_before(), PieceType::NONE);
        assert_eq!(apery_drop.piece_type_after(), PieceType::PAWN);
    }
}
