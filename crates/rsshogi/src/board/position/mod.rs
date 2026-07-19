//! 将棋盤面の管理
//!
//! このモジュールは [`Position`] 構造体を中心に、将棋の盤面状態を管理する。
//!
//! # Position
//!
//! `Position` は以下の情報を保持する：
//!
//! - 盤面の駒配置（81マス）
//! - 持ち駒（先手と後手）
//! - 手番
//! - 手数
//! - Zobrist ハッシュキー
//! - 王手情報のキャッシュ
//!
//! # 局面の構築
//!
//! ```
//! use rsshogi::board::{self, Position, position_from_sfen, hirate_position};
//!
//! board::init();
//!
//! // 平手初期局面
//! let pos = hirate_position();
//!
//! // SFEN から構築
//! let pos = position_from_sfen(
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
//! let mut pos = rsshogi::board::hirate_position();
//!
//! // 指し手を適用
//! let mv16 = Move::from_usi("7g7f").unwrap();
//! let m = pos.move32_from_move(mv16);
//! pos.apply_move32(m);
//!
//! // 局面が更新される
//! assert_eq!(pos.game_ply(), 2);
//!
//! // 指し手を取り消し
//! pos.undo_move32(m);
//! assert_eq!(pos.game_ply(), 1);
//! ```
//!
//! # 局面の状態取得
//!
//! ```
//! use rsshogi::board::{self, Position};
//! use rsshogi::types::{Color, Square, PieceType};
//!
//! board::init();
//! let pos = rsshogi::board::hirate_position();
//!
//! // 手番
//! assert_eq!(pos.turn(), Color::BLACK);
//!
//! // 特定のマスの駒
//! let sq = Square::from_file_rank(
//!     rsshogi::types::File::FILE_5,
//!     rsshogi::types::Rank::RANK_1
//! );
//! let piece = pos.piece_on(sq);
//! assert_eq!(piece.piece_type(), PieceType::KING);
//!
//! // 王手されているか
//! assert!(!pos.is_in_check());
//! ```
//!
//! # Zobrist ハッシュ
//!
//! 局面の同一性判定やトランスポジションテーブルで使用する。
//!
//! ```
//! use rsshogi::board::{self, Position};
//!
//! board::init();
//! let pos1 = rsshogi::board::hirate_position();
//! let pos2 = rsshogi::board::hirate_position();
//!
//! // 同じ局面は同じハッシュキー
//! assert_eq!(pos1.key(), pos2.key());
//! ```
//!
//! # PackedSfen（`position-serialization` feature）
//!
//! 局面をコンパクトなバイナリ形式で表現する。
//! cshogi の `PackedSfen` と互換性がある。`position-serialization` feature が必要である。
//!
//! ```
//! # #[cfg(feature = "position-serialization")] {
//! use rsshogi::board::{self, Position, PackedSfen};
//!
//! board::init();
//! let pos = rsshogi::board::hirate_position();
//!
//! // PackedSfen に変換
//! let psfen = pos.to_packed_sfen();
//!
//! // PackedSfen から復元
//! let mut pos2 = Position::empty();
//! pos2.set_packed_sfen(&psfen, false, pos.game_ply()).unwrap();
//! assert_eq!(pos.to_sfen(None), pos2.to_sfen(None));
//! # }
//! ```

mod accessors;
mod attacks;
#[cfg(feature = "position-serialization")]
mod bit_io;
mod cache;
mod constructors;
#[cfg(feature = "position-serialization")]
mod huffman_coded_pos;
mod legality;
mod maintenance;
#[cfg(feature = "position-serialization")]
mod packed_sfen;
mod parser;
mod rules;
#[cfg(feature = "svg")]
mod svg;

mod types;
mod updates;
mod hash;
#[cfg(feature = "validation")]
mod validation;

use super::bitboard_set::BitboardSet;
use crate::board::state_info::StateIndex;
use crate::board::zobrist::ZobristKey;
use crate::types::{Color, EnteringKingRule, File, Hand, Piece, Rank, Square};
use std::fmt;

use crate::board::state_info::StateStack;
#[cfg(test)]
pub(super) use crate::types::Move32;

pub use super::parser::{MissingFieldKind, SfenError};
pub use crate::board::state_info::PartialKeys;
pub use cache::StateCacheView;
#[cfg(feature = "position-serialization")]
pub use huffman_coded_pos::{HuffmanCodedPos, HuffmanCodedPosError};
#[cfg(feature = "position-serialization")]
pub use packed_sfen::{PackedSfen, PackedSfenError, PackedSfenSink};

