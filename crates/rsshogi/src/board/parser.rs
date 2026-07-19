//! SFEN/USIのパースとシリアライズ

use super::position::{BoardArray, Ply, Position};
use crate::types::{Color, File, Hand, HandPiece, Piece, PieceType, Rank, Square};
use std::convert::TryFrom;
use std::fmt;

/// SFEN解析エラー
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SfenError {
    MissingField(MissingFieldKind),
    InvalidPiece(char),
    InvalidSquare,
    InvalidHandCount { piece: PieceType, count: u8 },
    InvalidTurn(char),
    InvalidPly(std::num::ParseIntError),
    TrailingToken(String),
    InvalidMove(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MissingFieldKind {
    Placement,
    Turn,
    Hand,
    Ply,
}

impl fmt::Display for SfenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField(kind) => write!(f, "missing field: {kind:?}"),
            Self::InvalidPiece(piece) => write!(f, "invalid piece: '{piece}'"),
            Self::InvalidSquare => write!(f, "invalid square"),
            Self::InvalidHandCount { piece, count } => {
                write!(f, "invalid hand count for {piece:?}: {count}")
            }
            Self::InvalidTurn(turn) => write!(f, "invalid turn: '{turn}'"),
            Self::InvalidPly(err) => write!(f, "invalid ply: {err}"),
            Self::TrailingToken(token) => write!(f, "trailing token: '{token}'"),
            Self::InvalidMove(mv) => write!(f, "invalid move in sequence: '{mv}'"),
        }
    }
}

impl std::error::Error for SfenError {}

/// 盤面の raw state を保持する構造化 DTO。
///
/// `Position` の mutable state と、外部処理へ渡す plain な board-state の境界として使う。
/// この型は次の情報をそのまま保持する。
///
/// - 81 マスの駒配置
/// - 先手 / 後手の持ち駒
/// - 手番
/// - 手数
///
/// [`generate_position_state()`] で `Position` から export し、
/// [`crate::board::position_from_position_state()`] または
/// [`crate::board::Position::set_position_state()`] で SFEN 文字列を経由せず import できる。
///
/// この型そのものは局面のルール妥当性を保証しない。
/// import 後に妥当性も確認したい場合は、`validation` feature を有効にして
/// `Position::validate()` / `Position::validate_all()` を組み合わせて使う。
///
/// # Examples
///
/// ```
/// use rsshogi::board::{self, BoardArray, PositionState};
/// use rsshogi::types::{Color, File, Hand, Piece, Rank, Square};
///
/// board::init();
///
/// let mut board_array = BoardArray::empty();
/// board_array.set(
///     Square::from_file_rank(File::FILE_5, Rank::RANK_1),
///     Piece::B_KING,
/// );
/// board_array.set(
///     Square::from_file_rank(File::FILE_5, Rank::RANK_9),
///     Piece::W_KING,
/// );
/// board_array.set(
///     Square::from_file_rank(File::FILE_7, Rank::RANK_7),
///     Piece::B_PAWN,
/// );
///
/// let state = PositionState {
///     board: board_array,
///     hands: [Hand::ZERO; Color::COUNT],
///     side_to_move: Color::BLACK,
///     ply: 1,
/// };
///
/// let pos = board::position_from_position_state(&state);
/// assert_eq!(board::generate_position_state(&pos), state);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PositionState {
    pub board: BoardArray,
    pub hands: [Hand; Color::COUNT],
    pub side_to_move: Color,
    pub ply: Ply,
}

/// SFEN文字列を解析
pub fn parse_sfen(sfen: &str) -> Result<PositionState, SfenError> {
    let tokens: Vec<&str> = sfen.split_whitespace().collect();
    if tokens.is_empty() {
        return Err(SfenError::MissingField(MissingFieldKind::Placement));
    }

    if tokens.len() < 3 {
        return Err(SfenError::MissingField(match tokens.len() {
            0 => MissingFieldKind::Placement,
            1 => MissingFieldKind::Turn,
            _ => MissingFieldKind::Hand,
        }));
    }

    let board = parse_board(tokens[0])?;
    let side_to_move = parse_turn(tokens[1])?;
    let hands = parse_hands(tokens[2])?;
    let ply = if tokens.len() >= 4 { parse_ply(tokens[3])? } else { 0 };

    Ok(PositionState { board, hands, side_to_move, ply })
}

