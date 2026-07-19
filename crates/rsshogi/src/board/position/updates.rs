use super::Position;
use super::types::{CapturedPieceDelta, MoveApplyFacts, MoveDelta32, MoveError};
use crate::board::state_info::{
    MoveStateWriteBack, NullStateWriteBack, StateColdPayload, StateHotComputed, TacticalCache,
    UndoPayload,
};
use crate::board::zobrist::Zobrist;
use crate::types::{Bitboard, Color, HandPiece, Move, Move32, MoveBase, Piece, PieceType, Square};

#[inline]
fn direct_step_from_king(king_sq: Square, from: Square) -> Option<(i8, i8)> {
    let king_raw = king_sq.raw();
    let from_raw = from.raw();
    if king_raw < 0 || from_raw < 0 {
        return None;
    }

    let king_file = king_raw / 9;
    let king_rank = king_raw % 9;
    let from_file = from_raw / 9;
    let from_rank = from_raw % 9;
    let df = from_file - king_file;
    let dr = from_rank - king_rank;

    if df == 0 && dr != 0 {
        return Some((0, dr.signum()));
    }
    if dr == 0 && df != 0 {
        return Some((df.signum(), 0));
    }
    if df.abs() == dr.abs() && df != 0 {
        return Some((df.signum(), dr.signum()));
    }

    None
}

#[inline]
fn direct_attacks_from(from: Square, file_step: i8, rank_step: i8, occupied: Bitboard) -> Bitboard {
    let from_raw = from.raw();
    if from_raw < 0 {
        return Bitboard::EMPTY;
    }

    let mut file = from_raw / 9 + file_step;
    let mut rank = from_raw % 9 + rank_step;
    let mut attacks = Bitboard::EMPTY;

    while (0..=8).contains(&file) && (0..=8).contains(&rank) {
        let sq = Square::new(file * 9 + rank);
        attacks.set(sq);
        if occupied.test(sq) {
            break;
        }
        file += file_step;
        rank += rank_step;
    }

    attacks
}

#[inline]
fn discovered_direct_attacks(from: Square, king_sq: Square, occupied: Bitboard) -> Bitboard {
    let Some((file_step, rank_step)) = direct_step_from_king(king_sq, from) else {
        return Bitboard::EMPTY;
    };
    direct_attacks_from(from, file_step, rank_step, occupied)
}

#[inline]
const fn us_color<const BLACK: bool>() -> Color {
    if BLACK { Color::BLACK } else { Color::WHITE }
}

#[inline]
const fn them_color<const BLACK: bool>() -> Color {
    if BLACK { Color::WHITE } else { Color::BLACK }
}

impl Position {
    /// `Move` 指し手を局面依存の `Move32` に展開して適用する。
    ///
    /// `move32_from_move` で完全な `Move32`（駒情報付き）に変換してから `apply_move32` に委譲する。
    /// `StateInfo.last_move` に完全な `Move32` が格納されることを保証する。
    pub fn apply_move(&mut self, mv: Move) {
        let mv32 = self.move32_from_move(mv);
        self.apply_move32(mv32);
    }

    /// 指し手を適用
    #[allow(clippy::too_many_lines, clippy::cast_possible_truncation, clippy::cognitive_complexity)]
    pub fn apply_move32(&mut self, mv: Move32) {
        debug_assert!(self.is_legal_move32(mv), "apply_move32 expects a legal move");
        let gives_check = self.gives_check_move32(mv);
        self.apply_move32_with_gives_check(mv, gives_check);
    }

    /// 指し手を適用（王手判定を外部で計算済みの場合）
    #[inline]
    pub fn apply_move32_with_gives_check(&mut self, mv: Move32, gives_check: bool) {
        debug_assert!(self.is_legal_move32(mv), "apply_move32 expects a legal move");
        match self.turn() {
            Color::BLACK => {
                let _ = self.apply_move32_with_gives_check_for::<true, false>(mv, gives_check);
            }
            Color::WHITE => {
                let _ = self.apply_move32_with_gives_check_for::<false, false>(mv, gives_check);
            }
        }
    }

