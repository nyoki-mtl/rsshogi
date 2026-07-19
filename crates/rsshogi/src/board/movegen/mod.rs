//! 指し手生成
//!
//! このモジュールはジェネリクスによる型安全な指し手生成 API を提供する。
//! 疑似合法手と合法手を用途別の API で生成する。
//!
//! # 基本的な使い方
//!
//! ```
//! use rsshogi::board::{self, MoveList};
//! use rsshogi::movegen::generate_legal_all;
//!
//! board::init();
//! let pos = board::hirate_position();
//!
//! // 通常省略される不成も含む全合法手を生成
//! let mut moves = MoveList::new();
//! generate_legal_all(&pos, &mut moves);
//!
//! println!("合法手数: {}", moves.len());
//! for m in moves.iter() {
//!     println!("{}", m.to_usi());
//! }
//! ```
//!
//! # 手生成タイプ
//!
//! 型パラメータで生成する手の種類を指定する。
//!
//! | タイプ | 説明 | 王手時 |
//! |--------|------|:------:|
//! | [`Legal`] | legal-only 手（通常省略される不成は省く） | ○ |
//! | [`Captures`] | 駒を取る手 | - |
//! | [`Quiets`] | 駒を取らない手 | - |
//! | [`Evasions`] | 王手回避の pseudo-legal 手 | ○ |
//! | [`Checks`] | 王手をかける手 | - |
//! | [`QuietChecks`] | 駒を取らない王手 | - |
//! | [`Recaptures`] | 取り返し手 | - |
//! | [`NonEvasions`] | 回避以外の手 | - |
//! | [`CapturePlusPro`] | 駒取り + 成り | - |
//!
//! 各タイプには `*All` バリアント（例: `LegalAll`）があり、
//! 通常省略される不成の手も含めて生成する。
//! 全合法手集合を外部へ返す用途では、専用 API の [`generate_legal_all`] を優先する。
//!
//! # リスト版とシンク版
//!
//! 各生成関数は、`MoveList` へ書き出す無印版（例: [`generate_legal_all`]）と、任意の
//! [`MoveSink`] へ書き出す `_into` 版（例: [`generate_legal_all_into`]）を対で持つ。
//! `Move32` 版は `_move32` / `_move32_into` で同じ対を持つ。
//!
//! # 王手時の手生成
//!
//! 王手されている場合は [`Evasions`] を使用する必要がある。
//! [`Legal`] は内部で自動的に判定する。
//!
//! ```
//! use rsshogi::board::{self, MoveList, position_from_sfen};
//! use rsshogi::movegen::{Evasions, Legal, generate_moves};
//!
//! board::init();
//!
//! // 王手されている局面
//! let pos = position_from_sfen(
//!     "4k4/9/4G4/9/9/9/9/9/4K4 w - 1"  // 後手玉に金で王手
//! ).unwrap();
//!
//! // 方法1: Legal を使う（自動判定）
//! let mut moves = MoveList::new();
//! generate_moves::<Legal>(&pos, &mut moves);
//!
//! // 方法2: Evasions を明示的に使う
//! let mut evasions = MoveList::new();
//! generate_moves::<Evasions>(&pos, &mut evasions);
//!
//! // Evasions は pseudo-legal なので、legal-only とは限らない。
//! // legal-only が必要なら generate_legal_evasions / generate_legal_all を使う。
//! ```
//!
//! # 特定のマスへの手生成
//!
//! ```
//! use rsshogi::board::{self, MoveList};
//! use rsshogi::movegen::{Recaptures, generate_recaptures};
//! use rsshogi::types::Square;
//!
//! board::init();
//! let pos = board::hirate_position();
//!
//! // 特定のマス（例: 5五）への取り返し手を生成
//! let target = Square::from_file_rank(
//!     rsshogi::types::File::FILE_5,
//!     rsshogi::types::Rank::RANK_5
//! );
//! let mut moves = MoveList::new();
//! generate_recaptures(&pos, target, &mut moves);
//! ```
//!
//! # 成りの扱い
//!
//! デフォルトでは探索効率のため、明らかに不利な不成は生成しない。
//! 例えば、敵陣での角の不成は通常生成されない。
//!
//! 全ての合法手（不成を含む）を生成するには [`generate_legal_all`] を使用する：
//!
//! ```
//! use rsshogi::board::{self, MoveList};
//! use rsshogi::movegen::{Legal, generate_legal_all, generate_moves};
//!
//! board::init();
//! let pos = board::hirate_position();
//!
//! let mut normal = MoveList::new();
//! generate_moves::<Legal>(&pos, &mut normal);
//!
//! let mut all = MoveList::new();
//! generate_legal_all(&pos, &mut all);
//!
//! // all の方が多くなる可能性がある（不成を含むため）
//! assert!(all.len() >= normal.len());
//! ```
//!
//! `generate_moves::<LegalAll>` も generic mode として利用できるが、通常の全合法手列挙では
//! 専用 API の [`generate_legal_all`] が意図を最も明確に表す。
//!
//! # Move32 リストの直接生成
//!
//! 探索や perft で `Move32`（駒情報付き 32bit 指し手）が必要な場合、
//! [`generate_legal_all_move32`] / [`generate_moves_move32`] を使用する。
//! 王手回避を legal-only で直接欲しい場合は
//! [`generate_legal_evasions_move32`] / [`generate_legal_evasions_all_move32`] を使用する。
//! 生成パイプライン内で駒情報を直接伝播するため、`move32_from_move` 変換が不要である。
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
//! for m in moves.iter() {
//!     println!("{}", m.to_usi());
//! }
//! ```