/// [`PositionState`] から SFEN 文字列を生成する。
///
/// `Position` を経由せずに raw-state として保持した `PositionState` から
/// 直接 SFEN 文字列を得る。board-state を外部で編集して保持しつつ、
/// 必要なタイミングでのみ SFEN へシリアライズしたい用途を想定する。
/// `include_ply` は手数を出力するかどうかを指定する。
#[must_use]
pub fn generate_sfen_from_position_state(state: &PositionState, include_ply: bool) -> String {
    let mut result = String::new();

    // 1. 盤面
    for rank_idx in 0..9 {
        let rank = Rank::new(i8::try_from(rank_idx).expect("rank index within range"));
        let mut empty_count = 0;
        for file_idx in (0..9).rev() {
            let file = File::new(i8::try_from(file_idx).expect("file index within range"));
            let sq = Square::from_file_rank(file, rank);
            let piece = state.board.get(sq);

            if piece == Piece::NONE {
                empty_count += 1;
            } else {
                if empty_count > 0 {
                    result.push_str(&empty_count.to_string());
                    empty_count = 0;
                }
                let piece_type = piece.piece_type();
                if piece_type.raw() >= 9 && piece_type.raw() <= 14 {
                    result.push('+');
                }
                result.push(piece_to_sfen_char(piece));
            }
        }

        if empty_count > 0 {
            result.push_str(&empty_count.to_string());
        }

        if rank_idx < 8 {
            result.push('/');
        }
    }

    // 2. 手番
    result.push(' ');
    result.push(if state.side_to_move == Color::BLACK { 'b' } else { 'w' });

    // 3. 持ち駒
    result.push(' ');
    let hands_str = generate_hands_from_data(state.hands);
    if hands_str.is_empty() {
        result.push('-');
    } else {
        result.push_str(&hands_str);
    }

    // 4. 手数
    if include_ply {
        result.push(' ');
        result.push_str(&state.ply.to_string());
    }

    result
}

/// `Position` から [`PositionState`] を生成する
///
/// 局面の盤面、持ち駒、手番、手数を plain な raw-state として取り出す。
/// `Position` をそのまま共有したくない場合の export 境界として使える。
#[must_use]
pub fn generate_position_state(pos: &Position) -> PositionState {
    PositionState {
        board: pos.board_array(),
        hands: pos.hands_array(),
        side_to_move: pos.turn(),
        ply: pos.game_ply(),
    }
}

/// SFEN文字列を生成
#[must_use]
pub fn generate_sfen(pos: &Position) -> String {
    generate_sfen_with_ply(pos, Some(i32::from(pos.game_ply())))
}

/// SFEN文字列を生成（手数の出力を制御）
#[must_use]
pub fn generate_sfen_with_ply(pos: &Position, ply: Option<i32>) -> String {
    let mut result = String::new();

    // 1. 盤面
    for rank_idx in 0..9 {
        let rank = Rank::new(i8::try_from(rank_idx).expect("rank index within range"));
        let mut empty_count = 0;
        for file_idx in (0..9).rev() {
            let file = File::new(i8::try_from(file_idx).expect("file index within range"));
            let sq = Square::from_file_rank(file, rank);
            let piece = pos.piece_on(sq);

            if piece == Piece::NONE {
                empty_count += 1;
            } else {
                if empty_count > 0 {
                    result.push_str(&empty_count.to_string());
                    empty_count = 0;
                }
                // 成り駒の場合は+を追加
                let piece_type = piece.piece_type();
                if piece_type.raw() >= 9 && piece_type.raw() <= 14 {
                    result.push('+');
                }
                result.push(piece_to_sfen_char(piece));
            }
        }

        if empty_count > 0 {
            result.push_str(&empty_count.to_string());
        }

        if rank_idx < 8 {
            result.push('/');
        }
    }

    // 2. 手番
    result.push(' ');
    result.push(if pos.turn() == Color::BLACK { 'b' } else { 'w' });

    // 3. 持ち駒
    result.push(' ');
    let hands_str = generate_hands(pos);
    if hands_str.is_empty() {
        result.push('-');
    } else {
        result.push_str(&hands_str);
    }

    // 4. 手数
    if let Some(ply) = ply {
        result.push(' ');
        result.push_str(&ply.to_string());
    }

    result
}

// --- ヘルパー関数 ---