    /// 指し手を適用し、eval 非依存の盤面差分を返す。
    #[inline]
    #[must_use]
    pub fn apply_move32_with_delta(&mut self, mv: Move32, gives_check: bool) -> MoveDelta32 {
        self.apply_move32_with_facts(mv, gives_check).delta
    }

    /// 指し手を適用し、探索・評価が共有する適用 facts を返す。
    #[inline]
    #[must_use]
    pub fn apply_move32_with_facts(&mut self, mv: Move32, gives_check: bool) -> MoveApplyFacts {
        debug_assert!(self.is_legal_move32(mv), "apply_move32 expects a legal move");
        match self.turn() {
            Color::BLACK => self
                .apply_move32_with_gives_check_for::<true, true>(mv, gives_check)
                .expect("delta must be present when requested"),
            Color::WHITE => self
                .apply_move32_with_gives_check_for::<false, true>(mv, gives_check)
                .expect("delta must be present when requested"),
        }
    }

    /// allocation なしで利用できる state slot に探索手を適用する。
    ///
    /// # Errors
    ///
    /// allocation なしで利用できる次の物理 slot がない場合は、盤面を変更せず
    /// [`MoveError::StateCapacityExceeded`] を返す。
    ///
    /// `mv` は合法手、`gives_check` はその手の王手判定結果でなければならない。
    #[inline]
    pub fn try_apply_search_move32_with_facts(
        &mut self,
        mv: Move32,
        gives_check: bool,
    ) -> Result<MoveApplyFacts, MoveError> {
        if !self.state_stack().has_prepared_next() {
            return Err(MoveError::StateCapacityExceeded);
        }
        Ok(self.apply_move32_with_facts(mv, gives_check))
    }