#![allow(clippy::inline_always)]

mod checks;
mod drops;
mod evasions;
mod generate;
mod pieces;
mod promotions;
mod recaptures;
mod types;

#[cfg(test)]
mod tests;

// `MoveListGen`（`book` / `records` データ機能の内部専用）でのみ使う。
#[cfg(any(feature = "book", feature = "records"))]
use std::marker::PhantomData;

use crate::board::Position;
use crate::board::move_list::{Move32List, MoveList};
use crate::types::{Bitboard, Color, Move, Move32, Piece, PieceType, Square};

pub use checks::{
    generate_checks, generate_checks_drops_part, generate_checks_drops_part_move32,
    generate_checks_move32, generate_quiet_checks, generate_quiet_checks_move32,
};
pub use evasions::{generate_evasions, generate_evasions_move32};
pub use generate::{
    generate_legal_all, generate_legal_all_into, generate_legal_evasions,
    generate_legal_evasions_all, generate_legal_evasions_all_into, generate_legal_evasions_into,
    generate_moves, generate_moves_into, generate_moves_to, generate_moves_to_into,
};
pub use recaptures::{
    generate_recaptures, generate_recaptures_all, generate_recaptures_all_move32,
    generate_recaptures_move32,
};
pub use types::{
    CapturePlusPro, CapturePlusProAll, Captures, CapturesAll, Checks, ChecksAll, Evasions,
    EvasionsAll, Legal, LegalAll, MoveGenType, NonEvasions, NonEvasionsAll, QuietChecks,
    QuietChecksAll, Quiets, QuietsAll, QuietsProMinus, QuietsProMinusAll, Recaptures,
    RecapturesAll,
};

pub(crate) trait ColorMarker {
    const COLOR: Color;
    const THEM: Color;
    const IS_BLACK: bool;
}

pub(crate) struct Black;
pub(crate) struct White;

impl ColorMarker for Black {
    const COLOR: Color = Color::BLACK;
    const THEM: Color = Color::WHITE;
    const IS_BLACK: bool = true;
}

impl ColorMarker for White {
    const COLOR: Color = Color::WHITE;
    const THEM: Color = Color::BLACK;
    const IS_BLACK: bool = false;
}

