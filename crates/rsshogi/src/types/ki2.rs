//! 1 手分の KI2 記法を表す値型。
//!
//! [`Ki2Notation`] は、KI2 文字列を構成する要素をヒープ確保なしで保持する。
//! 表示、比較、ハッシュ、文字列からの解析、局面上の合法手への解決に使える。

use core::{fmt, str::FromStr};

use crate::board::{self, Move32List, Position};
use crate::types::{Bitboard, Color, File, Move, Move32, PieceType, Rank, Square};

const SIDE_SHIFT: u32 = 0;
const SAME_SHIFT: u32 = 1;
const SQUARE_SHIFT: u32 = 2;
const PIECE_SHIFT: u32 = 9;
const SUFFIX_SHIFT: u32 = 13;
const PROMOTION_SHIFT: u32 = 17;

/// 1 手分の正規化された KI2 記法。
///
/// 値の等価性は [`Move32::to_ki2`] が返す文字列の等価性と一致する。
/// `同` と表記される場合は移動先が文字列に現れないため、この値も移動先を保持しない。
/// 着手へ戻すときは [`Ki2Notation::to_move32`] に局面を渡す必要がある。
///
/// # Examples
///
/// ```
/// use rsshogi::board;
/// use rsshogi::types::Ki2Notation;
///
/// board::init();
/// let position = board::hirate_position();
/// let mv = board::move_from_usi_expect(&position, "7g7f");
/// let notation = mv.to_ki2_notation(&position).expect("legal move has KI2 notation");
///
/// assert_eq!(notation.to_string(), "▲７六歩");
/// assert_eq!("▲７六歩".parse::<Ki2Notation>(), Ok(notation));
/// assert_eq!(notation.to_move32(&position), Ok(mv));
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ki2Notation(u32);

/// 1 手分の KI2 記法を解析できなかったことを表すエラー。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ParseKi2NotationError;

impl fmt::Display for ParseKi2NotationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid KI2 notation")
    }
}

impl std::error::Error for ParseKi2NotationError {}

/// KI2 記法を局面上の合法手へ一意に解決できなかったことを表すエラー。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ki2ResolveError {
    /// 記法に一致する合法手がない。
    NoMatchingMove,
    /// 記法に一致する合法手が複数ある。
    Ambiguous,
}

impl fmt::Display for Ki2ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NoMatchingMove => "KI2 notation does not match a legal move",
            Self::Ambiguous => "KI2 notation matches multiple legal moves",
        })
    }
}

impl std::error::Error for Ki2ResolveError {}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Ki2Suffix {
    None,
    Advance,
    Retreat,
    Sideways,
    Straight,
    Right,
    Left,
    RightAdvance,
    RightRetreat,
    LeftAdvance,
    LeftRetreat,
    Drop,
}

impl Ki2Suffix {
    const fn code(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Advance => 1,
            Self::Retreat => 2,
            Self::Sideways => 3,
            Self::Straight => 4,
            Self::Right => 5,
            Self::Left => 6,
            Self::RightAdvance => 7,
            Self::RightRetreat => 8,
            Self::LeftAdvance => 9,
            Self::LeftRetreat => 10,
            Self::Drop => 11,
        }
    }

    const fn text(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Advance => "上",
            Self::Retreat => "引",
            Self::Sideways => "寄",
            Self::Straight => "直",
            Self::Right => "右",
            Self::Left => "左",
            Self::RightAdvance => "右上",
            Self::RightRetreat => "右引",
            Self::LeftAdvance => "左上",
            Self::LeftRetreat => "左引",
            Self::Drop => "打",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "" => Self::None,
            "上" => Self::Advance,
            "引" => Self::Retreat,
            "寄" => Self::Sideways,
            "直" => Self::Straight,
            "右" => Self::Right,
            "左" => Self::Left,
            "右上" => Self::RightAdvance,
            "右引" => Self::RightRetreat,
            "左上" => Self::LeftAdvance,
            "左引" => Self::LeftRetreat,
            "打" => Self::Drop,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Ki2Promotion {
    None,
    Promote,
    Decline,
}

impl Ki2Promotion {
    const fn code(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Promote => 1,
            Self::Decline => 2,
        }
    }

    const fn text(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Promote => "成",
            Self::Decline => "不成",
        }
    }
}