    #[inline]
    #[allow(clippy::too_many_lines, clippy::cast_possible_truncation, clippy::cognitive_complexity)]
    fn apply_move32_with_gives_check_for<const BLACK: bool, const WITH_DELTA: bool>(
        &mut self,
        mv: Move32,
        gives_check: bool,
    ) -> Option<MoveApplyFacts> {
        debug_assert!(self.is_legal_move32(mv), "apply_move32 expects a legal move");
        let us = us_color::<BLACK>();
        let them = them_color::<BLACK>();
        let mut board_key = self.board_key;
        let mut hand_key = self.hand_key;

        let (
            prev_continuous_check,
            prev_plies_from_null,
            prev_check_square_for_moved,
            prev_blockers_for_them,
            mut partial_keys,
        ) = {
            let current = self.current_hot();
            (
                current.continuous_check,
                current.plies_from_null,
                if gives_check {
                    current.check_square(mv.piece_after_move().piece_type())
                } else {
                    Bitboard::EMPTY
                },
                if gives_check { current.blockers_for_king(them) } else { Bitboard::EMPTY },
                current.partial_keys(),
            )
        };

        let state_idx = self.state_stack_mut().prepare_next_for_move();

        let mut captured = Piece::NONE;
        let move_delta = if mv.is_drop() {
            // 駒打ち処理
            let to = mv.to_sq();
            let dropped_piece = mv.dropped_piece();
            debug_assert!(dropped_piece.is_some(), "drop move must include a dropped piece type");
            // SAFETY: legal drop では dropped_piece が必ず設定される。
            let piece_type = unsafe { dropped_piece.unwrap_unchecked() };
            let color = us;

            debug_assert!(self.board.get(to).is_empty(), "drop destination must be empty");
            let hand_piece_opt = HandPiece::from_piece_type(piece_type);
            debug_assert!(hand_piece_opt.is_some(), "drop move must use a hand piece type");
            // SAFETY: 打ち駒に使える PieceType のみがここに到達する。
            let hand_piece = unsafe { hand_piece_opt.unwrap_unchecked() };
            let hand_count_before = self.hand(color).count(hand_piece);
            debug_assert!(hand_count_before > 0, "drop move must have a matching hand piece");

            // 駒を配置
            let piece = Piece::from_parts(color, piece_type);
            self.board.set(to, piece);

            // ビットボードを更新
            // SAFETY: 打ち駒は常に盤上駒（NONE ではない）。
            unsafe { self.bitboards.set_no_refresh_unchecked(to, piece) };

            // 持ち駒を減らす
            self.hand_mut(color).sub(hand_piece, 1);

            // Zobrist更新: 駒打ち
            board_key ^= Zobrist::psq(to, piece);
            hand_key.sub(Zobrist::hand(color, piece_type, 1));
            Self::add_board_piece_to_partial_keys(&mut partial_keys, to, piece);
            Self::sub_hand_piece_material_value(&mut partial_keys, color, piece_type, 1);
            (
                piece_type,
                if WITH_DELTA {
                    Some(MoveDelta32::drop(color, piece_type, to, hand_count_before))
                } else {
                    None
                },
            )
        } else {
            // 通常移動または成り
            let from = mv.from_sq();
            let to = mv.to_sq();

            // 移動元の駒を取得
            let moved_piece = self.board.get(from);
            debug_assert!(!moved_piece.is_empty(), "move must originate from a piece");
            debug_assert!(moved_piece.color() == us, "move must be from the side to move");
            let new_piece = if mv.is_promotion() { moved_piece.promote() } else { moved_piece };
            let mut captured_delta = None;

            // 移動先の駒を取得（捕獲判定）
            let captured_piece = self.board.get(to);
            if !captured_piece.is_empty() {
                debug_assert!(captured_piece.color() != us, "capture must target opponent piece");
                captured = captured_piece;

                // ビットボードから削除
                // SAFETY: captured_piece は empty でないことを直前で確認済み。
                unsafe { self.bitboards.clear_no_refresh_unchecked(to, captured_piece) };

                // 持ち駒に追加
                let captured_hand_piece_opt =
                    HandPiece::from_piece_type(captured_piece.piece_type().demote());
                debug_assert!(
                    captured_hand_piece_opt.is_some(),
                    "captured piece must map to hand piece"
                );
                // SAFETY: 捕獲駒の成り戻しは必ず持ち駒種に対応する。
                let captured_hand_piece = unsafe { captured_hand_piece_opt.unwrap_unchecked() };
                let captured_hand_count_before = self.hand(us).count(captured_hand_piece);
                if WITH_DELTA {
                    captured_delta = Some(CapturedPieceDelta {
                        piece: captured_piece,
                        hand_count_before: captured_hand_count_before,
                    });
                }
                self.hand_mut(us).add(captured_hand_piece, 1);

                // Zobrist: 捕獲された駒を盤上から削除
                board_key ^= Zobrist::psq(to, captured_piece);
                // Zobrist: 持ち駒を更新
                hand_key.add(Zobrist::hand(us, captured_piece.piece_type().demote(), 1));
                Self::remove_board_piece_from_partial_keys(&mut partial_keys, to, captured_piece);
                Self::add_hand_piece_material_value(
                    &mut partial_keys,
                    us,
                    captured_piece.piece_type().demote(),
                    1,
                );
            }

            // 移動元から駒を取り除く
            self.board.set(from, Piece::NONE);
            // SAFETY: moved_piece は empty でないことを直前で確認済み。
            unsafe { self.bitboards.clear_no_refresh_unchecked(from, moved_piece) };

            // 移動先に駒を配置
            self.board.set(to, new_piece);
            // SAFETY: 移動後の駒は常に盤上駒（NONE ではない）。
            unsafe { self.bitboards.set_no_refresh_unchecked(to, new_piece) };

            if moved_piece.piece_type() == PieceType::KING {
                self.set_king_square(us, to);
            }

            // Zobrist更新: 通常移動または成り
            board_key ^= Zobrist::psq(from, moved_piece);
            board_key ^= Zobrist::psq(to, new_piece);
            Self::remove_board_piece_from_partial_keys(&mut partial_keys, from, moved_piece);
            Self::add_board_piece_to_partial_keys(&mut partial_keys, to, new_piece);

            (
                moved_piece.piece_type(),
                if WITH_DELTA {
                    Some(MoveDelta32::board(us, from, to, moved_piece, new_piece, captured_delta))
                } else {
                    None
                },
            )
        };
        let last_moved_piece_type = move_delta.0;
        let move_delta = move_delta.1;
        self.bitboards.refresh_derived();

        // 手番を反転し、手数を進め、Zobristを更新
        self.flip_side_to_move();
        self.ply += 1;
        board_key ^= Zobrist::side();
        self.board_key = board_key;
        self.hand_key = hand_key;
        self.zobrist = board_key ^ hand_key;

        // continuous_check と plies_from_null の更新
        let moved_color = us;
        let mut new_continuous_check = prev_continuous_check;
        let moved_idx = moved_color.to_index();
        new_continuous_check[moved_idx] =
            if gives_check { prev_continuous_check[moved_idx] + 2 } else { 0 };
        let new_plies_from_null = prev_plies_from_null + 1;
        let hand_them = self.hand(them);

        let mut repetition_counter = 0;
        let mut repetition_distance = 0;
        let mut repetition_times = 0;
        let mut repetition_type = crate::types::RepetitionState::None;
        if new_plies_from_null >= 4 {
            self.compute_repetition_info_in_place(
                self.state_stack(),
                state_idx,
                new_plies_from_null,
                hand_them,
                new_continuous_check,
                board_key,
                &mut repetition_counter,
                &mut repetition_distance,
                &mut repetition_times,
                &mut repetition_type,
            );
        }

        let (black_blockers, black_pinners) = self.compute_slider_info_for::<true>();
        let (white_blockers, white_pinners) = self.compute_slider_info_for::<false>();
        let check_squares = if BLACK {
            // 先手の手を適用した後は後手番。
            self.compute_check_squares_for::<false>()
        } else {
            // 後手の手を適用した後は先手番。
            self.compute_check_squares_for::<true>()
        };
        let checkers = if gives_check {
            // - 直接王手は prev.check_squares から差分更新する
            // - 開き王手は directEffect(from, direct_of(ksq, from), occupied) 相当で差分合成する
            let mut checkers = if prev_check_square_for_moved.test(mv.to_sq()) {
                Bitboard::from_square(mv.to_sq())
            } else {
                Bitboard::EMPTY
            };

            if !mv.is_drop() {
                let from = mv.from_sq();
                let king_sq = self.king_square(them);
                let is_discovered = !king_sq.is_none()
                    && prev_blockers_for_them.test(from)
                    && !Bitboard::is_aligned(from, mv.to_sq(), king_sq);

                if is_discovered {
                    let occupied = self.bitboards().occupied();
                    let discovered = discovered_direct_attacks(from, king_sq, occupied)
                        & self.bitboards().color_pieces(us);
                    checkers |= discovered;
                }
            }

            debug_assert_eq!(checkers, self.compute_checkers_for(them));
            checkers
        } else {
            Bitboard::EMPTY
        };

        let facts = move_delta.map(|delta| {
            MoveApplyFacts::from_delta(
                delta,
                mv,
                gives_check,
                board_key,
                hand_key,
                self.zobrist,
                partial_keys,
            )
        });

        self.state_stack_mut().write_move_state(
            state_idx,
            MoveStateWriteBack {
                hot: StateHotComputed {
                    plies_from_null: new_plies_from_null,
                    board_key,
                    hand_key,
                    partial_keys,
                    hand: hand_them,
                    continuous_check: new_continuous_check,
                    tactical: TacticalCache::new(
                        checkers,
                        [black_pinners, white_pinners],
                        [black_blockers, white_blockers],
                        check_squares,
                    ),
                    repetition_counter,
                    repetition_distance,
                    repetition_times,
                    repetition_type,
                },
                cold: StateColdPayload {
                    undo: UndoPayload::new(captured, mv, last_moved_piece_type),
                },
            },
        );
        self.st_index = state_idx;
        self.debug_assert_partial_keys_consistent();
        facts
    }