pub trait MoveSink {
    fn push_move(&mut self, mv: Move);
    fn retain_unordered<F>(&mut self, f: F)
    where
        F: FnMut(Move) -> bool;

    /// 通常移動手を駒情報付きでプッシュ。
    ///
    /// デフォルト実装は駒情報を無視し `push_move` に委譲する。
    /// `Move32SinkAdapter` がオーバーライドして `piece_on` なしで `Move32` を直接構築する。
    #[inline]
    fn push_normal(&mut self, from: Square, to: Square, _piece: Piece) {
        self.push_move(Move::normal_fast(from, to));
    }

    /// 成り手を駒情報付きでプッシュ。
    ///
    /// `piece` は成り前の駒。デフォルト実装は駒情報を無視する。
    #[inline]
    fn push_promotion(&mut self, from: Square, to: Square, _piece: Piece) {
        self.push_move(Move::promotion_fast(from, to));
    }

    /// 成り手を出力し、必要なら同じ `to` の不成手を続けて出力する。
    #[inline]
    fn push_promotion_then_optional_normal(
        &mut self,
        from: Square,
        to: Square,
        piece: Piece,
        include_normal: bool,
    ) {
        self.push_promotion(from, to, piece);
        if include_normal {
            self.push_normal(from, to, piece);
        }
    }

    /// 同一 `from` / `piece` について、各 `to` ごとに `成り -> (任意で)不成` を出力する。
    #[inline]
    fn push_promotion_then_normals_with_piece(
        &mut self,
        from: Square,
        piece: Piece,
        mut destinations: Bitboard,
        include_normal: bool,
    ) {
        while let Some(to) = destinations.pop_lsb() {
            self.push_promotion_then_optional_normal(from, to, piece, include_normal);
        }
    }

    /// 同一 `from` / `piece` の通常手をまとめてプッシュする。
    ///
    /// デフォルト実装は `push_normal` を繰り返し呼ぶ。
    #[inline]
    fn push_normals_with_piece(&mut self, from: Square, piece: Piece, mut destinations: Bitboard) {
        while let Some(to) = destinations.pop_lsb() {
            self.push_normal(from, to, piece);
        }
    }

    /// 同一 `from` / `piece` の成り手をまとめてプッシュする。
    ///
    /// デフォルト実装は `push_promotion` を繰り返し呼ぶ。
    #[inline]
    fn push_promotions_with_piece(
        &mut self,
        from: Square,
        piece: Piece,
        mut destinations: Bitboard,
    ) {
        while let Some(to) = destinations.pop_lsb() {
            self.push_promotion(from, to, piece);
        }
    }

    /// 駒打ちを駒情報付きでプッシュ。
    ///
    /// デフォルト実装は `color` を無視する。
    #[inline]
    fn push_drop(&mut self, pt: PieceType, to: Square, _color: Color) {
        self.push_move(Move::drop_fast(pt, to));
    }

    /// 事前エンコード済みの駒打ち手をプッシュ。
    ///
    /// - `base`: 下位16bit は `MOVE_DROP | (piece_type << 7)`、上位16bit は任意。
    ///   `Move32` sink では上位16bitに移動後駒を載せる。
    #[inline]
    fn push_drop_encoded(&mut self, base: u32, to: Square) {
        self.push_move(Move::from_raw((base as u16) | to.raw() as u16));
    }

    #[inline]
    fn push_moves_from(&mut self, from: Square, mut destinations: Bitboard) {
        let base = (from.raw() as u16) << 7;
        while let Some(to) = destinations.pop_lsb() {
            let mv = Move::from_raw(base | to.raw() as u16);
            self.push_move(mv);
        }
    }

    #[inline]
    fn push_promotions_from(&mut self, from: Square, mut destinations: Bitboard) {
        let base = Move::MOVE_PROMOTE | ((from.raw() as u16) << 7);
        while let Some(to) = destinations.pop_lsb() {
            let mv = Move::from_raw(base | to.raw() as u16);
            self.push_move(mv);
        }
    }

