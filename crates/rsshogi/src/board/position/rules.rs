#![allow(clippy::inline_always)]

use super::Position;
use super::types::{DeclarationDetail, DeclarationEvaluation, Ply};
use crate::board::attack_tables::KING_ATTACKS;
#[cfg(test)]
use crate::board::state_info::CheckSquares;
use crate::board::zobrist::ZobristKey;
use crate::types::Bitboard;
use crate::types::{
    Color, EnteringKingRule, Hand, HandPiece, MOVE_NONE, MOVE_WIN, Move, Move32, MoveBase, Piece,
    PieceType, Rank, RepetitionState, SQ_51, SQ_59, Square,
};
use std::sync::atomic::{AtomicU16, Ordering};

/// 千日手判定で遡る最大手数
const DEFAULT_MAX_REPETITION_PLY: Ply = 16;
static MAX_REPETITION_PLY: AtomicU16 = AtomicU16::new(DEFAULT_MAX_REPETITION_PLY);

#[inline]
fn max_repetition_ply() -> Ply {
    MAX_REPETITION_PLY.load(Ordering::Relaxed)
}

impl Position {
    /// 指定マスが指定色に攻撃されているか判定（玉の移動用）
    #[inline(always)]
    #[must_use]
    pub fn is_attacked_by_color_with_king(
        &self,
        by_color: Color,
        sq: Square,
        king_sq: Square,
    ) -> bool {
        match by_color {
            Color::BLACK => self.is_attacked_by_color_with_king_for::<true>(sq, king_sq),
            Color::WHITE => self.is_attacked_by_color_with_king_for::<false>(sq, king_sq),
        }
    }

    #[inline(always)]
    pub(crate) fn is_attacked_by_color_with_king_for<const BLACK: bool>(
        &self,
        sq: Square,
        king_sq: Square,
    ) -> bool {
        let occupied_without_king =
            self.bitboards().occupied().and_not(Bitboard::from_square(king_sq));
        self.attacked_by_color_fast_for::<BLACK>(sq, occupied_without_king)
    }

    /// 指定した手が王手をかけるか判定
    #[must_use]
    #[inline]
    pub fn gives_check_move32(&self, mv: Move32) -> bool {
        let them = self.turn().flip();
        let king_sq = self.king_square(them);
        if king_sq.is_none() {
            return false;
        }
        let to = mv.to_sq();

        if mv.is_drop() {
            self.check_square(mv.piece_after_move().piece_type()).test(to)
        } else {
            let from = mv.from_sq();
            let moved_pt = mv.piece_after_move().piece_type();
            let direct_check = self.check_square(moved_pt).test(to);
            let discovered_check =
                self.blockers_for_king(them).test(from) && !Bitboard::is_aligned(from, to, king_sq);
            direct_check || discovered_check
        }
    }

    /// `Move` 版の王手判定。
    #[must_use]
    #[inline]
    pub fn gives_check_move(&self, mv: Move) -> bool {
        self.gives_check_impl(mv)
    }

    /// 指定した手が王手をかけるか判定（同一局面の check_squares snapshot を利用）。
    #[cfg(test)]
    #[must_use]
    #[inline]
    pub(crate) fn gives_check_with_check_squares(
        &self,
        mv: Move32,
        check_squares: &CheckSquares,
    ) -> bool {
        let them = self.turn().flip();
        let king_sq = self.king_square(them);
        if king_sq.is_none() {
            return false;
        }
        let to = mv.to_sq();

        if mv.is_drop() {
            check_squares.get(mv.piece_after_move().piece_type()).test(to)
        } else {
            let from = mv.from_sq();
            let moved_pt = mv.piece_after_move().piece_type();
            let direct_check = check_squares.get(moved_pt).test(to);
            let discovered_check =
                self.blockers_for_king(them).test(from) && !Bitboard::is_aligned(from, to, king_sq);
            direct_check || discovered_check
        }
    }

    /// 王手判定の内部実装。`Move` / `Move32` 共通。
    #[inline]
    fn gives_check_impl(&self, mv: impl MoveBase) -> bool {
        let m = mv.to_move();
        let us = self.turn();
        let them = us.flip();

        let king_sq = self.king_square(them);
        if king_sq.is_none() {
            return false;
        }

        if m.is_drop() {
            let Some(piece_type) = m.dropped_piece() else {
                return false;
            };
            let to = m.to_sq();
            self.check_square(piece_type).test(to)
        } else {
            let from = m.from_sq();
            let to = m.to_sq();
            let piece = self.piece_on(from);
            if piece == Piece::NONE {
                return false;
            }
            let moved_pt =
                if m.is_promotion() { piece.piece_type().promote() } else { piece.piece_type() };

            let direct_check = self.check_square(moved_pt).test(to);
            let discovered_check =
                self.blockers_for_king(them).test(from) && !Bitboard::is_aligned(from, to, king_sq);

            direct_check || discovered_check
        }
    }