    /// 指し手を巻き戻す（`Move` 版）。
    #[inline]
    pub fn undo_move(&mut self, mv: Move) -> Result<(), MoveError> {
        self.undo_move_impl(mv)
    }

    /// 指し手を巻き戻す（`Move32` 版）。
    #[inline]
    pub fn undo_move32(&mut self, mv: Move32) -> Result<(), MoveError> {
        self.undo_move_impl(mv)
    }

    /// 指し手を巻き戻し、元の `do_move` を表す順方向 delta を返す。
    #[inline]
    pub fn undo_move32_with_delta(&mut self, mv: Move32) -> Result<MoveDelta32, MoveError> {
        self.undo_move_impl_with_delta::<true>(mv.to_move())
            .map(|delta| delta.expect("delta must be present when requested"))
    }

    /// エラーチェックなしの高速 undo。perft 等の内部ループ用。
    ///
    /// # Safety
    /// - StateStack が空でないこと
    /// - mv が直前に `apply_move32` で適用した手であること
    #[inline]
    pub unsafe fn undo_move32_unchecked(&mut self, mv: Move32) {
        let m = mv.to_move();
        let (captured, current_keys, current_index) = {
            let stack = &mut self.state_stack;
            // SAFETY: 呼び出し側がステートスタックを空でないことと、`mv` が直前に適用した手であることを保証するため、ポップにより対応する前の状態が復元される。
            let prev_idx = unsafe { stack.pop_unchecked() };
            let current_index = stack.current_index();
            let captured = stack.cold(prev_idx).captured_piece();
            let current = stack.hot(current_index);
            (captured, (current.board_key, current.hand_key), current_index)
        };
        self.st_index = current_index;
        self.board_key = current_keys.0;
        self.hand_key = current_keys.1;
        self.zobrist = current_keys.0 ^ current_keys.1;

        self.flip_side_to_move();
        self.ply -= 1;

        if m.is_drop() {
            let to = m.to_sq();
            // SAFETY: 呼び出し側が `mv` を直前に適用した手であることを保証するため、駒打ちの場合はエンコードされた打ち駒種が必ず存在し有効である。
            let piece_type = unsafe { m.dropped_piece().unwrap_unchecked() };
            let color = self.turn();
            // SAFETY: 合法な駒打ちは持ち駒種に限定される。
            let hand_piece = unsafe { HandPiece::from_piece_type(piece_type).unwrap_unchecked() };

            self.board.set(to, Piece::NONE);
            // SAFETY: 駒打ちを戻す駒は常に盤上駒（NONE ではない）。
            unsafe {
                self.bitboards.clear_no_refresh_unchecked(to, Piece::from_parts(color, piece_type));
            }
            self.hand_mut(color).add(hand_piece, 1);
        } else {
            let from = m.from_sq();
            let to = m.to_sq();
            let current_piece = self.board.get(to);

            let original_piece = if m.is_promotion() {
                Piece::from_parts(current_piece.color(), current_piece.piece_type().demote())
            } else {
                current_piece
            };

            self.board.set(to, Piece::NONE);
            // SAFETY: current_piece は直前に to から取得した盤上駒。
            unsafe { self.bitboards.clear_no_refresh_unchecked(to, current_piece) };
            self.board.set(from, original_piece);
            // SAFETY: original_piece は盤上駒（NONE ではない）。
            unsafe { self.bitboards.set_no_refresh_unchecked(from, original_piece) };

            let color = current_piece.color();
            if original_piece.piece_type() == PieceType::KING {
                self.set_king_square(color, from);
            }

            if !captured.is_empty() {
                self.board.set(to, captured);
                // SAFETY: captured は empty でないことを if 条件で保証。
                unsafe { self.bitboards.set_no_refresh_unchecked(to, captured) };
                // SAFETY: 捕獲された盤上の駒は常に成り戻しにより持ち駒種に対応する。
                let hand_piece = unsafe {
                    HandPiece::from_piece_type(captured.piece_type().demote()).unwrap_unchecked()
                };
                self.hand_mut(self.turn()).sub(hand_piece, 1);
            }
        }
        self.bitboards.refresh_derived();
        self.debug_assert_partial_keys_consistent();
    }