    #[inline]
    fn push_drops_to(&mut self, piece_type: PieceType, mut destinations: Bitboard) {
        let base = u32::from(Move::MOVE_DROP | ((piece_type.to_index() as u16) << 7));
        while let Some(to) = destinations.pop_lsb() {
            self.push_drop_encoded(base, to);
        }
    }
}

impl MoveSink for MoveList {
    #[inline]
    fn push_move(&mut self, mv: Move) {
        // SAFETY: MoveList は MAX_MOVES 超過が起きない前提のホットパス。
        unsafe { self.push_unchecked(mv) };
    }

    #[inline]
    fn retain_unordered<F>(&mut self, f: F)
    where
        F: FnMut(Move) -> bool,
    {
        self.retain_unordered(f);
    }

    #[inline]
    fn push_promotion_then_optional_normal(
        &mut self,
        from: Square,
        to: Square,
        _piece: Piece,
        include_normal: bool,
    ) {
        // SAFETY: 手生成内では MoveList 容量超過しない前提。
        unsafe { self.push_unchecked(Move::promotion_fast(from, to)) };
        if include_normal {
            // SAFETY: 手生成内では MoveList 容量超過しない前提。
            unsafe { self.push_unchecked(Move::normal_fast(from, to)) };
        }
    }

    #[inline]
    fn push_promotion_then_normals_with_piece(
        &mut self,
        from: Square,
        _piece: Piece,
        mut destinations: Bitboard,
        include_normal: bool,
    ) {
        let promote_base = Move::MOVE_PROMOTE | ((from.raw() as u16) << 7);
        let normal_base = (from.raw() as u16) << 7;
        while let Some(to) = destinations.pop_lsb() {
            let to_raw = to.raw() as u16;
            // SAFETY: 手生成内では MoveList 容量超過しない前提。
            unsafe { self.push_unchecked(Move::from_raw(promote_base | to_raw)) };
            if include_normal {
                // SAFETY: 手生成内では MoveList 容量超過しない前提。
                unsafe { self.push_unchecked(Move::from_raw(normal_base | to_raw)) };
            }
        }
    }
}

/// `Move32` を直接受け取るシンク。
///
/// `Move32List` へ書き込むのと同じく、生成パイプライン中の駒情報を保ったまま
/// 外部コンテナへ直接流し込みたい場合に使用する。
pub trait Move32Sink {
    fn push_move32(&mut self, mv: Move32);
    fn retain_unordered<F>(&mut self, f: F)
    where
        F: FnMut(Move32) -> bool;
}

impl Move32Sink for Move32List {
    #[inline]
    fn push_move32(&mut self, mv: Move32) {
        // SAFETY: Move32List は MAX_MOVES 超過が起きない前提のホットパス。
        unsafe { self.push_unchecked(mv) };
    }

    #[inline]
    fn retain_unordered<F>(&mut self, f: F)
    where
        F: FnMut(Move32) -> bool,
    {
        self.retain_unordered(f);
    }
}

/// 生成種別を型で指定する指し手リスト。
///
/// `book` / `records` データ機能の内部でのみ使用するため、`book` または `records`
/// feature 有効時にのみコンパイルされる（コアの探索 consumer は使用しない）。
///
/// ```
/// use rsshogi::board::{self, movegen::MoveListGen};
/// use rsshogi::movegen::Legal;
///
/// board::init();
/// let pos = board::hirate_position();
///
/// let moves = MoveListGen::<Legal>::new(&pos);
/// println!("合法手数: {}", moves.len());
/// ```
#[cfg(any(feature = "book", feature = "records"))]
pub struct MoveListGen<T: MoveGenType> {
    moves: MoveList,
    _marker: PhantomData<T>,
}