    /// from->to の移動が空き王手になるかを判定する。
    #[must_use]
    #[inline]
    pub fn is_discovered_check(
        &self,
        from: Square,
        to: Square,
        our_king: Square,
        pinned: Bitboard,
    ) -> bool {
        pinned.test(from) && !Bitboard::is_aligned(from, to, our_king)
    }

    /// 捕獲する指し手かどうか
    #[must_use]
    #[inline]
    pub fn is_capture_move32(&self, mv: Move32) -> bool {
        debug_assert!(mv.is_normal(), "is_capture_move32 expects a normal move");
        self.is_capture_impl(mv)
    }

    /// `Move` 版の捕獲判定。
    #[must_use]
    #[inline]
    pub fn is_capture_move(&self, mv: Move) -> bool {
        debug_assert!(mv.is_normal(), "is_capture_move expects a normal move");
        self.is_capture_impl(mv)
    }

    #[inline]
    fn is_capture_impl(&self, mv: impl MoveBase) -> bool {
        let m = mv.to_move();
        !m.is_drop() && self.piece_on(m.to_sq()) != Piece::NONE
    }

    /// 捕獲または成りの指し手かどうか
    #[must_use]
    #[inline]
    pub fn is_capture_or_promotion(&self, mv: Move32) -> bool {
        mv.is_promotion() || self.is_capture_move32(mv)
    }

    /// 歩の成り指し手かどうか
    #[must_use]
    #[inline]
    pub fn is_pawn_promotion(&self, mv: Move32) -> bool {
        self.is_pawn_promotion_impl(mv)
    }

    #[inline]
    fn is_pawn_promotion_impl(&self, mv: impl MoveBase) -> bool {
        let m = mv.to_move();
        if !m.is_promotion() || m.is_drop() {
            return false;
        }
        let from = m.from_sq();
        let piece = self.piece_on(from);
        piece.piece_type() == PieceType::PAWN
    }

    /// 捕獲または歩の成り指し手かどうか
    #[must_use]
    #[inline]
    pub fn is_capture_or_pawn_promotion(&self, mv: Move32) -> bool {
        self.is_pawn_promotion(mv) || self.is_capture_move32(mv)
    }

    /// 捕獲または価値のある駒の成り（歩・角・飛）かどうか
    #[must_use]
    #[inline]
    pub fn is_capture_or_valuable_promotion(&self, mv: Move32) -> bool {
        self.is_capture_or_valuable_promotion_impl(mv)
    }

    #[inline]
    fn is_capture_or_valuable_promotion_impl(&self, mv: impl MoveBase) -> bool {
        let m = mv.to_move();
        if m.is_promotion() {
            let from = m.from_sq();
            let raw = self.piece_on(from).piece_type();
            if matches!(raw, PieceType::PAWN | PieceType::BISHOP | PieceType::ROOK) {
                return true;
            }
        }
        self.is_capture_impl(mv)
    }

    /// MovePicker の capture 段階で生成される指し手かどうか
    #[must_use]
    #[inline]
    pub fn is_capture_stage(&self, mv: Move32) -> bool {
        self.is_capture_move32(mv)
    }

    /// 指定された手が合法かどうかを判定（自殺手チェックのみ）。
    #[inline]
    fn is_legal_after_pseudo_impl(&self, mv: impl MoveBase) -> bool {
        let m = mv.to_move();
        if m.is_drop() {
            return true;
        }

        let us = self.turn();
        let from = m.from_sq();
        let to = m.to_sq();
        let piece = self.piece_on(from);
        if piece.piece_type() == PieceType::KING {
            return self.is_legal_king_move(us, from, to);
        }

        self.is_legal_non_king_move(us, from, to)
    }

    #[inline]
    fn is_legal_king_move(&self, us: Color, from: Square, to: Square) -> bool {
        !self.is_attacked_by_color_with_king(us.flip(), to, from)
    }

    #[inline]
    fn is_legal_non_king_move(&self, us: Color, from: Square, to: Square) -> bool {
        let pinned = self.blockers_for_king(us);
        if !pinned.test(from) {
            return true;
        }
        let king_sq = self.king_square(us);
        if king_sq.is_none() {
            return true;
        }
        Bitboard::is_aligned(from, to, king_sq)
    }