    #[inline]
    fn undo_move_impl(&mut self, mv: impl MoveBase) -> Result<(), MoveError> {
        self.undo_move_impl_with_delta::<false>(mv.to_move()).map(|_| ())
    }

    #[inline]
    fn undo_move_impl_with_delta<const WITH_DELTA: bool>(
        &mut self,
        m: Move,
    ) -> Result<Option<MoveDelta32>, MoveError> {
        // StateStackから前の状態を取得
        let (captured, current_keys, current_index) = {
            let stack = self.state_stack_mut();
            let prev_idx = stack.pop().ok_or(MoveError::StackUnderflow)?;
            let current_index = stack.current_index();
            let captured = stack.cold(prev_idx).captured_piece();
            let current = stack.hot(current_index);
            let keys = (current.board_key, current.hand_key);
            (captured, keys, current_index)
        };
        self.st_index = current_index;
        let move_delta = if WITH_DELTA && m.is_drop() {
            let piece_type = m.dropped_piece().ok_or(MoveError::NoStateInfo)?;
            let mover = self.turn().flip();
            let hand_piece =
                HandPiece::from_piece_type(piece_type).expect("dropped piece must map to hand");
            Some(MoveDelta32::drop(
                mover,
                piece_type,
                m.to_sq(),
                self.hand(mover).count(hand_piece) + 1,
            ))
        } else if WITH_DELTA {
            let mover = self.turn().flip();
            let from = m.from_sq();
            let to = m.to_sq();
            let moved_piece_after = self.board.get(to);
            let moved_piece_before = if m.is_promotion() {
                Piece::from_parts(
                    moved_piece_after.color(),
                    moved_piece_after.piece_type().demote(),
                )
            } else {
                moved_piece_after
            };
            let captured_delta = if captured.is_empty() {
                None
            } else {
                let hand_piece = HandPiece::from_piece_type(captured.piece_type().demote())
                    .expect("captured piece must map to hand");
                Some(CapturedPieceDelta {
                    piece: captured,
                    hand_count_before: self.hand(mover).count(hand_piece).saturating_sub(1),
                })
            };
            Some(MoveDelta32::board(
                mover,
                from,
                to,
                moved_piece_before,
                moved_piece_after,
                captured_delta,
            ))
        } else {
            None
        };

        // Zobristを復元（現局面のStateから復元）
        self.board_key = current_keys.0;
        self.hand_key = current_keys.1;
        self.zobrist = self.board_key ^ self.hand_key;

        // 手番を反転（元に戻す）
        self.flip_side_to_move();
        self.ply -= 1;

        if m.is_drop() {
            // 駒打ちの巻き戻し
            let to = m.to_sq();
            let piece_type = m.dropped_piece().ok_or(MoveError::NoStateInfo)?;
            let color = self.turn();
            let hand_piece =
                HandPiece::from_piece_type(piece_type).expect("dropped piece must map to hand");

            // 打った駒を盤面から取り除く
            self.board.set(to, Piece::NONE);
            // SAFETY: 駒打ちを戻す駒は常に盤上駒（NONE ではない）。
            unsafe {
                self.bitboards.clear_no_refresh_unchecked(to, Piece::from_parts(color, piece_type));
            };

            // 持ち駒に戻す
            self.hand_mut(color).add(hand_piece, 1);
        } else {
            // 通常移動または成りの巻き戻し
            let from = m.from_sq();
            let to = m.to_sq();

            // 移動先の駒を取得（現在の駒）
            let current_piece = self.board.get(to);

            // 移動元の駒を復元（成りの場合は元に戻す）
            let original_piece = if m.is_promotion() {
                Piece::from_parts(current_piece.color(), current_piece.piece_type().demote())
            } else {
                current_piece
            };

            // 移動先から駒を取り除く
            self.board.set(to, Piece::NONE);
            // SAFETY: current_piece は直前に to から取得した盤上駒。
            unsafe { self.bitboards.clear_no_refresh_unchecked(to, current_piece) };

            // 移動元に駒を戻す
            self.board.set(from, original_piece);
            // SAFETY: original_piece は盤上駒（NONE ではない）。
            unsafe { self.bitboards.set_no_refresh_unchecked(from, original_piece) };

            let color = current_piece.color();
            if original_piece.piece_type() == PieceType::KING {
                self.set_king_square(color, from);
            }

            // 捕獲があった場合、捕獲された駒を復元
            if !captured.is_empty() {
                self.board.set(to, captured);
                // SAFETY: captured は empty でないことを if 条件で保証。
                unsafe { self.bitboards.set_no_refresh_unchecked(to, captured) };

                // 持ち駒から取り除く
                let hand_piece = HandPiece::from_piece_type(captured.piece_type().demote())
                    .expect("captured piece must map to hand");
                self.hand_mut(self.turn()).sub(hand_piece, 1);
            }
        }
        self.bitboards.refresh_derived();
        self.debug_assert_partial_keys_consistent();

        Ok(move_delta)
    }