#[cfg(any(feature = "book", feature = "records"))]
impl<T: MoveGenType + 'static> MoveListGen<T> {
    #[must_use]
    pub fn new(pos: &Position) -> Self {
        let mut moves = MoveList::new();
        debug_assert!(!T::IS_RECAPTURES, "MoveListGen::new is not for Recaptures");
        generate_moves::<T>(pos, &mut moves);
        Self { moves, _marker: PhantomData }
    }

    #[must_use]
    pub fn new_with_target(pos: &Position, target_sq: Square) -> Self {
        let mut moves = MoveList::new();
        if T::IS_RECAPTURES {
            generate_moves_to::<T>(pos, target_sq, &mut moves);
        } else {
            generate_moves::<T>(pos, &mut moves);
        }
        Self { moves, _marker: PhantomData }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Move> {
        self.moves.iter()
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.moves.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }

    #[must_use]
    pub fn contains(&self, mv: Move) -> bool {
        self.moves.iter().any(|&m| m == mv)
    }

    #[must_use]
    pub fn at(&self, idx: usize) -> Move {
        debug_assert!(idx < self.moves.len(), "index out of bounds");
        self.moves.as_slice()[idx]
    }
}

#[cfg(any(feature = "book", feature = "records"))]
impl<'a, T: MoveGenType> IntoIterator for &'a MoveListGen<T> {
    type Item = &'a Move;
    type IntoIter = std::slice::Iter<'a, Move>;

    fn into_iter(self) -> Self::IntoIter {
        self.moves.as_slice().iter()
    }
}

// --- Move32 生成アダプタ ---

/// `MoveSink` を実装し、`Move32` を直接構築するアダプタ。
///
/// 駒情報付きメソッド (`push_normal`, `push_promotion`, `push_drop`) では
/// `piece_on` ルックアップなしで `Move32` を直接構築する。
/// `push_move` フォールバックでは `move32_from_move` で変換する。
struct Move32SinkAdapter<'a, S> {
    pos: &'a Position,
    sink: &'a mut S,
}

/// `Move` の raw 値と移動後の駒から `Move32` を直接構築する。
#[inline(always)]
fn make_move32_fast(mv_raw: u16, piece_after: Piece) -> Move32 {
    Move32::from_raw((u32::from(piece_after.raw() as u8 & 0x1f) << 16) | u32::from(mv_raw))
}

