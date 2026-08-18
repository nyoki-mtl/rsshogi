use super::{Color, Ki2Notation, Piece, PieceType, Square};
use core::{fmt, str::FromStr};

const MASK_7: u16 = 0x7f;
const APERY_DROP_FROM: u16 = 81;

/// Number of entries in the public from-to move table.
///
/// The first 81 rows are board origins and the final seven are hand-piece
/// origins, each with 81 destination columns.
pub const FROM_TO_TABLE_SIZE: usize =
    (Square::COUNT + PieceType::HAND_TABLE_SIZE - 1) * Square::COUNT;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MoveType {
    Normal,
    Promotion,
    Drop,
}

impl MoveType {
    #[must_use]
    pub const fn from_bits(bits: u16) -> Self {
        if bits & Move::MOVE_DROP != 0 {
            Self::Drop
        } else if bits & Move::MOVE_PROMOTE != 0 {
            Self::Promotion
        } else {
            Self::Normal
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Move(u16);

pub const MOVE_NONE: Move32 = Move32::MOVE_NONE;
pub const MOVE_NULL: Move32 = Move32::MOVE_NULL;
pub const MOVE_RESIGN: Move32 = Move32::MOVE_RESIGN;
pub const MOVE_WIN: Move32 = Move32::MOVE_WIN;
pub const MOVE_END: Move32 = Move32::MOVE_END;

impl Move {
    pub const MOVE_DROP: u16 = 1 << 14;
    pub const MOVE_PROMOTE: u16 = 1 << 15;
    pub const MOVE_NONE: Self = Self(0);
    pub const MOVE_NULL: Self = Self((1 << 7) + 1);
    pub const MOVE_RESIGN: Self = Self((2 << 7) + 2);
    pub const MOVE_WIN: Self = Self((3 << 7) + 3);
    pub const MOVE_END: Self = Self((4 << 7) + 4);

    #[must_use]
    pub const fn from_raw(raw: u16) -> Self {
        Self(raw)
    }
    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }
    #[must_use]
    pub const fn normal(from: Square, to: Square) -> Self {
        Self(((from.raw() as u16 & MASK_7) << 7) | (to.raw() as u16 & MASK_7))
    }
    #[must_use]
    pub const fn promotion(from: Square, to: Square) -> Self {
        Self::from_raw(Self::normal(from, to).raw() | Self::MOVE_PROMOTE)
    }
    #[must_use]
    pub const fn drop(piece_type: PieceType, to: Square) -> Self {
        Self(
            ((piece_type.raw() as u16 & MASK_7) << 7)
                | (to.raw() as u16 & MASK_7)
                | Self::MOVE_DROP,
        )
    }
    #[must_use]
    pub const fn to_sq(self) -> Square {
        Square::new((self.0 & MASK_7) as i8)
    }
    #[must_use]
    pub const fn from_sq(self) -> Square {
        Square::new(((self.0 >> 7) & MASK_7) as i8)
    }
    #[must_use]
    pub const fn is_drop(self) -> bool {
        self.0 & Self::MOVE_DROP != 0
    }
    #[must_use]
    pub const fn is_promotion(self) -> bool {
        self.0 & Self::MOVE_PROMOTE != 0
    }
    #[must_use]
    pub const fn move_type(self) -> MoveType {
        MoveType::from_bits(self.0)
    }
    #[must_use]
    pub const fn dropped_piece(self) -> Option<PieceType> {
        if !self.is_drop() {
            return None;
        }
        let piece_type = PieceType::new(((self.0 >> 7) & MASK_7) as i8);
        if piece_type.is_hand_piece() { Some(piece_type) } else { None }
    }
    #[must_use]
    pub const fn is_normal(self) -> bool {
        if matches!(self.0, 0 | 129 | 258 | 387 | 516) {
            return false;
        }
        if !self.to_sq().is_valid() {
            return false;
        }
        if self.is_drop() {
            !self.is_promotion() && self.dropped_piece().is_some()
        } else {
            self.from_sq().is_valid() && self.from_sq().raw() != self.to_sq().raw()
        }
    }
    #[must_use]
    pub fn from_usi(usi: &str) -> Option<Self> {
        if !usi.is_ascii() {
            return None;
        }
        match usi {
            "none" => return Some(Self::MOVE_NONE),
            "null" => return Some(Self::MOVE_NULL),
            "0000" => return Some(Self::MOVE_NULL),
            "resign" => return Some(Self::MOVE_RESIGN),
            "win" => return Some(Self::MOVE_WIN),
            "end" => return Some(Self::MOVE_END),
            _ => {}
        }
        let bytes = usi.as_bytes();
        if bytes.len() == 4 && bytes[1] == b'*' {
            let piece_type = PieceType::from_str(&(bytes[0] as char).to_string()).ok()?;
            let to = parse_usi_square(&bytes[2..4])?;
            let mv = Self::drop(piece_type, to);
            return mv.is_normal().then_some(mv);
        }
        let (body, promote) = match bytes {
            [a, b, c, d] => (&[*a, *b, *c, *d][..], false),
            [a, b, c, d, b'+'] => (&[*a, *b, *c, *d][..], true),
            _ => return None,
        };
        let from = parse_usi_square(&body[..2])?;
        let to = parse_usi_square(&body[2..])?;
        let mv = if promote { Self::promotion(from, to) } else { Self::normal(from, to) };
        mv.is_normal().then_some(mv)
    }
    #[must_use]
    pub fn to_usi(self) -> String {
        match self {
            Self::MOVE_NONE => return "none".to_string(),
            Self::MOVE_NULL => return "null".to_string(),
            Self::MOVE_RESIGN => return "resign".to_string(),
            Self::MOVE_WIN => return "win".to_string(),
            Self::MOVE_END => return "end".to_string(),
            _ => {}
        }
        if !self.is_normal() {
            return "none".to_string();
        }
        if let Some(piece_type) = self.dropped_piece() {
            return format!("{piece_type}*{}", self.to_sq());
        }
        let suffix = if self.is_promotion() { "+" } else { "" };
        format!("{}{}{}", self.from_sq(), self.to_sq(), suffix)
    }
    #[must_use]
    pub const fn to_apery(self) -> AperyMove {
        AperyMove::from_move(self)
    }
    #[must_use]
    pub fn to_ki2(self, position: &crate::board::Position) -> Option<String> {
        self.to_ki2_notation(position).map(|notation| notation.to_string())
    }
    /// 局面 `position` でこの着手を指したときの KI2 記法を返す。
    #[must_use]
    pub fn to_ki2_notation(self, position: &crate::board::Position) -> Option<Ki2Notation> {
        Move32::from_move_with_piece(self, position.moved_piece_after_move(self))
            .to_ki2_notation(position)
    }
}

impl fmt::Debug for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Move").field(&self.to_usi()).finish()
    }
}
impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_usi())
    }
}
impl FromStr for Move {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_usi(s).ok_or("invalid USI move")
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Move32(u32);

impl Move32 {
    pub const MOVE_NONE: Self = Self(0);
    pub const MOVE_NULL: Self = Self(129);
    pub const MOVE_RESIGN: Self = Self(258);
    pub const MOVE_WIN: Self = Self(387);
    pub const MOVE_END: Self = Self(516);