    /// Null move（手を指さずに手番だけを反転）を適用する。
    ///
    /// 盤面の駒配置や持ち駒は変化させず、Zobrist手番フラグとキャッシュのみ更新する。
    #[inline]
    pub fn apply_null_move(&mut self) -> Result<(), MoveError> {
        self.apply_null_move_impl::<true>()
    }

    /// allocation なしで利用できる state slot に探索用 null move を適用する。
    ///
    /// 通常の null move と異なり `game_ply` を進めない。
    ///
    /// # Errors
    ///
    /// allocation なしで利用できる次の物理 slot がない場合は、局面を変更せず
    /// [`MoveError::StateCapacityExceeded`] を返す。
    #[inline]
    pub fn try_apply_search_null_move(&mut self) -> Result<(), MoveError> {
        if !self.state_stack().has_prepared_next() {
            return Err(MoveError::StateCapacityExceeded);
        }
        self.apply_null_move_impl::<false>()
    }

    #[inline]
    fn apply_null_move_impl<const ADVANCE_GAME_PLY: bool>(&mut self) -> Result<(), MoveError> {
        debug_assert!(self.checkers().is_empty(), "null move must not be in check");

        // 新しい状態を積む
        // `prepare_next_for_null_move` は null move 後も不変なキャッシュだけを引き継ぎ、
        // null move の不変条件で確定するフィールドを初期化する。
        let state_idx = self.state_stack_mut().prepare_next_for_null_move();

        // 手番を反転し、手数を進め、Zobristを更新
        self.flip_side_to_move();
        if ADVANCE_GAME_PLY {
            self.ply += 1;
        }
        self.board_key ^= Zobrist::side();
        self.zobrist = self.board_key ^ self.hand_key;

        let prev_continuous_check = self.state_stack().hot(state_idx).continuous_check;
        debug_assert!(
            prev_continuous_check[self.turn().to_index()] == 0,
            "null move must not follow a continuous check for side to move"
        );
        let prev_side = self.turn().flip();
        let mut new_continuous_check = prev_continuous_check;
        new_continuous_check[prev_side.to_index()] = 0;

        let board_key = self.board_key;
        let hand_key = self.hand_key;
        let turn = self.turn();
        let hand = self.hand(turn);
        let check_squares = self.compute_check_squares();
        self.state_stack_mut().write_null_state_after_turn_flip(
            state_idx,
            NullStateWriteBack {
                board_key,
                hand_key,
                hand,
                continuous_check: new_continuous_check,
                check_squares,
            },
        );
        self.st_index = state_idx;
        self.debug_assert_partial_keys_consistent();
        Ok(())
    }

