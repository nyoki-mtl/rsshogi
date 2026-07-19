//! 盤面表現と状態管理
//!
//! このモジュールは将棋の盤面を表現し、指し手の適用と取り消し、
//! 合法手生成などの機能を提供する。
//!
//! # 主要な型
//!
//! - [`Position`] - 盤面状態を管理する中心的な構造体
//! - [`MoveList`] - 指し手のリスト
//!
//! # 初期化
//!
//! 利きテーブルなどの事前計算は遅延初期化される。
//! [`init()`] は必須ではないが、起動時に一度呼ぶことで初回アクセス時の
//! コストを先に払う optional warm-up として利用できる。
//!
//! ```
//! use rsshogi::board;
//!
//! // プログラム起動時に1回呼び出す
//! board::init();
//! ```
//!
//! # 局面の構築
//!
//! ```
//! use rsshogi::board::{self, Position};
//!
//! board::init();
//!
//! // 平手初期局面
//! let pos = board::hirate_position();
//!
//! // SFEN から構築
//! let pos = board::position_from_sfen(
//!     "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"
//! ).unwrap();
//! ```
//!
//! # 指し手の適用と取り消し
//!
//! ```
//! use rsshogi::board::{self, Position};
//! use rsshogi::types::Move;
//!
//! board::init();
//! let mut pos = board::hirate_position();
//!
//! // 指し手を適用
//! let mv16 = Move::from_usi("7g7f").unwrap();
//! let m = pos.move32_from_move(mv16);
//! pos.apply_move32(m);
//!
//! // 指し手を取り消し
//! pos.undo_move32(m);
//! ```
//!
//! # 合法手生成
//!
//! 合法手生成は [`movegen`] サブモジュールで提供される。
//!
//! ```
//! use rsshogi::board::{self, MoveList};
//! use rsshogi::movegen::generate_legal_all;
//!
//! board::init();
//! let pos = board::hirate_position();
//!
//! let mut moves = MoveList::new();
//! generate_legal_all(&pos, &mut moves);
//!
//! println!("合法手数: {}", moves.len());
//! ```
//!
//! `Move32`（駒情報付き 32bit 指し手）のリストを直接生成するには
//! [`generate_legal_all_move32`] を使用する。
//!
//! ```
//! use rsshogi::board::{self, Move32List};
//! use rsshogi::movegen::generate_legal_all_move32;
//!
//! board::init();
//! let pos = board::hirate_position();
//!
//! let mut moves = Move32List::new();
//! generate_legal_all_move32(&pos, &mut moves);
//!
//! println!("合法手数: {}", moves.len());
//! ```
//!
//! # サブモジュール
//!
//! - [`movegen`] - 合法手生成
//! - [`attack_tables`] - 利きテーブル
//! - [`perft`] - Perft テスト
//! - [`zobrist`] - Zobrist ハッシュ

pub mod attack_tables;
mod bitboard256;
mod bitboard_set;

mod move_list;
pub mod movegen;
#[cfg(feature = "initial-positions")]
pub mod initial_positions;
pub mod perft;
mod parser;
pub use parser::{
    PositionState, SfenError, generate_position_state, generate_sfen,
    generate_sfen_from_position_state, generate_sfen_with_ply, parse_sfen,
};
mod lookup;
pub mod zobrist;
pub mod position;
pub(crate) mod state_info;
pub(crate) mod mate_constant;
#[cfg(test)]
pub(crate) mod test_support;

pub use crate::types::Bitboard;
pub use attack_tables::{
    GOLD_ATTACKS, KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS, SILVER_ATTACKS, check_candidate_bb,
};
pub use bitboard_set::BitboardSet;
pub use bitboard256::Bitboard256;
#[cfg(feature = "initial-positions")]
pub use initial_positions::InitialPosition;
pub use move_list::{Move32List, MoveList};
pub use movegen::{
    CapturePlusPro, CapturePlusProAll, Captures, CapturesAll, Checks, ChecksAll, Evasions,
    EvasionsAll, Legal, LegalAll, Move32Sink, MoveGenType, NonEvasions, NonEvasionsAll,
    QuietChecks, QuietChecksAll, Quiets, QuietsAll, QuietsProMinus, QuietsProMinusAll, Recaptures,
    RecapturesAll, generate_legal_all_move32, generate_legal_all_move32_into,
    generate_legal_evasions_all_move32, generate_legal_evasions_all_move32_into,
    generate_legal_evasions_move32, generate_legal_evasions_move32_into, generate_moves,
    generate_moves_move32, generate_moves_move32_into, generate_moves_to_move32,
    generate_moves_to_move32_into, generate_quiet_checks, generate_recaptures,
    generate_recaptures_all, generate_recaptures_all_move32, generate_recaptures_move32,
};
pub use position::{
    BoardArray, CapturedPieceDelta, DeclarationDetail, DeclarationEvaluation, MoveApplyFacts,
    MoveDelta32, MoveStackSiteFacts, PackedPiece, PartialKeys, Ply, Position, StateCacheView,
    StateHistory, StateHistoryEntry, ValidationIssue, ValidationReport,
};
#[cfg(feature = "position-serialization")]
pub use position::{
    HuffmanCodedPos, HuffmanCodedPosError, PackedSfen, PackedSfenError, PackedSfenSink,
};
// 局面の状態キャッシュ値の型のみ公開する。内部の状態保存方式（`StateInfo` /
// `StateHot` / `StateStack`）は `pub(crate)` のまま隠し、`Position` の accessor と
// `StateHistory` view を通してのみ読めるようにする。
pub use state_info::CheckSquares;
#[cfg(test)]
mod tests;