    /// `Move32` 版の split legality 判定。
    ///
    /// `mv` がすでに pseudo-legal であり、必要な成り条件チェックも済んでいる前提で、
    /// 自玉が取られないかだけを判定する。
    ///
    /// 探索のホットパスで `is_pseudo_legal_move32()` の後段だけを見たい場合に使う。
    /// 駒打ちは pseudo-legal 段階で打ち歩詰めまで検証済みであり、この段階では常に `true` を返す。
    #[must_use]
    #[inline]
    pub fn is_legal_after_pseudo_move32(&self, mv: Move32) -> bool {
        self.is_legal_after_pseudo_impl(mv)
    }

    /// 成りの条件を満たしているか判定する。
    #[inline]
    fn is_legal_promote_impl(&self, mv: impl MoveBase) -> bool {
        let m = mv.to_move();
        if !m.is_promotion() {
            return true;
        }
        let us = self.turn();
        let from = m.from_sq();
        let to = m.to_sq();
        crate::types::square::is_promotable_move(us, from, to)
    }

    /// `Move32` 版の成り条件判定（公開 API）。
    #[must_use]
    #[inline]
    pub fn is_legal_promote(&self, mv: Move32) -> bool {
        self.is_legal_promote_impl(mv)
    }

    /// 指定された手が合法かどうかを判定（pseudo-legal + 自殺手チェック）。
    ///
    /// `MoveBase` を実装する任意の指し手型に対応する内部実装。
    #[inline]
    fn is_legal_impl(&self, mv: impl MoveBase) -> bool {
        self.is_pseudo_legal_impl(mv, true)
            && self.is_legal_promote_impl(mv)
            && self.is_legal_after_pseudo_impl(mv)
    }

    /// `Move32` 版の合法判定。
    #[must_use]
    #[inline]
    pub fn is_legal_move32(&self, mv: Move32) -> bool {
        self.is_legal_impl(mv)
    }

    /// `Move` 版の合法判定。
    #[must_use]
    #[inline]
    pub fn is_legal_move(&self, mv: Move) -> bool {
        self.is_legal_impl(mv)
    }

    pub(crate) fn is_legal_pawn_drop(&self, us: Color, to: Square) -> bool {
        use crate::board::attack_tables::PAWN_ATTACKS;

        let file_bb = Bitboard::file_mask(to.file());
        let nifu = !self.bitboards().pieces_for(PieceType::PAWN, us).and(file_bb).is_empty();
        if nifu {
            return false;
        }

        let them = us.flip();
        let king_sq = self.king_square(them);
        if king_sq.is_none() {
            return true;
        }
        let pawn_attacks = PAWN_ATTACKS[to][us.to_index()];
        if pawn_attacks.test(king_sq) && !self.is_legal_drop(to) {
            return false;
        }

        true
    }

    // ANCHOR: position_drop_pawn_mate
    pub(crate) fn is_legal_drop(&self, to: Square) -> bool {
        use crate::board::attack_tables::KING_ATTACKS;

        let us = self.turn();
        let them = us.flip();

        if !self.is_attacked_by(to, us) {
            return true;
        }

        let attackers = self.attackers_to_pawn(them, to);
        let pinned = self.blockers_for_king(them);
        let file_bb = Bitboard::file_mask(to.file());
        if !attackers.and(pinned.not().or(file_bb)).is_empty() {
            return true;
        }

        let king_sq = self.king_square(them);
        if king_sq.is_none() {
            return true;
        }
        let mut escape_bb = KING_ATTACKS[king_sq].and_not(self.bitboards().color_pieces(them));
        escape_bb.clear(to);

        let mut occupied = self.bitboards().occupied();
        occupied.set(to);

        while let Some(king_to) = escape_bb.pop_lsb() {
            if !self.attacked_by_color_fast(us, king_to, occupied) {
                return true;
            }
        }

        false
    }
    // ANCHOR_END: position_drop_pawn_mate