impl<S: Move32Sink> MoveSink for Move32SinkAdapter<'_, S> {
    #[inline]
    fn push_move(&mut self, mv: Move) {
        // フォールバック: 駒情報が渡されなかった場合
        // SAFETY: push_move は合法手生成の内部でのみ呼ばれ、mv は有効な移動手/駒打ち手。
        let mv32 = unsafe { self.pos.move32_from_move_fast(mv) };
        self.sink.push_move32(mv32);
    }

    #[inline]
    fn push_normal(&mut self, from: Square, to: Square, piece: Piece) {
        let mv32 = make_move32_fast(Move::normal_fast(from, to).raw(), piece);
        self.sink.push_move32(mv32);
    }

    #[inline]
    fn push_promotion(&mut self, from: Square, to: Square, piece: Piece) {
        let mv32 = make_move32_fast(Move::promotion_fast(from, to).raw(), piece.promote());
        self.sink.push_move32(mv32);
    }

    #[inline]
    fn push_promotion_then_optional_normal(
        &mut self,
        from: Square,
        to: Square,
        piece: Piece,
        include_normal: bool,
    ) {
        let to_raw = to.raw() as u32;
        let promote_base = (u32::from(piece.promote().raw() as u8 & 0x1f) << 16)
            | u32::from(Move::MOVE_PROMOTE)
            | ((from.raw() as u32) << 7);
        let mv32 = Move32::from_raw(promote_base | to_raw);
        self.sink.push_move32(mv32);
        if include_normal {
            let normal_base =
                (u32::from(piece.raw() as u8 & 0x1f) << 16) | ((from.raw() as u32) << 7);
            let mv32 = Move32::from_raw(normal_base | to_raw);
            self.sink.push_move32(mv32);
        }
    }

    #[inline]
    fn push_promotion_then_normals_with_piece(
        &mut self,
        from: Square,
        piece: Piece,
        mut destinations: Bitboard,
        include_normal: bool,
    ) {
        let promote_base = (u32::from(piece.promote().raw() as u8 & 0x1f) << 16)
            | u32::from(Move::MOVE_PROMOTE)
            | ((from.raw() as u32) << 7);
        let normal_base = (u32::from(piece.raw() as u8 & 0x1f) << 16) | ((from.raw() as u32) << 7);
        while let Some(to) = destinations.pop_lsb() {
            let to_raw = to.raw() as u32;
            let mv32 = Move32::from_raw(promote_base | to_raw);
            self.sink.push_move32(mv32);
            if include_normal {
                let mv32 = Move32::from_raw(normal_base | to_raw);
                self.sink.push_move32(mv32);
            }
        }
    }

    #[inline]
    fn push_normals_with_piece(&mut self, from: Square, piece: Piece, mut destinations: Bitboard) {
        let base = (u32::from(piece.raw() as u8 & 0x1f) << 16) | ((from.raw() as u32) << 7);
        while let Some(to) = destinations.pop_lsb() {
            let mv32 = Move32::from_raw(base | to.raw() as u32);
            self.sink.push_move32(mv32);
        }
    }

    #[inline]
    fn push_promotions_with_piece(
        &mut self,
        from: Square,
        piece: Piece,
        mut destinations: Bitboard,
    ) {
        let promoted = piece.promote();
        let base = (u32::from(promoted.raw() as u8 & 0x1f) << 16)
            | u32::from(Move::MOVE_PROMOTE)
            | ((from.raw() as u32) << 7);
        while let Some(to) = destinations.pop_lsb() {
            let mv32 = Move32::from_raw(base | to.raw() as u32);
            self.sink.push_move32(mv32);
        }
    }

    #[inline]
    fn push_drop(&mut self, pt: PieceType, to: Square, color: Color) {
        let piece = Piece::from_parts(color, pt);
        let base = (u32::from(piece.raw() as u8 & 0x1f) << 16)
            | u32::from(Move::MOVE_DROP | ((pt.to_index() as u16) << 7));
        let mv32 = Move32::from_raw(base | to.raw() as u32);
        self.sink.push_move32(mv32);
    }

    #[inline]
    fn push_drop_encoded(&mut self, base: u32, to: Square) {
        let mv32 = Move32::from_raw(base | to.raw() as u32);
        self.sink.push_move32(mv32);
    }

    #[inline]
    fn retain_unordered<F>(&mut self, mut f: F)
    where
        F: FnMut(Move) -> bool,
    {
        self.sink.retain_unordered(|mv32| f(mv32.to_move()));
    }
}

/// `Move32` シンクへ指し手を直接生成する。
pub fn generate_moves_move32_into<T: MoveGenType + 'static, S: Move32Sink>(
    pos: &Position,
    sink: &mut S,
) {
    let mut adapter = Move32SinkAdapter { pos, sink };
    generate::generate_moves_into::<T, _>(pos, &mut adapter);
}

/// Move32リストとして指し手を生成する。
pub fn generate_moves_move32<T: MoveGenType + 'static>(pos: &Position, list: &mut Move32List) {
    generate_moves_move32_into::<T, _>(pos, list);
}

/// `Move32` シンクへ指定マスへの移動手を直接生成する。
pub fn generate_moves_to_move32_into<T: MoveGenType + 'static, S: Move32Sink>(
    pos: &Position,
    target_sq: Square,
    sink: &mut S,
) {
    let mut adapter = Move32SinkAdapter { pos, sink };
    generate::generate_moves_to_into::<T, _>(pos, target_sq, &mut adapter);
}

/// Move32リストとして指定マスへの移動手を生成する。
pub fn generate_moves_to_move32<T: MoveGenType + 'static>(
    pos: &Position,
    target_sq: Square,
    list: &mut Move32List,
) {
    generate_moves_to_move32_into::<T, _>(pos, target_sq, list);
}