/// 平手初期局面の SFEN。
///
/// `InitialPosition::Standard.to_sfen()` と同一の値を core 定数として供給する。
/// これにより `set_hirate()` / [`hirate_position()`] は `initial-positions` feature
/// （駒落ち含む `InitialPosition` enum）に依存せず core 単独でコンパイルできる。
pub const STARTPOS_SFEN: &str = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";

/// 盤面関連の事前計算を事前に行う。
///
/// 通常の API 利用に必須ではない。初回の合法手生成や利き計算で
/// 遅延初期化されるテーブルを、起動時などに明示的に warm-up したい場合に
/// 呼び出す。
pub fn init() {
    Bitboard::init_tables();
}

/// SFEN 文字列から局面を構築する（ヘルパー）。
pub fn position_from_sfen(sfen: &str) -> Result<Position, parser::SfenError> {
    let mut pos = Position::empty();
    pos.set_sfen(sfen)?;
    Ok(pos)
}

/// [`PositionState`] から局面を構築する（ヘルパー）。
///
/// エディタ等で構築した `PositionState` から `Position` を生成する。
/// SFEN 文字列を経由しない raw-state import に使用できる。
///
/// この関数は盤面キャッシュやハッシュを再構築するが、
/// 局面のルール妥当性までは検証しない unchecked import である。
/// checked workflow が必要な場合は `validation` feature を有効にして import 後に
/// `validate()` / `validate_all()` を呼ぶ。
///
/// # Examples
///
/// ```
/// use rsshogi::board::{self, BoardArray, PositionState};
/// use rsshogi::types::{Color, Hand, Piece};
///
/// board::init();
///
/// let mut board_array = BoardArray::empty();
/// board_array.set(rsshogi::types::SQ_59, Piece::B_KING);
/// board_array.set(rsshogi::types::SQ_51, Piece::W_KING);
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
#[must_use]
pub fn position_from_position_state(state: &PositionState) -> Position {
    let mut pos = Position::empty();
    pos.set_position_state(state);
    pos
}

/// 平手初期局面を構築する（ヘルパー）。
#[must_use]
pub fn hirate_position() -> Position {
    let mut pos = Position::empty();
    pos.set_hirate();
    pos
}

/// USI 文字列から、現局面 `pos` に対する [`Move32`](crate::types::Move32) を生成する。
///
/// 盤面に依存する駒情報（移動元の駒種と手番）を補完するため `Position` への参照を取る。
///
/// - 解析と駒情報の付与に成功した場合は `Some(Move32)` を返す。
/// - USI 文字列が不正、または drop 駒種が解決できない場合は `None` を返す。
///
/// **合法手判定は行わない。** 文字列の構文解析と盤面からの駒情報補完のみを行う。
/// 返された `Move32` が現局面で合法かどうかは、別途 `Position::is_legal_move32` 等で
/// 確認すること。
#[must_use]
pub fn move_from_usi(pos: &Position, usi: &str) -> Option<crate::types::Move32> {
    use crate::types::{Move, Move32};
    let mv16 = Move::from_usi(usi)?;

    if mv16.is_drop() {
        let piece_type = mv16.dropped_piece()?;
        Some(Move32::drop(piece_type, mv16.to_sq(), pos.turn()))
    } else {
        let from = mv16.from_sq();
        let to = mv16.to_sq();
        let piece = pos.piece_on(from);

        if mv16.is_promotion() {
            Some(Move32::promotion(from, to, piece))
        } else {
            Some(Move32::normal(from, to, piece))
        }
    }
}

/// [`move_from_usi`] の `expect` 版。USI 文字列が不正な場合は panic する。
///
/// # Panics
/// `usi` が現局面に対して有効な指し手として解釈できない場合に panic する。
#[must_use]
pub fn move_from_usi_expect(pos: &Position, usi: &str) -> crate::types::Move32 {
    move_from_usi(pos, usi).unwrap_or_else(|| panic!("invalid USI move: {usi}"))
}