    #[must_use]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }
    #[must_use]
    pub const fn to_move(self) -> Move {
        Move::from_raw(self.0 as u16)
    }
    #[must_use]
    pub const fn normal(from: Square, to: Square, piece: Piece) -> Self {
        Self::from_move_with_piece(Move::normal(from, to), piece)
    }
    #[must_use]
    pub const fn promotion(from: Square, to: Square, piece: Piece) -> Self {
        Self::from_move_with_piece(Move::promotion(from, to), piece.promote())
    }
    #[must_use]
    pub const fn drop(piece_type: PieceType, to: Square, color: Color) -> Self {
        Self::from_move_with_piece(Move::drop(piece_type, to), Piece::from_parts(color, piece_type))
    }
    #[must_use]
    pub const fn from_move_with_piece(mv: Move, piece_after: Piece) -> Self {
        Self(mv.raw() as u32 | ((piece_after.raw() as u32 & 0x1f) << 16))
    }
    #[must_use]
    pub const fn to_sq(self) -> Square {
        self.to_move().to_sq()
    }
    #[must_use]
    pub const fn from_sq(self) -> Square {
        self.to_move().from_sq()
    }
    #[must_use]
    pub const fn is_drop(self) -> bool {
        self.to_move().is_drop()
    }
    #[must_use]
    pub const fn is_promotion(self) -> bool {
        self.to_move().is_promotion()
    }
    #[must_use]
    pub const fn is_normal(self) -> bool {
        self.to_move().is_normal()
    }
    #[must_use]
    pub const fn move_type(self) -> MoveType {
        self.to_move().move_type()
    }
    #[must_use]
    pub const fn dropped_piece(self) -> Option<PieceType> {
        self.to_move().dropped_piece()
    }
    #[must_use]
    pub const fn piece_after_move(self) -> Piece {
        Piece::new(((self.0 >> 16) & 0x1f) as i8)
    }
    #[must_use]
    pub const fn has_piece_info(self) -> bool {
        self.piece_after_move().is_valid() && !self.piece_after_move().is_empty()
    }
    pub const FROM_TO_TABLE_SIZE: usize = crate::types::moves::FROM_TO_TABLE_SIZE;
    #[must_use]
    pub const fn from_index(self) -> usize {
        ((self.0 >> 7) as u16 & MASK_7) as usize
    }
    #[must_use]
    pub const fn to_index(self) -> usize {
        self.to_sq().to_index()
    }
    #[must_use]
    pub const fn from_to_index(self) -> Option<usize> {
        if !self.is_normal() {
            None
        } else {
            let from_row = if self.is_drop() {
                Square::COUNT + self.from_index() - PieceType::PAWN.raw() as usize
            } else {
                self.from_index()
            };
            Some(from_row * Square::COUNT + self.to_index())
        }
    }
    #[must_use]
    pub fn from_usi(usi: &str) -> Option<Self> {
        Move::from_usi(usi).map(|mv| Self::from_raw(mv.raw() as u32))
    }
    #[must_use]
    pub fn to_usi(self) -> String {
        self.to_move().to_usi()
    }
    #[must_use]
    pub fn to_apery(self, position: &crate::board::Position) -> AperyMove32 {
        position.apery_move32_from_move32(self)
    }
    #[must_use]
    pub fn to_csa(self) -> Option<String> {
        if !self.is_normal() || !self.has_piece_info() {
            return None;
        }
        let piece = self.piece_after_move();
        let side = if piece.color() == Color::BLACK { '+' } else { '-' };
        let (from_file, from_rank) = if self.is_drop() {
            (0, 0)
        } else {
            (self.from_sq().file().raw() + 1, self.from_sq().rank().raw() + 1)
        };
        let to = self.to_sq();
        Some(format!(
            "{side}{from_file}{from_rank}{}{}{}",
            to.file().raw() + 1,
            to.rank().raw() + 1,
            csa_piece(piece.piece_type())?
        ))
    }
    #[must_use]
    pub fn to_ki2(self, position: &crate::board::Position) -> Option<String> {
        self.to_ki2_notation(position).map(|notation| notation.to_string())
    }
    /// 局面 `position` でこの着手を指したときの KI2 記法を返す。
    #[must_use]
    pub fn to_ki2_notation(self, position: &crate::board::Position) -> Option<Ki2Notation> {
        Ki2Notation::from_move32(self, position)
    }
}

impl fmt::Debug for Move32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Move32").field(&self.to_usi()).finish()
    }
}
impl fmt::Display for Move32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_usi())
    }
}
impl FromStr for Move32 {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_usi(s).ok_or("invalid USI move")
    }
}
impl From<Move> for Move32 {
    fn from(value: Move) -> Self {
        Self::from_raw(value.raw() as u32)
    }
}
impl From<Move32> for Move {
    fn from(value: Move32) -> Self {
        value.to_move()
    }
}