    #[allow(clippy::too_many_arguments)]
    pub(in crate::board) fn compute_repetition_info_in_place(
        &self,
        stack: &crate::board::state_info::StateStack,
        current_idx: crate::board::state_info::StateIndex,
        plies_from_null: Ply,
        current_hand: Hand,
        current_continuous_check: [u16; 2],
        current_board_key: ZobristKey,
        out_counter: &mut i32,
        out_distance: &mut i32,
        out_times: &mut i32,
        out_type: &mut RepetitionState,
    ) {
        *out_counter = 0;
        *out_distance = 0;
        *out_times = 0;
        *out_type = RepetitionState::None;

        if plies_from_null < 4 {
            return;
        }

        let limit = plies_from_null.min(max_repetition_ply());
        let mut repetition_counter: i32 = 0;
        let mut repetition_distance: i32 = 0;
        let mut repetition_times: i32 = 0;
        let mut repetition_type = RepetitionState::None;

        let mut distance: Ply = 4;

        // 4手前から 2 手ずつ遡る（同一手番のみ比較）。
        while distance <= limit {
            let distance_usize = usize::from(distance);
            if current_idx < distance_usize {
                break;
            }
            let idx = current_idx - distance_usize;
            let state = stack.hot(idx);

            if state.board_key == current_board_key {
                if state.hand == current_hand {
                    repetition_times = state.repetition_times + 1;
                    repetition_counter = repetition_times;
                    repetition_distance = if repetition_times >= 3 {
                        -i32::from(distance)
                    } else {
                        i32::from(distance)
                    };

                    let us = self.turn();
                    repetition_type = if distance <= current_continuous_check[us.to_index()] {
                        RepetitionState::Lose
                    } else if distance <= current_continuous_check[us.flip().to_index()] {
                        RepetitionState::Win
                    } else {
                        RepetitionState::Draw
                    };

                    if state.repetition_times > 0 && repetition_type != state.repetition_type {
                        repetition_type = RepetitionState::Draw;
                    }
                    break;
                }

                if Hand::is_equal_or_superior(current_hand, state.hand) {
                    repetition_type = RepetitionState::Superior;
                    repetition_distance = i32::from(distance);
                    break;
                }

                if Hand::is_equal_or_superior(state.hand, current_hand) {
                    repetition_type = RepetitionState::Inferior;
                    repetition_distance = i32::from(distance);
                    break;
                }
            }

            if state.plies_from_null == 0 {
                break;
            }

            distance += 2;
        }

        *out_counter = repetition_counter;
        *out_distance = repetition_distance;
        *out_times = repetition_times;
        *out_type = repetition_type;
    }

    // ANCHOR: position_is_repetition
    /// 千日手判定。
    ///
    /// 現在の局面が `threshold` 回以上繰り返されていれば `true` を返す。
    /// `threshold` は千日手と判定する繰り返し回数（通常は 3）。
    #[must_use]
    pub fn is_repetition(&self, threshold: u8) -> bool {
        if threshold == 0 {
            return true;
        }

        let stack = self.state_stack();
        stack.hot(self.st_index).repetition_counter >= i32::from(threshold)
    }

    /// 千日手の詳細な状態を判定（探索用、ply 制約付き）
    ///
    /// 指定 ply 以内の千日手状態を判定する。
    /// 同一局面の繰り返しを検出し、連続王手の千日手や優等/劣等局面も判定する。
    #[must_use]
    pub fn repetition_state_with_ply(&self, ply: usize) -> RepetitionState {
        let stack = self.state_stack();
        // 千日手判定の前提条件：repetition_counter >= 3（4回目の出現）
        // 4回目の同一局面の場合は強制的に千日手となるため、ply に関わらず検出
        if stack.hot(self.st_index).repetition_counter >= 3 {
            return self.repetition_state();
        }

        // 遡り可能な手数を計算
        // null move より前と保持範囲の外側は走査しない。
        let plies_from_null = stack.hot(self.st_index).plies_from_null;
        let ply_limit = ply.min(plies_from_null as usize).min(max_repetition_ply() as usize);

        // 少なくとも4手かけないと千日手にはならない
        if ply_limit < 4 {
            return RepetitionState::None;
        }

        let current_state = stack.hot(self.st_index);
        let current_hand = current_state.hand;
        let current_continuous_check = current_state.continuous_check;
        let current_board_key = self.board_key;

        // スタックを遡って同一局面を検索
        // 4手前から、2手ずつ遡る（同一局面に戻るためには偶数手必要）
        let mut distance = 1usize;
        while distance <= ply_limit && distance <= self.st_index {
            let idx = self.st_index - distance;
            let state = stack.hot(idx);

            // 4手以上遡り、かつ偶数手の場合のみチェック
            if distance >= 4 && distance.is_multiple_of(2) {
                if state.board_key == current_board_key {
                    if state.hand == current_hand {
                        let us = self.turn();
                        let repetition_type =
                            if distance <= usize::from(current_continuous_check[us.to_index()]) {
                                RepetitionState::Lose
                            } else if distance
                                <= usize::from(current_continuous_check[us.flip().to_index()])
                            {
                                RepetitionState::Win
                            } else {
                                RepetitionState::Draw
                            };

                        if state.repetition_times > 0 && repetition_type != state.repetition_type {
                            return RepetitionState::Draw;
                        }

                        return repetition_type;
                    }

                    if Hand::is_equal_or_superior(current_hand, state.hand) {
                        return RepetitionState::Superior;
                    }

                    if Hand::is_equal_or_superior(state.hand, current_hand) {
                        return RepetitionState::Inferior;
                    }
                }

                // plies_from_null == 0 に到達したら終了
                if state.plies_from_null == 0 {
                    break;
                }
            }

            distance += 1;
        }

        RepetitionState::None
    }