fn parse_board(s: &str) -> Result<BoardArray, SfenError> {
    let mut board = BoardArray::empty();
    let mut rank: i8 = 0;
    let mut file: i8 = 8;
    let mut chars = s.chars();

    while let Some(ch) = chars.next() {
        match ch {
            '/' => {
                if file != -1 {
                    return Err(SfenError::InvalidSquare);
                }
                rank += 1;
                file = 8;
            }
            '1'..='9' => {
                let skip = i8::try_from(ch.to_digit(10).expect("digit")).expect("skip fits in i8");
                file -= skip;
                if file < -1 {
                    return Err(SfenError::InvalidSquare);
                }
            }
            '+' => {
                // 成り駒: 次の文字を読む
                if file < 0 || rank >= 9 {
                    return Err(SfenError::InvalidSquare);
                }

                let next_ch = chars.next().ok_or(SfenError::InvalidPiece('+'))?;
                let piece = sfen_char_to_promoted_piece(next_ch)?;
                let sq = Square::from_file_rank(File::new(file), Rank::new(rank));
                board.set(sq, piece);
                file -= 1;
            }
            _ => {
                if file < 0 || rank >= 9 {
                    return Err(SfenError::InvalidSquare);
                }
                let piece = sfen_char_to_piece(ch)?;
                let sq = Square::from_file_rank(File::new(file), Rank::new(rank));
                board.set(sq, piece);
                file -= 1;
            }
        }
    }

    if rank != 8 || file != -1 {
        return Err(SfenError::InvalidSquare);
    }

    Ok(board)
}

fn parse_turn(s: &str) -> Result<Color, SfenError> {
    match s {
        "b" => Ok(Color::BLACK),
        "w" => Ok(Color::WHITE),
        _ => Err(SfenError::InvalidTurn(s.chars().next().unwrap_or(' '))),
    }
}

fn parse_hands(s: &str) -> Result<[Hand; Color::COUNT], SfenError> {
    let mut hands = [Hand::ZERO; Color::COUNT];

    if s == "-" {
        return Ok(hands);
    }

    let mut count = 1;
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch.is_ascii_digit() {
            count = 0;
            count = count * 10 + ch.to_digit(10).expect("digit");

            // 複数桁の数字を読む
            while let Some(&next_ch) = chars.peek() {
                if next_ch.is_ascii_digit() {
                    chars.next();
                    count = count * 10 + next_ch.to_digit(10).expect("digit");
                } else {
                    break;
                }
            }
        } else {
            let color = if ch.is_ascii_uppercase() { Color::BLACK } else { Color::WHITE };
            let piece = sfen_char_to_hand_piece(ch)?;

            for _ in 0..count {
                let idx = color.to_index();
                hands[idx] = Hand::add_one(hands[idx], piece);
            }

            count = 1;
        }
    }

    Ok(hands)
}

fn parse_ply(s: &str) -> Result<Ply, SfenError> {
    s.parse::<Ply>().map_err(SfenError::InvalidPly)
}

const fn sfen_char_to_piece(ch: char) -> Result<Piece, SfenError> {
    let piece = match ch.to_ascii_lowercase() {
        'p' => Piece::from_parts(Color::BLACK, PieceType::PAWN),
        'l' => Piece::from_parts(Color::BLACK, PieceType::LANCE),
        'n' => Piece::from_parts(Color::BLACK, PieceType::KNIGHT),
        's' => Piece::from_parts(Color::BLACK, PieceType::SILVER),
        'g' => Piece::from_parts(Color::BLACK, PieceType::GOLD),
        'b' => Piece::from_parts(Color::BLACK, PieceType::BISHOP),
        'r' => Piece::from_parts(Color::BLACK, PieceType::ROOK),
        'k' => Piece::from_parts(Color::BLACK, PieceType::KING),
        '+' => {
            // 成り駒は次の文字を見る必要がある
            return Err(SfenError::InvalidPiece(ch));
        }
        _ => return Err(SfenError::InvalidPiece(ch)),
    };

    // 小文字なら後手
    let piece = if ch.is_ascii_lowercase() {
        Piece::from_parts(Color::WHITE, piece.piece_type())
    } else {
        piece
    };

    Ok(piece)
}