pub(crate) trait MoveBase: Copy {
    fn to_move(self) -> Move;
    fn piece_after_hint(self) -> Piece;
}
impl MoveBase for Move {
    fn to_move(self) -> Move {
        self
    }
    fn piece_after_hint(self) -> Piece {
        Piece::NONE
    }
}
impl MoveBase for Move32 {
    fn to_move(self) -> Move {
        self.to_move()
    }
    fn piece_after_hint(self) -> Piece {
        if self.has_piece_info() { self.piece_after_move() } else { Piece::NONE }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Move32Metadata {
    piece_after_move: Piece,
    captured_piece: Piece,
    is_capture: bool,
    is_promotion: bool,
    is_drop: bool,
    from_to_index: u16,
}
impl Move32Metadata {
    const NO_FROM_TO_INDEX: u16 = u16::MAX;

    const fn encode_from_to_index(index: Option<usize>) -> u16 {
        match index {
            Some(index) if index < FROM_TO_TABLE_SIZE => index as u16,
            _ => Self::NO_FROM_TO_INDEX,
        }
    }

    #[must_use]
    pub const fn new(
        piece_after_move: Piece,
        captured_piece: Piece,
        from_to_index: Option<usize>,
    ) -> Self {
        Self {
            piece_after_move,
            captured_piece,
            is_capture: !captured_piece.is_empty(),
            is_promotion: false,
            is_drop: false,
            from_to_index: Self::encode_from_to_index(from_to_index),
        }
    }
    #[must_use]
    pub const fn with_from_to_index(
        piece_after_move: Piece,
        captured_piece: Piece,
        is_capture: bool,
        is_promotion: bool,
        is_drop: bool,
        from_to_index: Option<usize>,
    ) -> Self {
        Self {
            piece_after_move,
            captured_piece,
            is_capture,
            is_promotion,
            is_drop,
            from_to_index: Self::encode_from_to_index(from_to_index),
        }
    }
    #[must_use]
    pub const fn piece_after_move(self) -> Piece {
        self.piece_after_move
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
    #[must_use]
    pub const fn from_to_index(self) -> Option<usize> {
        if self.from_to_index == Self::NO_FROM_TO_INDEX {
            None
        } else {
            Some(self.from_to_index as usize)
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AperyMove(u16);
impl AperyMove {
    #[must_use]
    pub const fn from_raw(raw: u16) -> Self {
        Self(raw)
    }
    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }
    #[must_use]
    pub const fn from_move(mv: Move) -> Self {
        if !mv.is_drop() {
            let promotion = if mv.is_promotion() { 1 << 14 } else { 0 };
            return Self((mv.raw() & !(Move::MOVE_DROP | Move::MOVE_PROMOTE)) | promotion);
        }
        let piece_type = match mv.dropped_piece() {
            Some(piece_type) => piece_type,
            None => PieceType::NONE,
        };
        let source = APERY_DROP_FROM + piece_type.raw() as u16 - 1;
        let promotion = if mv.is_promotion() { 1 << 14 } else { 0 };
        Self((mv.to_sq().raw() as u16 & MASK_7) | (source << 7) | promotion)
    }
    #[must_use]
    pub const fn to_move(self) -> Move {
        let source = (self.0 >> 7) & MASK_7;
        let to = Square::new((self.0 & MASK_7) as i8);
        if source >= APERY_DROP_FROM {
            let mut mv = Move::drop(PieceType::new((source - APERY_DROP_FROM + 1) as i8), to);
            if self.is_promotion() {
                mv = Move::from_raw(mv.raw() | Move::MOVE_PROMOTE);
            }
            return mv;
        }
        let promotion = if self.is_promotion() { Move::MOVE_PROMOTE } else { 0 };
        Move::from_raw((self.0 & !(1 << 14)) | promotion)
    }
    #[must_use]
    pub const fn to_sq(self) -> Square {
        Square::new((self.0 & MASK_7) as i8)
    }
    #[must_use]
    pub const fn from_sq(self) -> Square {
        let source = (self.0 >> 7) & MASK_7;
        if source >= APERY_DROP_FROM { Square::NONE } else { Square::new(source as i8) }
    }
    #[must_use]
    pub const fn is_drop(self) -> bool {
        ((self.0 >> 7) & MASK_7) >= APERY_DROP_FROM
    }
    #[must_use]
    pub const fn is_promotion(self) -> bool {
        self.0 & (1 << 14) != 0
    }
    #[must_use]
    pub const fn move_type(self) -> MoveType {
        self.to_move().move_type()
    }
    #[must_use]
    pub const fn dropped_piece(self) -> Option<PieceType> {
        if !self.is_drop() {
            None
        } else {
            let pt = PieceType::new((((self.0 >> 7) & MASK_7) - APERY_DROP_FROM + 1) as i8);
            if pt.is_hand_piece() { Some(pt) } else { None }
        }
    }
    #[must_use]
    pub const fn is_normal(self) -> bool {
        !(self.is_drop() && self.is_promotion()) && self.to_move().is_normal()
    }
    #[must_use]
    pub fn from_usi(usi: &str) -> Option<Self> {
        Move::from_usi(usi).map(Self::from_move)
    }
    #[must_use]
    pub fn to_usi(self) -> String {
        self.to_move().to_usi()
    }
}
impl fmt::Debug for AperyMove {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AperyMove").field(&self.to_usi()).finish()
    }
}
impl fmt::Display for AperyMove {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_usi())
    }
}
impl FromStr for AperyMove {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_usi(s).ok_or("invalid USI move")
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AperyMove32(u32);
impl AperyMove32 {
    #[must_use]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }
    #[must_use]
    pub const fn to_apery_move(self) -> AperyMove {
        AperyMove::from_raw(self.0 as u16)
    }
    #[must_use]
    pub const fn to_move(self) -> Move {
        self.to_apery_move().to_move()
    }
    #[must_use]
    pub const fn to_sq(self) -> Square {
        self.to_apery_move().to_sq()
    }
    #[must_use]
    pub const fn from_sq(self) -> Square {
        self.to_apery_move().from_sq()
    }
    #[must_use]
    pub const fn is_drop(self) -> bool {
        self.to_apery_move().is_drop()
    }
    #[must_use]
    pub const fn is_promotion(self) -> bool {
        self.to_apery_move().is_promotion()
    }
    #[must_use]
    pub const fn is_normal(self) -> bool {
        self.to_apery_move().is_normal()
    }
    #[must_use]
    pub const fn is_capture(self) -> bool {
        ((self.0 >> 20) & 0xf) != 0
    }
    #[must_use]
    pub const fn move_type(self) -> MoveType {
        self.to_apery_move().move_type()
    }
    #[must_use]
    pub const fn dropped_piece(self) -> Option<PieceType> {
        self.to_apery_move().dropped_piece()
    }
    #[must_use]
    pub const fn piece_type_before(self) -> PieceType {
        if self.is_drop() { PieceType::NONE } else { PieceType::new(((self.0 >> 16) & 0xf) as i8) }
    }
    #[must_use]
    pub const fn piece_type_after(self) -> PieceType {
        if self.is_drop() {
            match self.dropped_piece() {
                Some(piece_type) => piece_type,
                None => PieceType::NONE,
            }
        } else if self.is_promotion() {
            self.piece_type_before().promote()
        } else {
            self.piece_type_before()
        }
    }
    #[must_use]
    pub const fn captured_piece_type(self) -> PieceType {
        PieceType::new(((self.0 >> 20) & 0xf) as i8)
    }
    #[must_use]
    pub fn to_usi(self) -> String {
        self.to_move().to_usi()
    }
}
impl fmt::Debug for AperyMove32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AperyMove32").field(&self.to_usi()).finish()
    }
}
impl fmt::Display for AperyMove32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_usi())
    }
}