    /// Null move を巻き戻す。
    #[inline]
    pub fn undo_null_move(&mut self) -> Result<(), MoveError> {
        self.undo_null_move_impl::<true>()
    }

    #[inline]
    fn undo_null_move_impl<const REWIND_GAME_PLY: bool>(&mut self) -> Result<(), MoveError> {
        if self.st_index == 0 || (REWIND_GAME_PLY && self.ply == 0) {
            return Err(MoveError::StackUnderflow);
        }
        self.flip_side_to_move();
        if REWIND_GAME_PLY {
            self.ply -= 1;
        }

        // スタックをポップ
        let (current_keys, current_index) = {
            let stack = self.state_stack_mut();
            let _ = stack.pop().ok_or(MoveError::StackUnderflow)?;
            let current_index = stack.current_index();
            let current = stack.hot(current_index);
            ((current.board_key, current.hand_key), current_index)
        };
        self.st_index = current_index;

        self.board_key = current_keys.0;
        self.hand_key = current_keys.1;
        self.zobrist = self.board_key ^ self.hand_key;
        self.debug_assert_partial_keys_consistent();

        Ok(())
    }

    /// [`Position::try_apply_search_null_move`] を巻き戻す。
    /// 成功した同 API の直後に、対となる操作として呼び出す必要がある。
    ///
    /// # Errors
    ///
    /// 巻き戻せる state がない場合は [`MoveError::StackUnderflow`] を返す。
    #[inline]
    pub fn undo_search_null_move(&mut self) -> Result<(), MoveError> {
        self.undo_null_move_impl::<false>()
    }
}