#[inline]
fn retain_generated_legal_move32_for_color<C: ColorMarker>(
    pos: &Position,
    sink: &mut impl Move32Sink,
) {
    // `LEGAL_ALL` は生成後に split legality 判定で絞り込む。
    // Move32 リストでは Move への変換を挟まず、同じ判定を直接行う。
    let us = C::COLOR;
    let king_sq = pos.king_square(us);
    if king_sq.is_none() {
        return;
    }
    let blockers = pos.blockers_for_king(us);

    sink.retain_unordered(|mv32| {
        if mv32.is_drop() {
            return true;
        }

        let from = mv32.from_sq();
        if from != king_sq && !blockers.test(from) {
            return true;
        }

        if from == king_sq {
            if C::IS_BLACK {
                !pos.is_attacked_by_color_with_king_for::<false>(mv32.to_sq(), king_sq)
            } else {
                !pos.is_attacked_by_color_with_king_for::<true>(mv32.to_sq(), king_sq)
            }
        } else {
            Bitboard::is_aligned(from, mv32.to_sq(), king_sq)
        }
    });
}

#[inline]
fn generate_legal_all_move32_into_non_evasions_for_color<C: ColorMarker>(
    pos: &Position,
    sink: &mut impl Move32Sink,
) {
    let us = C::COLOR;
    let bb = pos.bitboards();
    let our_pieces = bb.color_pieces(us);
    let occupied = bb.occupied();
    let target = !our_pieces;
    let pawn_target = target;

    {
        let mut adapter = Move32SinkAdapter { pos, sink };
        pieces::generate_pawn_moves::<NonEvasionsAll, C>(pos, &mut adapter, pawn_target);
        pieces::generate_lance_moves::<NonEvasionsAll, C>(pos, &mut adapter, target, occupied);
        pieces::generate_knight_moves::<NonEvasionsAll, C>(pos, &mut adapter, target);
        pieces::generate_silver_moves::<NonEvasionsAll, C>(pos, &mut adapter, target);
        pieces::generate_br_moves::<NonEvasionsAll, C>(pos, &mut adapter, target, occupied);
        pieces::generate_gold_hdk_moves::<NonEvasionsAll, C>(pos, &mut adapter, target, occupied);
    }
    retain_generated_legal_move32_for_color::<C>(pos, sink);
    let mut adapter = Move32SinkAdapter { pos, sink };
    drops::generate_drops_color::<NonEvasionsAll, C>(pos, &mut adapter);
}

#[inline]
fn generate_legal_evasions_move32_into_for_color<T: MoveGenType + 'static, C: ColorMarker>(
    pos: &Position,
    sink: &mut impl Move32Sink,
) {
    let mut adapter = Move32SinkAdapter { pos, sink };
    evasions::generate_evasions_for_color::<T, C>(pos, &mut adapter);
    retain_generated_legal_move32_for_color::<C>(pos, sink);
}

/// `Move32` シンクへ王手回避の legal-only 手を直接生成する。
///
/// `generate_moves_move32::<Evasions>()` が返す pseudo-legal evasion を、
/// `LEGAL_ALL` と同じ split legality fast path で legal-only に絞り込む。
/// 通常省略される不成は含まない。
pub fn generate_legal_evasions_move32_into<S: Move32Sink>(pos: &Position, sink: &mut S) {
    debug_assert!(!pos.checkers().is_empty(), "generate_legal_evasions_move32_into expects check");
    match pos.turn() {
        Color::BLACK => generate_legal_evasions_move32_into_for_color::<Evasions, Black>(pos, sink),
        Color::WHITE => generate_legal_evasions_move32_into_for_color::<Evasions, White>(pos, sink),
    }
}

/// Move32 リストとして王手回避の legal-only 手を生成する。
///
/// 通常省略される不成は含まない。すべての不成を含めたい場合は
/// [`generate_legal_evasions_all_move32`] を使う。
pub fn generate_legal_evasions_move32(pos: &Position, list: &mut Move32List) {
    generate_legal_evasions_move32_into(pos, list);
}