    /// 千日手の詳細な状態を判定（探索用、ply 制約付き、cached/current-state ベース）
    ///
    /// `current_state()` にキャッシュされた `repetition_distance` /
    /// `repetition_type` をそのまま用いる lightweight 判定。
    /// `repetition_distance < ply` の signed 比較で判定するため、
    /// 4回目以降の同一局面で `repetition_distance < 0` の場合は `ply == 0` でも
    /// 即座に千日手を返す。
    ///
    /// `repetition_state_with_ply()` とは意味が異なり、こちらは state stack を
    /// 再走査しない。探索ホットパスで snapshot 判定が必要な
    /// ときはこちらを使う。
    #[must_use]
    pub fn cached_repetition_state_with_ply(&self, ply: usize) -> RepetitionState {
        self.cached_repetition_state_with_found_ply_impl(ply)
            .map_or(RepetitionState::None, |(state, _)| state)
    }

    /// 千日手の詳細な状態を判定
    ///
    /// 同一局面の繰り返しを検出し、連続王手の千日手や優等/劣等局面も判定する。
    #[must_use]
    pub fn repetition_state(&self) -> RepetitionState {
        let stack = self.state_stack();
        let current_state = stack.hot(self.st_index);
        let repetition_type = current_state.repetition_type;

        if matches!(repetition_type, RepetitionState::Superior | RepetitionState::Inferior) {
            return repetition_type;
        }

        if current_state.repetition_counter >= 3 {
            return repetition_type;
        }

        RepetitionState::None
    }
    // ANCHOR_END: position_is_repetition

    /// 千日手判定で遡る最大手数を設定する。
    pub fn set_max_repetition_ply(ply: Ply) {
        MAX_REPETITION_PLY.store(ply, Ordering::Relaxed);
    }

    /// 探索ワーカー向けに、直近の反復判定履歴だけを保持した clone を返す。
    ///
    /// 全履歴を複製せず、千日手判定・連続王手判定に必要な範囲
    /// `min(max_repetition_ply, plies_from_null)` だけを残す。
    /// 返される局面の盤面・手駒・手番・ハッシュ・直近 state は元局面と等価で、
    /// `state_stack` だけが search 向けに切り詰められる。
    #[must_use]
    pub fn clone_for_search(&self) -> Self {
        let head = self.st_index;
        let current = self.current_hot();
        let history_limit = usize::from(current.plies_from_null.min(max_repetition_ply()));
        let keep_from = head.saturating_sub(history_limit);

        let replay_moves: Vec<_> = if keep_from < head {
            ((keep_from + 1)..=head).map(|idx| self.state_stack().cold(idx).last_move()).collect()
        } else {
            Vec::new()
        };

        let mut ancestor = self.clone();
        for &mv in replay_moves.iter().rev() {
            ancestor
                .undo_move32(mv)
                .expect("clone_for_search should rewind retained history safely");
        }

        let ancestor_state = crate::board::generate_position_state(&ancestor);
        let mut search = crate::board::position_from_position_state(&ancestor_state);
        search.set_entering_king_rule(self.entering_king_rule);

        for mv in replay_moves {
            search.apply_move32(mv);
        }

        search
    }