fn parse_usi_square(bytes: &[u8]) -> Option<Square> {
    match bytes {
        [file @ b'1'..=b'9', rank @ b'a'..=b'i'] => Some(Square::from_file_rank(
            super::File::new((file - b'1') as i8),
            super::Rank::new((rank - b'a') as i8),
        )),
        _ => None,
    }
}
fn csa_piece(piece_type: PieceType) -> Option<&'static str> {
    Some(match piece_type {
        PieceType::PAWN => "FU",
        PieceType::LANCE => "KY",
        PieceType::KNIGHT => "KE",
        PieceType::SILVER => "GI",
        PieceType::GOLD => "KI",
        PieceType::BISHOP => "KA",
        PieceType::ROOK => "HI",
        PieceType::KING => "OU",
        PieceType::PRO_PAWN => "TO",
        PieceType::PRO_LANCE => "NY",
        PieceType::PRO_KNIGHT => "NK",
        PieceType::PRO_SILVER => "NG",
        PieceType::HORSE => "UM",
        PieceType::DRAGON => "RY",
        _ => return None,
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move32_from_to_index_includes_drop_rows_and_public_accessors() {
        let from = Square::from_usi("7g").expect("valid square");
        let to = Square::from_usi("5e").expect("valid square");
        let normal = Move32::normal(from, to, Piece::B_PAWN);
        let drop = Move32::drop(PieceType::GOLD, to, Color::BLACK);

        assert_eq!(FROM_TO_TABLE_SIZE, 88 * Square::COUNT);
        assert_eq!(Move32::FROM_TO_TABLE_SIZE, FROM_TO_TABLE_SIZE);
        assert_eq!(normal.from_index(), from.to_index());
        assert_eq!(normal.to_index(), to.to_index());
        assert_eq!(normal.from_to_index(), Some(from.to_index() * Square::COUNT + to.to_index()));
        assert_eq!(drop.from_index(), PieceType::GOLD.raw() as usize);
        assert_eq!(drop.to_index(), to.to_index());
        assert_eq!(drop.from_to_index(), Some((Square::COUNT + 6) * Square::COUNT + to.to_index()));
        let position = crate::board::hirate_position();
        let position_move = position.move32_from_move(Move::from_usi("7g7f").expect("valid move"));
        assert_eq!(
            position_move.to_apery(&position),
            position.apery_move32_from_move32(position_move)
        );
        assert_eq!(MoveType::from_bits(0), MoveType::Normal);
        assert_eq!(MoveType::from_bits(Move::MOVE_PROMOTE), MoveType::Promotion);
        assert_eq!(MoveType::from_bits(Move::MOVE_DROP), MoveType::Drop);
    }

    #[test]
    fn move32_metadata_uses_a_compact_optional_index_sentinel() {
        let metadata = Move32Metadata::new(Piece::B_PAWN, Piece::NONE, Some(123));
        let absent = Move32Metadata::with_from_to_index(
            Piece::B_PAWN,
            Piece::NONE,
            false,
            false,
            false,
            None,
        );
        let out_of_range =
            Move32Metadata::new(Piece::B_PAWN, Piece::NONE, Some(FROM_TO_TABLE_SIZE));

        assert!(core::mem::size_of::<Move32Metadata>() <= 8);
        assert_eq!(metadata.from_to_index(), Some(123));
        assert_eq!(absent.from_to_index(), None);
        assert_eq!(out_of_range.from_to_index(), None);
    }

    #[test]
    fn apery_rejects_and_roundtrips_malformed_drop_promotion_bits() {
        let to = Square::from_usi("5e").expect("valid square");
        let malformed = AperyMove::from_raw(
            AperyMove::from_move(Move::drop(PieceType::PAWN, to)).raw() | (1 << 14),
        );

        assert!(malformed.is_drop());
        assert!(malformed.is_promotion());
        assert!(!malformed.is_normal());
        assert!(!malformed.to_move().is_normal());
        assert_eq!(AperyMove::from_move(malformed.to_move()).raw(), malformed.raw());
        assert_eq!(malformed.to_usi(), "none");
    }

    #[test]
    fn apery_normal_promotion_translates_the_promotion_bit() {
        let from = Square::from_usi("7c").expect("valid square");
        let to = Square::from_usi("7b").expect("valid square");
        let promotion = Move::promotion(from, to);
        let apery = AperyMove::from_move(promotion);

        assert_eq!(
            apery.raw(),
            (promotion.raw() & !(Move::MOVE_DROP | Move::MOVE_PROMOTE)) | (1 << 14)
        );
        assert!(apery.is_promotion());
        assert_eq!(apery.move_type(), MoveType::Promotion);
        assert!(apery.is_normal());
        assert_eq!(apery.to_move(), promotion);
        assert_eq!(AperyMove::from_move(apery.to_move()), apery);
    }

    #[test]
    fn from_usi_rejects_trailing_tokens_and_non_ascii() {
        assert_eq!(Move::from_usi("end"), Some(Move::MOVE_END));
        assert_eq!(Move::from_usi("0000"), Some(Move::MOVE_NULL));
        assert_eq!(Move::from_usi("7g7f 3c3d"), None);
        assert_eq!(Move::from_usi("P*5e\n"), None);
        assert_eq!(Move::from_usi("７g7f"), None);
    }

    #[test]
    fn ki2_drop_marker_is_only_used_for_board_move_disambiguation() {
        let existing_piece = crate::board::position_from_sfen(
            "ln1g4l/1ks6/1pp3n1p/p2pS4/9/P1P3P1P/1P1P2N2/2K1p4/LN1GP2+rL b RB2SPb2g4p 1",
        )
        .expect("valid sfen");
        let ambiguous_drop =
            existing_piece.move32_from_move(Move::from_usi("S*6c").expect("valid move"));
        assert_eq!(ambiguous_drop.to_ki2(&existing_piece), Some("▲６三銀打".to_string()));

        let unique_position =
            crate::board::position_from_sfen("4k4/9/9/9/9/9/9/9/4K4 b S 1").expect("valid sfen");
        let unique_drop =
            unique_position.move32_from_move(Move::from_usi("S*5e").expect("valid move"));
        assert_eq!(unique_drop.to_ki2(&unique_position), Some("▲５五銀".to_string()));
    }

    #[test]
    fn ki2_non_promotion_marks_available_promotion_choice() {
        let position =
            crate::board::position_from_sfen("k8/4B4/9/9/9/9/9/9/4K4 b - 1").expect("valid sfen");
        let move32 = position.move32_from_move(Move::from_usi("5b4c").expect("valid move"));
        assert!(position.is_legal_move(Move::from_usi("5b4c+").expect("valid move")));
        assert_eq!(move32.to_ki2(&position), Some("▲４三角不成".to_string()));
    }

    #[test]
    fn ki2_pads_three_character_same_destination_moves() {
        let mut position = crate::board::hirate_position();
        for usi in ["7g7f", "3c3d", "8h2b+"] {
            position.apply_move(Move::from_usi(usi).expect("valid move"));
        }
        let move32 = position.move32_from_move(Move::from_usi("3a2b").expect("valid move"));
        assert_eq!(move32.to_ki2(&position), Some("△同　銀".to_string()));
    }

    #[test]
    fn ki2_disambiguation_keeps_pinned_board_candidates() {
        let position = crate::board::position_from_sfen("k3r4/9/9/9/9/9/9/4GG3/4K4 b G 1")
            .expect("valid sfen");
        let board_move = position.move32_from_move(Move::from_usi("4h4g").expect("valid move"));
        assert_eq!(board_move.to_ki2(&position), Some("▲４七金直".to_string()));

        let drop = position.move32_from_move(Move::from_usi("G*4g").expect("valid move"));
        assert_eq!(drop.to_ki2(&position), Some("▲４七金打".to_string()));
    }
}