impl Ki2Notation {
    /// 局面 `position` で着手 `mv` を指したときの KI2 記法を返す。
    ///
    /// 通常手でない着手、または駒情報を取得できない着手では `None` を返す。
    #[must_use]
    pub fn from_move32(mv: Move32, position: &Position) -> Option<Self> {
        if !mv.is_normal() {
            return None;
        }
        let piece = if mv.has_piece_info() {
            mv.piece_after_move()
        } else {
            position.moved_piece_after(mv)
        };
        if !piece.is_valid() || piece.is_empty() || !mv.to_sq().is_valid() {
            return None;
        }

        let side = piece.color();
        let to = mv.to_sq();
        let same = position.last_move().is_normal() && position.last_move().to_sq() == to;
        let displayed =
            if mv.is_promotion() { piece.piece_type().demote() } else { piece.piece_type() };
        piece_text(displayed)?;

        let (suffix, promotion) = if mv.is_drop() {
            let suffix = if board_candidates(position, to, side, piece.piece_type()).is_empty() {
                Ki2Suffix::None
            } else {
                Ki2Suffix::Drop
            };
            (suffix, Ki2Promotion::None)
        } else {
            let from = mv.from_sq();
            if !from.is_valid() {
                return None;
            }
            let suffix = suffix_of(
                from,
                to,
                side,
                displayed,
                board_candidates(position, to, side, displayed),
            );
            let promotion = if mv.is_promotion() {
                Ki2Promotion::Promote
            } else if position.is_legal_move(Move::promotion(from, to)) {
                Ki2Promotion::Decline
            } else {
                Ki2Promotion::None
            };
            (suffix, promotion)
        };

        Some(Self::pack(side, same, to, displayed, suffix, promotion))
    }

    /// 現在局面でこの記法に一致する合法手を返す。
    ///
    /// # Errors
    ///
    /// 一致する合法手がない場合は [`Ki2ResolveError::NoMatchingMove`]、複数ある場合は
    /// [`Ki2ResolveError::Ambiguous`] を返す。
    pub fn to_move32(self, position: &Position) -> Result<Move32, Ki2ResolveError> {
        let mut moves = Move32List::new();
        board::generate_legal_all_move32(position, &mut moves);
        let mut matched =
            moves.iter().copied().filter(|&mv| Self::from_move32(mv, position) == Some(self));
        let result = matched.next().ok_or(Ki2ResolveError::NoMatchingMove)?;
        if matched.next().is_some() {
            return Err(Ki2ResolveError::Ambiguous);
        }
        Ok(result)
    }

    fn pack(
        side: Color,
        same: bool,
        to: Square,
        displayed: PieceType,
        suffix: Ki2Suffix,
        promotion: Ki2Promotion,
    ) -> Self {
        let square = if same { 0 } else { (to.to_index() as u32) + 1 };
        Self(
            (u32::from(side == Color::WHITE) << SIDE_SHIFT)
                | (u32::from(same) << SAME_SHIFT)
                | (square << SQUARE_SHIFT)
                | ((displayed.raw() as u32) << PIECE_SHIFT)
                | (suffix.code() << SUFFIX_SHIFT)
                | (promotion.code() << PROMOTION_SHIFT),
        )
    }

    fn side(self) -> Color {
        if (self.0 >> SIDE_SHIFT) & 1 == 0 { Color::BLACK } else { Color::WHITE }
    }

    fn is_same_destination(self) -> bool {
        (self.0 >> SAME_SHIFT) & 1 == 1
    }

    fn square(self) -> Square {
        let raw = (self.0 >> SQUARE_SHIFT) & 0x7f;
        debug_assert!(!self.is_same_destination() && (1..=Square::COUNT as u32).contains(&raw));
        Square::from_index((raw - 1) as usize)
    }

    fn piece_type(self) -> PieceType {
        PieceType::new(((self.0 >> PIECE_SHIFT) & 0x0f) as i8)
    }

    fn suffix(self) -> Ki2Suffix {
        match (self.0 >> SUFFIX_SHIFT) & 0x0f {
            0 => Ki2Suffix::None,
            1 => Ki2Suffix::Advance,
            2 => Ki2Suffix::Retreat,
            3 => Ki2Suffix::Sideways,
            4 => Ki2Suffix::Straight,
            5 => Ki2Suffix::Right,
            6 => Ki2Suffix::Left,
            7 => Ki2Suffix::RightAdvance,
            8 => Ki2Suffix::RightRetreat,
            9 => Ki2Suffix::LeftAdvance,
            10 => Ki2Suffix::LeftRetreat,
            11 => Ki2Suffix::Drop,
            _ => unreachable!("Ki2Notation contains a valid suffix"),
        }
    }

    fn promotion(self) -> Ki2Promotion {
        match (self.0 >> PROMOTION_SHIFT) & 0x03 {
            0 => Ki2Promotion::None,
            1 => Ki2Promotion::Promote,
            2 => Ki2Promotion::Decline,
            _ => unreachable!("Ki2Notation contains a valid promotion"),
        }
    }
}