pub use types::{
    BoardArray, CapturedPieceDelta, DeclarationDetail, DeclarationEvaluation, MoveApplyFacts,
    MoveDelta32, MoveError, MoveStackSiteFacts, PackedPiece, Ply, ValidationError, ValidationIssue,
    ValidationReport,
};

// ANCHOR: position_struct
/// 将棋の盤面状態を管理する構造体
///
/// キャッシュ効率のため、頻繁にアクセスされるデータを先頭に配置
#[repr(C, align(64))]
#[derive(Clone)]
pub struct Position {
    /// `Piece` を81要素並べた盤面
    pub(super) board: BoardArray,

    /// 駒種別と先後別のビットボード集合
    pub(super) bitboards: BitboardSet,

    /// 持ち駒カウント配列
    pub(super) hands: [Hand; Color::COUNT],

    /// 現局面のZobristハッシュ
    pub(super) zobrist: ZobristKey,

    /// 盤面ハッシュ（手番込み）
    pub(super) board_key: ZobristKey,

    /// 持ち駒ハッシュ
    pub(super) hand_key: ZobristKey,

    /// 手番
    pub(super) side_to_move: Color,

    /// 入玉ルール設定
    pub(super) entering_king_rule: EnteringKingRule,

    /// 入玉判定に必要な駒点
    pub(super) entering_king_point: [i32; Color::COUNT],

    /// USIで定義される手数（1から始まる）
    pub(super) ply: Ply,

    /// 現局面に対応するStateInfoのインデックス
    pub(super) st_index: StateIndex,

    /// 局面履歴。
    pub(super) state_stack: StateStack,

    /// 玉の位置（色別）
    pub(super) king_square: [Square; Color::COUNT],
}
// ANCHOR_END: position_struct

impl Default for Position {
    fn default() -> Self {
        Self::empty()
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 盤面を表示
        writeln!(f, "  9 8 7 6 5 4 3 2 1")?;
        writeln!(f, "+-------------------+")?;

        for rank_idx in 0..9 {
            let rank_value = i8::try_from(rank_idx).expect("rank index within range");
            let rank = Rank::new(rank_value);
            write!(f, "|")?;
            for file_idx in (0..9).rev() {
                let file_value = i8::try_from(file_idx).expect("file index within range");
                let sq = Square::from_file_rank(File::new(file_value), rank);
                let piece = self.piece_on(sq);

                if piece == Piece::NONE {
                    write!(f, " .")?;
                } else {
                    // 簡易表示（実際はもっと詳細な表示が必要）
                    write!(f, " *")?;
                }
            }
            let rank_char =
                char::from(b'a' + u8::try_from(rank_idx).expect("rank index fits in u8"));
            writeln!(f, "|{rank_char}")?;
        }

        writeln!(f, "+-------------------+")?;
        writeln!(f, "Side to move: {:?}", self.turn())?;
        writeln!(f, "Ply: {}", self.ply)?;

        Ok(())
    }
}

/// 局面の状態履歴への read-only view（[`Position::state_history`] が返す）。
///
/// 内部表現（`StateStack` / `StateInfo`）を隠し、過去 state を走査するための
/// 安定した accessor だけを提供する。state 保存方式を変えても public API は不変。
#[derive(Clone, Copy)]
pub struct StateHistory<'a> {
    stack: &'a StateStack,
    current: StateIndex,
}

impl<'a> StateHistory<'a> {
    /// 履歴に含まれる state 数（root から現局面まで、常に 1 以上）。
    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize {
        self.current + 1
    }

    /// 常に `false`（root state を必ず含む）。
    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        false
    }

    /// root(0) から現局面までの各 state entry を順に返す。
    pub fn iter(&self) -> impl Iterator<Item = StateHistoryEntry<'a>> {
        let stack = self.stack;
        let current = self.current;
        (0..=current).map(move |idx| StateHistoryEntry { state: &stack[idx] })
    }
}

/// 状態履歴中の単一 state への read-only view（[`StateHistory::iter`] が返す）。
#[derive(Clone, Copy)]
pub struct StateHistoryEntry<'a> {
    state: &'a crate::board::state_info::StateInfo,
}

