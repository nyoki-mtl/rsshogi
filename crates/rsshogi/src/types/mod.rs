//! 将棋の基本型定義
//!
//! このモジュールは将棋で使用する基本的な型を提供する。
//!
//! # 主要な型
//!
//! ## 盤面座標
//!
//! - [`Square`] - マス（0-80、9x9 盤面の各マスを表す）
//! - [`File`] - 筋（1-9、右から左へ）
//! - [`Rank`] - 段（一-九、上から下へ）
//!
//! ## 駒
//!
//! - [`PieceType`] - 駒種（歩、香、桂、銀、金、角、飛、玉、と、成香、...）
//! - [`Piece`] - 駒（駒種 + 手番の組み合わせ）
//! - [`Hand`] - 持ち駒（各駒種の枚数をビットパック）
//!
//! ## 指し手
//!
//! - [`Move32`] - 32bit 指し手（移動元、移動先、成り、駒打ち、取った駒など）
//! - [`Move`] - 16bit 指し手（コンパクト版、移動元、移動先、成り、駒打ちのみ）
//!
//! ## ビットボード
//!
//! - [`Bitboard`] - 81bit ビットボード（各マスの状態を1ビットで表現）
//!
//! ## ゲーム状態
//!
//! - [`Color`] - 手番（先手 `BLACK` / 後手 `WHITE`）
//! - [`GameResult`] - 対局結果（勝敗、千日手、入玉宣言など 16 種類）
//! - [`RepetitionState`] - 千日手状態
//! - [`EnteringKingRule`] - 入玉ルール
//!
//! # Examples
//!
//! ```
//! use rsshogi::types::{Color, Square, File, Rank, PieceType, Piece};
//!
//! // 手番
//! let black = Color::BLACK;
//! let white = Color::WHITE;
//! assert_eq!(black.flip(), white);
//!
//! // マス（7七）
//! let sq = Square::from_file_rank(File::FILE_7, Rank::RANK_7);
//! assert_eq!(sq.to_string(), "7g");
//!
//! // 駒（先手の歩）
//! let piece = Piece::from_parts(black, PieceType::PAWN);
//! assert_eq!(piece.to_string(), "P");
//! ```

pub mod color;
pub mod bitboard;
pub mod file;
pub mod hand;
pub mod game_result;
pub mod eval;
pub mod moves;
pub mod piece;
pub mod rank;
pub mod repetition_state;
pub mod entering_king_rule;
pub mod square;

// 主要な型を再エクスポート
pub use bitboard::Bitboard;
pub use color::Color;
pub use entering_king_rule::EnteringKingRule;
pub use eval::Eval;
pub use file::File;
pub use game_result::GameResult;
pub use hand::{HAND_BIT_MASK, HAND_BORROW_MASK, Hand, HandPiece};
pub(crate) use moves::MoveBase;
pub use moves::{
    AperyMove, AperyMove32, MOVE_END, MOVE_NONE, MOVE_NULL, MOVE_RESIGN, MOVE_WIN, Move, Move32,
    Move32Metadata,
};
pub use piece::{Piece, PieceType};
pub use rank::Rank;
pub use repetition_state::RepetitionState;
pub use square::{
    SQ_11, SQ_12, SQ_13, SQ_14, SQ_15, SQ_16, SQ_17, SQ_18, SQ_19, SQ_21, SQ_22, SQ_23, SQ_24,
    SQ_25, SQ_26, SQ_27, SQ_28, SQ_29, SQ_31, SQ_32, SQ_33, SQ_34, SQ_35, SQ_36, SQ_37, SQ_38,
    SQ_39, SQ_41, SQ_42, SQ_43, SQ_44, SQ_45, SQ_46, SQ_47, SQ_48, SQ_49, SQ_51, SQ_52, SQ_53,
    SQ_54, SQ_55, SQ_56, SQ_57, SQ_58, SQ_59, SQ_61, SQ_62, SQ_63, SQ_64, SQ_65, SQ_66, SQ_67,
    SQ_68, SQ_69, SQ_71, SQ_72, SQ_73, SQ_74, SQ_75, SQ_76, SQ_77, SQ_78, SQ_79, SQ_81, SQ_82,
    SQ_83, SQ_84, SQ_85, SQ_86, SQ_87, SQ_88, SQ_89, SQ_91, SQ_92, SQ_93, SQ_94, SQ_95, SQ_96,
    SQ_97, SQ_98, SQ_99, SQ_D, SQ_L, SQ_LD, SQ_LU, SQ_NONE, SQ_R, SQ_RD, SQ_RU, SQ_U, SQ_ZERO,
    Square, SquareTable,
};