impl fmt::Display for Ki2Notation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.side() == Color::BLACK { "▲" } else { "△" })?;
        if self.is_same_destination() {
            f.write_str("同")?;
            if self.suffix() == Ki2Suffix::None && self.promotion() == Ki2Promotion::None {
                f.write_str("　")?;
            }
        } else {
            let square = self.square();
            write!(
                f,
                "{}{}",
                wide_digit(square.file().raw() + 1),
                kanji_rank(square.rank().raw() + 1)
            )?;
        }
        f.write_str(piece_text(self.piece_type()).expect("Ki2Notation contains a valid piece"))?;
        f.write_str(self.suffix().text())?;
        f.write_str(self.promotion().text())
    }
}

impl fmt::Debug for Ki2Notation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Ki2Notation(\"{self}\")")
    }
}

impl FromStr for Ki2Notation {
    type Err = ParseKi2NotationError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut chars = text.chars();
        let side = match chars.next() {
            Some('▲') => Color::BLACK,
            Some('△') => Color::WHITE,
            _ => return Err(ParseKi2NotationError),
        };
        let first_destination = chars.next().ok_or(ParseKi2NotationError)?;
        let (same, to) = if first_destination == '同' {
            if chars.as_str().starts_with('　') {
                chars.next();
            }
            (true, Square::from_index(0))
        } else {
            let file = file_from_wide_digit(first_destination).ok_or(ParseKi2NotationError)?;
            let rank = rank_from_kanji(chars.next().ok_or(ParseKi2NotationError)?)
                .ok_or(ParseKi2NotationError)?;
            (false, Square::from_file_rank(file, rank))
        };
        let piece = piece_from_text(chars.next().ok_or(ParseKi2NotationError)?)
            .ok_or(ParseKi2NotationError)?;
        let tail = chars.as_str();
        let (suffix_text, promotion) = if let Some(value) = tail.strip_suffix("不成") {
            (value, Ki2Promotion::Decline)
        } else if let Some(value) = tail.strip_suffix('成') {
            (value, Ki2Promotion::Promote)
        } else {
            (tail, Ki2Promotion::None)
        };
        let suffix = Ki2Suffix::parse(suffix_text).ok_or(ParseKi2NotationError)?;
        if suffix == Ki2Suffix::Drop && promotion != Ki2Promotion::None {
            return Err(ParseKi2NotationError);
        }
        if suffix == Ki2Suffix::Drop && !piece.is_hand_piece() {
            return Err(ParseKi2NotationError);
        }
        if promotion != Ki2Promotion::None && !piece.is_promotable() {
            return Err(ParseKi2NotationError);
        }
        Ok(Self::pack(side, same, to, piece, suffix, promotion))
    }
}

fn board_candidates(
    position: &Position,
    to: Square,
    side: Color,
    piece_type: PieceType,
) -> Bitboard {
    position.attackers_to_color_current(side, to) & position.pieces_for(piece_type, side)
}

fn suffix_of(
    from: Square,
    to: Square,
    side: Color,
    piece_type: PieceType,
    candidates: Bitboard,
) -> Ki2Suffix {
    if candidates.count() <= 1 {
        return Ki2Suffix::None;
    }
    let horizontal = horizontal_of(from, to, side);
    let motion = motion_of(from, to, side);
    if candidates.iter().filter(|&square| motion_of(square, to, side) == motion).count() == 1 {
        return motion;
    }
    if horizontal == Ki2Suffix::Straight {
        if matches!(
            piece_type,
            PieceType::LANCE
                | PieceType::BISHOP
                | PieceType::ROOK
                | PieceType::HORSE
                | PieceType::DRAGON
        ) {
            if candidates.iter().any(|square| horizontal_of(square, to, side) == Ki2Suffix::Right) {
                return Ki2Suffix::Left;
            }
            if candidates.iter().any(|square| horizontal_of(square, to, side) == Ki2Suffix::Left) {
                return Ki2Suffix::Right;
            }
        }
        return Ki2Suffix::Straight;
    }
    let same_horizontal =
        candidates.iter().filter(|&square| horizontal_of(square, to, side) == horizontal).count();
    if same_horizontal > 1 { combine(horizontal, motion) } else { horizontal }
}

fn horizontal_of(from: Square, to: Square, side: Color) -> Ki2Suffix {
    let (from_file, _) = oriented(from, side);
    let (to_file, _) = oriented(to, side);
    if from_file < to_file {
        Ki2Suffix::Right
    } else if from_file > to_file {
        Ki2Suffix::Left
    } else {
        Ki2Suffix::Straight
    }
}