#[cfg(test)]
mod tests {
    use crate::board::{self, MoveList, NonEvasionsAll, generate_moves, position_from_sfen};

    /// `undo_move32_unchecked` が安全版 `undo_move32` と同一の盤面状態を復元することを検証
    #[test]
    fn undo_move32_unchecked_equivalence() {
        let init = board::hirate_position();
        let mut moves = MoveList::new();
        generate_moves::<NonEvasionsAll>(&init, &mut moves);

        for &mv in moves.iter() {
            let mv32 = init.move32_from_move(mv);
            if !mv32.is_normal() || !init.is_legal_move32(mv32) {
                continue;
            }

            // safe 版
            let mut pos_safe = init.clone();
            pos_safe.apply_move32(mv32);
            pos_safe.undo_move32(mv32).expect("safe undo must succeed");

            // unchecked 版
            let mut pos_unchecked = init.clone();
            pos_unchecked.apply_move32(mv32);
            unsafe { pos_unchecked.undo_move32_unchecked(mv32) };

            assert_eq!(
                pos_safe.to_sfen(None),
                pos_unchecked.to_sfen(None),
                "SFEN mismatch after undo of {mv32:?}"
            );
            assert_eq!(
                pos_safe.zobrist, pos_unchecked.zobrist,
                "Zobrist mismatch after undo of {mv32:?}"
            );
        }
    }

    /// 数手進めた局面（駒取り・成りが含まれる可能性）でも等価性を確認
    #[test]
    fn undo_move32_unchecked_equivalence_midgame() {
        let sfens = [
            "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
            "l6nl/5+P1gk/2np1S3/p1p4Pp/3P2S2/1PPb2P1P/P5GS1/R8/LN4bKL w RGSNLPbsnl3p 1",
            "8l/1l+R2P3/p2pBG1pp/kps1p4/Nn1P2G2/P1P1P2PP/1PS6/1KSG3+r1/LN2+p3L w Sbgn3p 1",
        ];

        for sfen in &sfens {
            let init = position_from_sfen(sfen).expect("valid sfen");
            let mut moves = MoveList::new();
            generate_moves::<NonEvasionsAll>(&init, &mut moves);

            for &mv in moves.iter() {
                let mv32 = init.move32_from_move(mv);
                if !mv32.is_normal() || !init.is_legal_move32(mv32) {
                    continue;
                }

                let mut pos_safe = init.clone();
                pos_safe.apply_move32(mv32);
                pos_safe.undo_move32(mv32).expect("safe undo must succeed");

                let mut pos_unchecked = init.clone();
                pos_unchecked.apply_move32(mv32);
                unsafe { pos_unchecked.undo_move32_unchecked(mv32) };

                assert_eq!(
                    pos_safe.to_sfen(None),
                    pos_unchecked.to_sfen(None),
                    "SFEN mismatch after undo of {mv32:?} in position {sfen}"
                );
                assert_eq!(
                    pos_safe.zobrist, pos_unchecked.zobrist,
                    "Zobrist mismatch after undo of {mv32:?} in position {sfen}"
                );
            }
        }
    }
}