    // ANCHOR: position_declare_win
    /// 宣言勝ち判定
    #[must_use]
    pub fn declaration_win_move(&self) -> Move32 {
        match self.entering_king_rule {
            EnteringKingRule::None | EnteringKingRule::Unset => {
                return MOVE_NONE;
            }
            EnteringKingRule::TryRule => {
                let us = self.turn();
                let king_try_sq = if us == Color::BLACK { SQ_51 } else { SQ_59 };
                let king_sq = self.king_square(us);
                if king_sq.is_none() {
                    return MOVE_NONE;
                }

                if !KING_ATTACKS[king_sq].test(king_try_sq) {
                    return MOVE_NONE;
                }
                if self.bitboards().color_pieces(us).test(king_try_sq) {
                    return MOVE_NONE;
                }
                if self.is_attacked_by_color_with_king(us.flip(), king_try_sq, king_sq) {
                    return MOVE_NONE;
                }

                return Move32::normal(
                    king_sq,
                    king_try_sq,
                    Piece::from_parts(us, PieceType::KING),
                );
            }
            EnteringKingRule::Point24
            | EnteringKingRule::Point24Handicap
            | EnteringKingRule::Point27
            | EnteringKingRule::Point27Handicap => {}
        }

        let us = self.turn();

        // 条件5: 玉に王手がかかっていない
        if !self.checkers().is_empty() {
            return MOVE_NONE;
        }

        // 条件1: 宣言側の手番である（自明）

        // 玉の位置を取得
        let king_sq = self.king_square(us);
        if king_sq.is_none() {
            return MOVE_NONE; // 玉がない場合（異常な局面）
        }

        // 条件2: 宣言側の玉が敵陣三段目以内に入っている
        // 先手（BLACK）の敵陣 = 後手の陣地1-3段目 = rank 0-2（内部表現）
        // 後手（WHITE）の敵陣 = 先手の陣地1-3段目 = rank 6-8（内部表現）
        let king_rank = king_sq.rank();
        let in_enemy_camp = match us {
            Color::BLACK => king_rank.raw() <= 2, // 後手の1-3段目（rank 0-2）
            Color::WHITE => king_rank.raw() >= 6, // 先手の1-3段目（rank 6-8）
        };

        if !in_enemy_camp {
            return MOVE_NONE;
        }

        // 条件3と4: 持点計算と駒数カウント
        let hand = self.hand(us);
        let mut points: i32 = 0;

        // 持ち駒の点数計算（点数のみ、駒数は敵陣の駒だけ）
        let hand_pieces = [
            (HandPiece::ROOK, PieceType::ROOK, 5),
            (HandPiece::BISHOP, PieceType::BISHOP, 5),
            (HandPiece::GOLD, PieceType::GOLD, 1),
            (HandPiece::SILVER, PieceType::SILVER, 1),
            (HandPiece::KNIGHT, PieceType::KNIGHT, 1),
            (HandPiece::LANCE, PieceType::LANCE, 1),
            (HandPiece::PAWN, PieceType::PAWN, 1),
        ];

        for (hand_piece, _, value) in hand_pieces {
            let count = i32::try_from(hand.count(hand_piece)).expect("hand count fits in i32");
            points += count * value;
            // 持ち駒は点数に加算するが、駒数には加算しない
        }

        // 盤上の敵陣三段目以内の駒をカウント（玉を除く）
        // 先手（BLACK）の敵陣 = 後手の陣地1-3段目 = rank 0-2（内部表現）
        // 後手（WHITE）の敵陣 = 先手の陣地1-3段目 = rank 6-8（内部表現）
        let enemy_camp_mask = match us {
            Color::BLACK => {
                // 後手の1-3段目（rank 0-2）
                Bitboard::rank_mask(Rank::new(0))
                    | Bitboard::rank_mask(Rank::new(1))
                    | Bitboard::rank_mask(Rank::new(2))
            }
            Color::WHITE => {
                // 先手の1-3段目（rank 6-8）
                Bitboard::rank_mask(Rank::new(6))
                    | Bitboard::rank_mask(Rank::new(7))
                    | Bitboard::rank_mask(Rank::new(8))
            }
        };

        let our_pieces_in_camp = self.bitboards().color_pieces(us) & enemy_camp_mask;
        let p1 = i32::try_from(our_pieces_in_camp.count()).expect("camp piece count fits in i32");
        if p1 < 11 {
            return MOVE_NONE;
        }

        let majors_in_camp = (self.bitboards().pieces_for(PieceType::BISHOP, us)
            | self.bitboards().pieces_for(PieceType::HORSE, us)
            | self.bitboards().pieces_for(PieceType::ROOK, us)
            | self.bitboards().pieces_for(PieceType::DRAGON, us))
            & enemy_camp_mask;
        let p2 = i32::try_from(majors_in_camp.count()).expect("camp major count fits in i32");

        points += p1 + p2 * 4 - 1;

        let required_points = self.entering_king_point[us.to_index()];
        if points >= required_points { MOVE_WIN } else { MOVE_NONE }
    }
    // ANCHOR_END: position_declare_win

    pub fn set_entering_king_rule(&mut self, rule: EnteringKingRule) {
        self.entering_king_rule = rule;
        self.update_entering_point();
    }