/// SFEN文字から成り駒への変換（'+'の次の文字を受け取る）
const fn sfen_char_to_promoted_piece(ch: char) -> Result<Piece, SfenError> {
    let piece_type = match ch.to_ascii_lowercase() {
        'p' => PieceType::PRO_PAWN,
        'l' => PieceType::PRO_LANCE,
        'n' => PieceType::PRO_KNIGHT,
        's' => PieceType::PRO_SILVER,
        'b' => PieceType::HORSE,
        'r' => PieceType::DRAGON,
        _ => return Err(SfenError::InvalidPiece(ch)),
    };

    // 大文字なら先手、小文字なら後手
    let color = if ch.is_ascii_uppercase() { Color::BLACK } else { Color::WHITE };

    Ok(Piece::from_parts(color, piece_type))
}

fn sfen_char_to_hand_piece(ch: char) -> Result<HandPiece, SfenError> {
    match ch.to_ascii_lowercase() {
        'p' => Ok(HandPiece::from_piece_type(PieceType::PAWN).unwrap()),
        'l' => Ok(HandPiece::from_piece_type(PieceType::LANCE).unwrap()),
        'n' => Ok(HandPiece::from_piece_type(PieceType::KNIGHT).unwrap()),
        's' => Ok(HandPiece::from_piece_type(PieceType::SILVER).unwrap()),
        'g' => Ok(HandPiece::from_piece_type(PieceType::GOLD).unwrap()),
        'b' => Ok(HandPiece::from_piece_type(PieceType::BISHOP).unwrap()),
        'r' => Ok(HandPiece::from_piece_type(PieceType::ROOK).unwrap()),
        _ => Err(SfenError::InvalidPiece(ch)),
    }
}

fn piece_to_sfen_char(piece: Piece) -> char {
    if piece == Piece::NONE {
        return '.';
    }

    let piece_type = piece.piece_type();

    // 成り駒の場合は+付きで返す必要があるため、特別処理
    let ch = match piece_type {
        pt if pt == PieceType::PAWN => 'P',
        pt if pt == PieceType::LANCE => 'L',
        pt if pt == PieceType::KNIGHT => 'N',
        pt if pt == PieceType::SILVER => 'S',
        pt if pt == PieceType::GOLD => 'G',
        pt if pt == PieceType::BISHOP => 'B',
        pt if pt == PieceType::ROOK => 'R',
        pt if pt == PieceType::KING => 'K',
        pt if pt == PieceType::PRO_PAWN => 'P', // 成り駒は元の駒種を返す
        pt if pt == PieceType::PRO_LANCE => 'L',
        pt if pt == PieceType::PRO_KNIGHT => 'N',
        pt if pt == PieceType::PRO_SILVER => 'S',
        pt if pt == PieceType::HORSE => 'B',
        pt if pt == PieceType::DRAGON => 'R',
        _ => '?',
    };

    if piece.color() == Color::WHITE { ch.to_ascii_lowercase() } else { ch }
}

fn generate_hands(pos: &Position) -> String {
    generate_hands_from_data([pos.hand(Color::BLACK), pos.hand(Color::WHITE)])
}

fn generate_hands_from_data(hands: [Hand; Color::COUNT]) -> String {
    let mut result = String::new();

    for (idx, color) in [Color::BLACK, Color::WHITE].iter().enumerate() {
        let hand = hands[idx];

        // 飛、角、金、銀、桂、香、歩の順
        let order = [
            HandPiece::from_piece_type(PieceType::ROOK).unwrap(),
            HandPiece::from_piece_type(PieceType::BISHOP).unwrap(),
            HandPiece::from_piece_type(PieceType::GOLD).unwrap(),
            HandPiece::from_piece_type(PieceType::SILVER).unwrap(),
            HandPiece::from_piece_type(PieceType::KNIGHT).unwrap(),
            HandPiece::from_piece_type(PieceType::LANCE).unwrap(),
            HandPiece::from_piece_type(PieceType::PAWN).unwrap(),
        ];

        for hp in order {
            let count = Hand::count_of(hand, hp);
            if count > 0 {
                if count > 1 {
                    result.push_str(&count.to_string());
                }

                let ch = match hp.to_piece_type() {
                    PieceType::PAWN => 'P',
                    PieceType::LANCE => 'L',
                    PieceType::KNIGHT => 'N',
                    PieceType::SILVER => 'S',
                    PieceType::GOLD => 'G',
                    PieceType::BISHOP => 'B',
                    PieceType::ROOK => 'R',
                    _ => '?',
                };

                result.push(if *color == Color::BLACK { ch } else { ch.to_ascii_lowercase() });
            }
        }
    }

    result
}