/// `Move32` シンクへ王手回避の legal-only 手を直接生成する（不成を省略しない）。
///
/// `generate_moves_move32::<EvasionsAll>()` が返す pseudo-legal evasion を、
/// `LEGAL_ALL` と同じ split legality fast path で legal-only に絞り込む。
pub fn generate_legal_evasions_all_move32_into<S: Move32Sink>(pos: &Position, sink: &mut S) {
    debug_assert!(
        !pos.checkers().is_empty(),
        "generate_legal_evasions_all_move32_into expects check"
    );
    match pos.turn() {
        Color::BLACK => {
            generate_legal_evasions_move32_into_for_color::<EvasionsAll, Black>(pos, sink)
        }
        Color::WHITE => {
            generate_legal_evasions_move32_into_for_color::<EvasionsAll, White>(pos, sink)
        }
    }
}

/// Move32 リストとして王手回避の legal-only 手を生成する（不成を省略しない）。
pub fn generate_legal_evasions_all_move32(pos: &Position, list: &mut Move32List) {
    generate_legal_evasions_all_move32_into(pos, list);
}

/// `Move32` シンクへ全合法手を直接生成する。
pub fn generate_legal_all_move32_into<S: Move32Sink>(pos: &Position, sink: &mut S) {
    let in_check = !pos.checkers().is_empty();
    match pos.turn() {
        Color::BLACK => {
            if in_check {
                generate_legal_evasions_all_move32_into(pos, sink);
            } else {
                generate_legal_all_move32_into_non_evasions_for_color::<Black>(pos, sink);
            }
        }
        Color::WHITE => {
            if in_check {
                generate_legal_evasions_all_move32_into(pos, sink);
            } else {
                generate_legal_all_move32_into_non_evasions_for_color::<White>(pos, sink);
            }
        }
    }
}

#[inline]
fn generate_legal_all_move32_non_evasions_for_color<C: ColorMarker>(
    pos: &Position,
    list: &mut Move32List,
) {
    let us = C::COLOR;
    let bb = pos.bitboards();
    let our_pieces = bb.color_pieces(us);
    let occupied = bb.occupied();
    let target = !our_pieces;
    let pawn_target = target;

    {
        let mut adapter = Move32SinkAdapter { pos, sink: list };
        pieces::generate_pawn_moves::<NonEvasionsAll, C>(pos, &mut adapter, pawn_target);
        pieces::generate_lance_moves::<NonEvasionsAll, C>(pos, &mut adapter, target, occupied);
        pieces::generate_knight_moves::<NonEvasionsAll, C>(pos, &mut adapter, target);
        pieces::generate_silver_moves::<NonEvasionsAll, C>(pos, &mut adapter, target);
        pieces::generate_br_moves::<NonEvasionsAll, C>(pos, &mut adapter, target, occupied);
        pieces::generate_gold_hdk_moves::<NonEvasionsAll, C>(pos, &mut adapter, target, occupied);
    }
    retain_generated_legal_move32_for_color::<C>(pos, list);
    // 駒打ちは生成段階で合法性（特に打ち歩詰め）を満たすため、retain 後に追加する。
    drops::generate_drops_color_move32::<NonEvasionsAll, C>(pos, list);
}

/// `Move32` リストとして全合法手を生成する。
pub fn generate_legal_all_move32(pos: &Position, list: &mut Move32List) {
    let in_check = !pos.checkers().is_empty();
    match pos.turn() {
        Color::BLACK => {
            if in_check {
                generate_legal_evasions_all_move32(pos, list);
            } else {
                generate_legal_all_move32_non_evasions_for_color::<Black>(pos, list);
            }
        }
        Color::WHITE => {
            if in_check {
                generate_legal_evasions_all_move32(pos, list);
            } else {
                generate_legal_all_move32_non_evasions_for_color::<White>(pos, list);
            }
        }
    }
}