    pub(super) fn update_entering_point(&mut self) {
        let mut points = [0i32; Color::COUNT];

        match self.entering_king_rule {
            EnteringKingRule::Point24 | EnteringKingRule::Point24Handicap => {
                points[Color::BLACK.to_index()] = 31;
                points[Color::WHITE.to_index()] = 31;
            }
            EnteringKingRule::Point27 | EnteringKingRule::Point27Handicap => {
                points[Color::BLACK.to_index()] = 28;
                points[Color::WHITE.to_index()] = 27;
            }
            EnteringKingRule::Unset | EnteringKingRule::None | EnteringKingRule::TryRule => {
                return;
            }
        }

        let p1 =
            i32::try_from(self.bitboards().occupied().count()).expect("piece count fits in i32");
        let majors = self.bitboards().pieces(PieceType::BISHOP)
            | self.bitboards().pieces(PieceType::HORSE)
            | self.bitboards().pieces(PieceType::ROOK)
            | self.bitboards().pieces(PieceType::DRAGON);
        let p2 = i32::try_from(majors.count()).expect("major count fits in i32");

        let mut total = p1 + p2 * 4;

        for color in [Color::BLACK, Color::WHITE] {
            let hand = self.hand(color);
            total += i32::try_from(hand.count(HandPiece::PAWN)).expect("pawn count fits in i32");
            total += i32::try_from(hand.count(HandPiece::LANCE)).expect("lance count fits in i32");
            total +=
                i32::try_from(hand.count(HandPiece::KNIGHT)).expect("knight count fits in i32");
            total +=
                i32::try_from(hand.count(HandPiece::SILVER)).expect("silver count fits in i32");
            total += i32::try_from(hand.count(HandPiece::GOLD)).expect("gold count fits in i32");
            total +=
                i32::try_from(hand.count(HandPiece::BISHOP)).expect("bishop count fits in i32") * 5;
            total +=
                i32::try_from(hand.count(HandPiece::ROOK)).expect("rook count fits in i32") * 5;
        }

        if total != 56
            && matches!(
                self.entering_king_rule,
                EnteringKingRule::Point24Handicap | EnteringKingRule::Point27Handicap
            )
        {
            points[Color::WHITE.to_index()] -= 56 - total;
        }

        self.entering_king_point = points;
    }

    /// 現局面で指し手がないかをテストする
    #[must_use]
    pub fn is_mated(&self) -> bool {
        use crate::board::MoveList;
        use crate::board::movegen::{Legal, generate_moves};

        let mut list = MoveList::new();
        generate_moves::<Legal>(self, &mut list);
        list.is_empty()
    }

    /// 入玉宣言の条件を構造化して評価する
    ///
    /// [`declaration_win_move()`](Self::declaration_win_move) が「宣言できるか否か」
    /// を返すのに対し、こちらは各条件の充足状況を
    /// [`DeclarationEvaluation`] として返す。
    ///
    /// エディタや GUI で「何が足りないか」を表示する用途に適する。
    ///
    /// # Examples
    ///
    /// ```
    /// use rsshogi::board::{self, position_from_sfen};
    /// use rsshogi::types::EnteringKingRule;
    ///
    /// board::init();
    /// let mut pos = position_from_sfen(
    ///     "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"
    /// ).unwrap();
    /// pos.set_entering_king_rule(EnteringKingRule::Point27);
    ///
    /// let eval = pos.evaluate_declaration();
    /// assert!(!eval.can_declare);
    /// assert_eq!(eval.rule, EnteringKingRule::Point27);
    /// ```
    #[must_use]
    pub fn evaluate_declaration(&self) -> DeclarationEvaluation {
        match self.entering_king_rule {
            EnteringKingRule::None | EnteringKingRule::Unset => DeclarationEvaluation {
                rule: self.entering_king_rule,
                can_declare: false,
                detail: DeclarationDetail::NoRule,
            },
            EnteringKingRule::TryRule => self.evaluate_try_rule(),
            EnteringKingRule::Point24
            | EnteringKingRule::Point24Handicap
            | EnteringKingRule::Point27
            | EnteringKingRule::Point27Handicap => self.evaluate_point_rule(),
        }
    }