impl StateHistoryEntry<'_> {
    /// 盤面 Zobrist（board_key）。
    #[must_use]
    #[inline]
    pub fn board_key(&self) -> ZobristKey {
        self.state.board_key()
    }

    /// その局面の手番側の持ち駒。
    #[must_use]
    #[inline]
    pub fn hand(&self) -> Hand {
        self.state.hand()
    }

    /// null move からの手数。
    #[must_use]
    #[inline]
    pub fn plies_from_null(&self) -> Ply {
        self.state.plies_from_null()
    }

    /// 繰り返し回数（- 1）。
    #[must_use]
    #[inline]
    pub fn repetition_times(&self) -> i32 {
        self.state.repetition_times()
    }

    /// 千日手の状態（キャッシュ値）。
    #[must_use]
    #[inline]
    pub fn repetition_type(&self) -> crate::types::RepetitionState {
        self.state.repetition_type()
    }
}

impl Position {
    /// `StateStack` への参照を取得
    #[must_use]
    #[inline]
    pub(crate) fn state_stack(&self) -> &StateStack {
        &self.state_stack
    }

    /// 現局面の状態履歴（root から現局面までの過去 state を含む）への read-only view。
    ///
    /// 内部の `StateStack` / `StateInfo` は露出せず、[`StateHistoryEntry`] の accessor
    /// （`board_key` / `hand` / `plies_from_null` / `repetition_times` /
    /// `repetition_type`）だけを通して読める。千日手判定などで過去局面の状態を走査
    /// したい外部 consumer 向け。
    #[must_use]
    #[inline]
    pub fn state_history(&self) -> StateHistory<'_> {
        StateHistory { stack: &self.state_stack, current: self.st_index }
    }

    /// USI 文字列から現局面に対する [`Move32`](crate::types::Move32) を生成する。
    ///
    /// [`crate::board::move_from_usi`] の `Position` メソッド版。**合法手判定は行わない**
    /// （構文解析と盤面からの駒情報補完のみ）。
    #[must_use]
    #[inline]
    pub fn move32_from_usi(&self, usi: &str) -> Option<crate::types::Move32> {
        crate::board::move_from_usi(self, usi)
    }

    /// 現在局面に対応する `StateInfo` への参照を取得
    #[cfg(test)]
    #[must_use]
    #[inline]
    pub(crate) fn current_state(&self) -> &crate::board::state_info::StateInfo {
        &self.state_stack[self.st_index]
    }

    /// 現在局面に対応する hot state への参照を取得
    #[must_use]
    #[inline]
    pub(crate) fn current_hot(&self) -> &crate::board::state_info::StateHot {
        self.state_stack.hot(self.st_index)
    }

    /// 現在局面に対応する cold payload への参照を取得
    #[must_use]
    #[inline]
    pub(crate) fn current_cold(&self) -> &crate::board::state_info::StateColdPayload {
        self.state_stack.cold(self.st_index)
    }

    /// 現在の state stack 深さを返す
    #[must_use]
    pub const fn state_stack_depth(&self) -> usize {
        self.st_index
    }

    /// recursive search で使用する追加 state slot を事前確保する。
    ///
    /// 現在の head から `additional_states` 回の連続 push が `StateStack` の
    /// chunk を増設しないことを保証する。予約 token は管理しないため、undo 後は
    /// 同じ物理 slot を再利用できる。
    ///
    /// # Errors
    ///
    /// 現在の head と `additional_states` の加算が overflow する場合は
    /// [`MoveError::StateCapacityExceeded`] を返す。
    pub fn prepare_search_state_capacity(
        &mut self,
        additional_states: usize,
    ) -> Result<(), MoveError> {
        if self.state_stack.prepare_additional(additional_states) {
            Ok(())
        } else {
            Err(MoveError::StateCapacityExceeded)
        }
    }

    /// 次の search state を allocation なしで積めるかを返す。
    ///
    /// strict reservation の残量ではなく、現在の head の次に物理 slot があるかを表す。
    #[must_use]
    #[inline(always)]
    pub fn has_prepared_search_state(&self) -> bool {
        self.state_stack.has_prepared_next()
    }

    /// `StateStack` への可変参照を取得
    pub(crate) fn state_stack_mut(&mut self) -> &mut StateStack {
        &mut self.state_stack
    }
}

#[cfg(test)]
mod tests;