fn motion_of(from: Square, to: Square, side: Color) -> Ki2Suffix {
    let (_, from_rank) = oriented(from, side);
    let (_, to_rank) = oriented(to, side);
    if from_rank < to_rank {
        Ki2Suffix::Retreat
    } else if from_rank > to_rank {
        Ki2Suffix::Advance
    } else {
        Ki2Suffix::Sideways
    }
}

const fn combine(horizontal: Ki2Suffix, motion: Ki2Suffix) -> Ki2Suffix {
    match (horizontal, motion) {
        (Ki2Suffix::Right, Ki2Suffix::Advance) => Ki2Suffix::RightAdvance,
        (Ki2Suffix::Right, Ki2Suffix::Retreat) => Ki2Suffix::RightRetreat,
        (Ki2Suffix::Left, Ki2Suffix::Advance) => Ki2Suffix::LeftAdvance,
        (Ki2Suffix::Left, Ki2Suffix::Retreat) => Ki2Suffix::LeftRetreat,
        _ => horizontal,
    }
}

fn oriented(square: Square, side: Color) -> (i8, i8) {
    if side == Color::BLACK {
        (square.file().raw(), square.rank().raw())
    } else {
        (8 - square.file().raw(), 8 - square.rank().raw())
    }
}

fn wide_digit(value: i8) -> char {
    ['１', '２', '３', '４', '５', '６', '７', '８', '９'][(value - 1) as usize]
}

fn file_from_wide_digit(value: char) -> Option<File> {
    "１２３４５６７８９"
        .chars()
        .position(|candidate| candidate == value)
        .map(|raw| File::new(raw as i8))
}

fn kanji_rank(value: i8) -> char {
    ['一', '二', '三', '四', '五', '六', '七', '八', '九'][(value - 1) as usize]
}

fn rank_from_kanji(value: char) -> Option<Rank> {
    "一二三四五六七八九"
        .chars()
        .position(|candidate| candidate == value)
        .map(|raw| Rank::new(raw as i8))
}

fn piece_text(piece_type: PieceType) -> Option<&'static str> {
    Some(match piece_type {
        PieceType::PAWN => "歩",
        PieceType::LANCE => "香",
        PieceType::KNIGHT => "桂",
        PieceType::SILVER => "銀",
        PieceType::GOLD => "金",
        PieceType::BISHOP => "角",
        PieceType::ROOK => "飛",
        PieceType::KING => "玉",
        PieceType::PRO_PAWN => "と",
        PieceType::PRO_LANCE => "杏",
        PieceType::PRO_KNIGHT => "圭",
        PieceType::PRO_SILVER => "全",
        PieceType::HORSE => "馬",
        PieceType::DRAGON => "龍",
        _ => return None,
    })
}

fn piece_from_text(value: char) -> Option<PieceType> {
    Some(match value {
        '歩' => PieceType::PAWN,
        '香' => PieceType::LANCE,
        '桂' => PieceType::KNIGHT,
        '銀' => PieceType::SILVER,
        '金' => PieceType::GOLD,
        '角' => PieceType::BISHOP,
        '飛' => PieceType::ROOK,
        '玉' => PieceType::KING,
        'と' => PieceType::PRO_PAWN,
        '杏' => PieceType::PRO_LANCE,
        '圭' => PieceType::PRO_KNIGHT,
        '全' => PieceType::PRO_SILVER,
        '馬' => PieceType::HORSE,
        '龍' => PieceType::DRAGON,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notation_round_trips_and_resolves() {
        board::init();
        let position = board::hirate_position();
        let mv = board::move_from_usi_expect(&position, "7g7f");
        let notation = Ki2Notation::from_move32(mv, &position).expect("legal move has notation");

        assert_eq!(notation.to_string(), "▲７六歩");
        assert_eq!(notation.to_string().parse(), Ok(notation));
        assert_eq!(notation.to_move32(&position), Ok(mv));
    }

    #[test]
    fn notation_is_a_compact_copy_value() {
        assert_eq!(core::mem::size_of::<Ki2Notation>(), 4);
    }

    #[test]
    fn same_destination_padding_is_canonicalized() {
        let padded: Ki2Notation = "△同　角".parse().expect("valid notation");
        let unpadded: Ki2Notation = "△同角".parse().expect("valid notation");

        assert_eq!(padded, unpadded);
        assert_eq!(padded.to_string(), "△同　角");
    }

    #[test]
    fn invalid_notations_are_rejected() {
        for text in ["７六歩", "▲7六歩", "▲７6歩", "▲７六王", "▲７六金打成", "▲７六と打", ""]
        {
            assert_eq!(text.parse::<Ki2Notation>(), Err(ParseKi2NotationError), "{text}");
        }
    }
}