    fn evaluate_try_rule(&self) -> DeclarationEvaluation {
        let us = self.turn();
        let try_square = if us == Color::BLACK { SQ_51 } else { SQ_59 };
        let king_sq = self.king_square(us);

        if king_sq.is_none() {
            return DeclarationEvaluation {
                rule: EnteringKingRule::TryRule,
                can_declare: false,
                detail: DeclarationDetail::TryRule {
                    king_square: None,
                    try_square,
                    can_reach: false,
                    can_occupy: true,
                    is_safe: true,
                },
            };
        }

        let can_reach = KING_ATTACKS[king_sq].test(try_square);
        let can_occupy = !self.bitboards().color_pieces(us).test(try_square);
        let is_safe = !self.is_attacked_by_color_with_king(us.flip(), try_square, king_sq);

        let can_declare = can_reach && can_occupy && is_safe;

        DeclarationEvaluation {
            rule: EnteringKingRule::TryRule,
            can_declare,
            detail: DeclarationDetail::TryRule {
                king_square: Some(king_sq),
                try_square,
                can_reach,
                can_occupy,
                is_safe,
            },
        }
    }

    fn evaluate_point_rule(&self) -> DeclarationEvaluation {
        let us = self.turn();
        let rule = self.entering_king_rule;

        // 条件5: 王手されていないか
        let not_in_check = self.checkers().is_empty();

        // 玉の位置
        let king_sq = self.king_square(us);
        if king_sq.is_none() {
            return DeclarationEvaluation {
                rule,
                can_declare: false,
                detail: DeclarationDetail::PointRule {
                    not_in_check,
                    king_in_enemy_camp: false,
                    pieces_in_camp: 0,
                    enough_pieces: false,
                    points: 0,
                    required_points: self.entering_king_point[us.to_index()],
                },
            };
        }

        // 条件2: 玉が敵陣三段目以内にあるか
        let king_rank = king_sq.rank();
        let king_in_enemy_camp = match us {
            Color::BLACK => king_rank.raw() <= 2,
            Color::WHITE => king_rank.raw() >= 6,
        };

        // 敵陣マスク
        let enemy_camp_mask = match us {
            Color::BLACK => {
                Bitboard::rank_mask(Rank::new(0))
                    | Bitboard::rank_mask(Rank::new(1))
                    | Bitboard::rank_mask(Rank::new(2))
            }
            Color::WHITE => {
                Bitboard::rank_mask(Rank::new(6))
                    | Bitboard::rank_mask(Rank::new(7))
                    | Bitboard::rank_mask(Rank::new(8))
            }
        };

        // 敵陣にある自駒数
        let our_pieces_in_camp = self.bitboards().color_pieces(us) & enemy_camp_mask;
        let pieces_in_camp =
            i32::try_from(our_pieces_in_camp.count()).expect("camp piece count fits in i32");
        let enough_pieces = pieces_in_camp >= 11;

        // 持点計算
        let hand = self.hand(us);
        let mut points: i32 = 0;

        let hand_pieces = [
            (HandPiece::ROOK, 5),
            (HandPiece::BISHOP, 5),
            (HandPiece::GOLD, 1),
            (HandPiece::SILVER, 1),
            (HandPiece::KNIGHT, 1),
            (HandPiece::LANCE, 1),
            (HandPiece::PAWN, 1),
        ];

        for (hand_piece, value) in hand_pieces {
            let count = i32::try_from(hand.count(hand_piece)).expect("hand count fits in i32");
            points += count * value;
        }

        // 盤上の敵陣にある大駒
        let majors_in_camp = (self.bitboards().pieces_for(PieceType::BISHOP, us)
            | self.bitboards().pieces_for(PieceType::HORSE, us)
            | self.bitboards().pieces_for(PieceType::ROOK, us)
            | self.bitboards().pieces_for(PieceType::DRAGON, us))
            & enemy_camp_mask;
        let p2 = i32::try_from(majors_in_camp.count()).expect("camp major count fits in i32");

        points += pieces_in_camp + p2 * 4;
        if king_in_enemy_camp {
            points -= 1;
        }

        let required_points = self.entering_king_point[us.to_index()];
        let can_declare =
            not_in_check && king_in_enemy_camp && enough_pieces && points >= required_points;

        DeclarationEvaluation {
            rule,
            can_declare,
            detail: DeclarationDetail::PointRule {
                not_in_check,
                king_in_enemy_camp,
                pieces_in_camp,
                enough_pieces,
                points,
                required_points,
            },
        }
    }
}

impl Position {
    #[inline]
    fn cached_repetition_state_with_found_ply_impl(
        &self,
        ply: usize,
    ) -> Option<(RepetitionState, usize)> {
        let current_state = self.current_hot();
        let repetition_distance = current_state.repetition_distance;
        let ply_i32 = i32::try_from(ply).unwrap_or(i32::MAX);

        if repetition_distance == 0 || repetition_distance >= ply_i32 {
            return None;
        }

        let found = usize::try_from(repetition_distance.unsigned_abs()).unwrap_or(usize::MAX);
        Some((current_state.repetition_type, found))
    }
}
