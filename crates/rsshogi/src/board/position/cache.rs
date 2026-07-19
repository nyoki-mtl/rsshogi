use super::Position;
use crate::board::state_info::{
    CHECK_SQ_BISHOP, CHECK_SQ_DRAGON, CHECK_SQ_GOLD, CHECK_SQ_HORSE, CHECK_SQ_KNIGHT,
    CHECK_SQ_LANCE, CHECK_SQ_PAWN, CHECK_SQ_ROOK, CHECK_SQ_SILVER, CheckSquares, StateHot,
    StateInfo, TacticalCache,
};
use crate::types::Bitboard;
use crate::types::{Color, PieceType, Square};

/// 現局面の戦術キャッシュ群への薄い view。
///
/// `Position::current_state_cache` から取得し、`checkers` / `check_squares`
/// / `blockers_for_king` / `pinners` などを「取得時点の current state」に対して
/// 一括で読むための入口となる。
///
/// # Lifetime と invalidation
///
/// この view は `Position`（および内部の `StateStack`）への共有 borrow である。
/// safe Rust の借用規則により、view が live な間は次のような mutation は
/// コンパイルエラーで防止される。
///
/// - `apply_move32` / `undo_move32` / `apply_null_move` / `undo_null_move`
/// - `state_stack_mut()` 経由の操作
///
/// 「物理アドレスは安定」と「view が指す state が current である」は別概念であり、
/// `apply_move32` などで current を進めた後は、たとえ raw pointer を介して
/// 旧 entry が live でも、その entry は current state を表さなくなる。
/// view を再取得すること。
///
/// # Cheaply copyable
///
/// `StateCacheView<'_>` は `Copy` であり、accessor 呼び出しは self consume で済む。
/// ただし mutation 境界を跨いで stash する用途には使わないこと。
#[derive(Clone, Copy)]
pub struct StateCacheView<'a> {
    tactical: &'a TacticalCache,
}

impl<'a> StateCacheView<'a> {
    #[inline(always)]
    pub(crate) const fn from_hot(hot: &'a StateHot) -> Self {
        Self { tactical: hot.tactical_cache() }
    }

    /// 王手をかけている駒の集合
    #[must_use]
    #[inline(always)]
    pub fn checkers(self) -> Bitboard {
        self.tactical.checkers()
    }

    /// 各駒種について「その駒を配置すると敵玉に王手となるマス」のキャッシュ配列。
    ///
    /// 配列全体への共有参照を返すため、配列コピーを発生させずに read できる。
    #[must_use]
    #[inline(always)]
    pub fn check_squares(self) -> &'a CheckSquares {
        &self.tactical.check_squares
    }

    /// 指定した駒種の `check_squares` を取得する。
    #[must_use]
    #[inline(always)]
    pub fn check_square(self, piece_type: PieceType) -> Bitboard {
        self.tactical.check_square(piece_type)
    }

    /// 玉を守っているブロッカー集合
    #[must_use]
    #[inline(always)]
    pub fn blockers_for_king(self, color: Color) -> Bitboard {
        self.tactical.blockers_for_king(color)
    }

    /// 玉に対してピンしている敵の大駒
    #[must_use]
    #[inline(always)]
    pub fn pinners(self, color: Color) -> Bitboard {
        self.tactical.pinners(color)
    }
}

impl Position {
    /// 現局面の戦術キャッシュ群を一括で読む view を取得する。
    ///
    /// `checkers` / `check_squares` / `blockers_for_king` / `pinners`
    /// などを 1 回の借用でまとめて読みたいときの入口。
    ///
    /// 個別 getter（`Position::checkers()` など）は内部的に同じ view を経由する
    /// 薄い shim になっており、view は state を 1 度だけ参照する形で
    /// 配列コピーを避けられる。
    ///
    /// # 寿命と invalidation
    ///
    /// 詳細は [`StateCacheView`] を参照。
    /// `apply_move32` / `apply_null_move` / `undo_*` / `state_stack_mut()` を
    /// 呼ぶ前に view を破棄すること（safe Rust の借用規則で強制される）。
    #[must_use]
    #[inline(always)]
    pub fn current_state_cache(&self) -> StateCacheView<'_> {
        StateCacheView::from_hot(self.current_hot())
    }

    /// 手番側の玉に王手をかけている駒の `Bitboard` を返す。
    #[must_use]
    #[inline]
    pub fn checkers(&self) -> Bitboard {
        self.current_state_cache().checkers()
    }

    /// 現局面の check-squares キャッシュ（駒種別の「配置すると敵玉に王手になるマス」）を取得する。
    ///
    /// 圧縮 cache 全体への共有参照を返すため、配列コピーを発生させずに read できる。
    /// 駒種を 1 つだけ読むなら [`Position::check_square`] / [`StateCacheView::check_square`]。
    #[must_use]
    #[inline]
    pub fn check_squares(&self) -> &CheckSquares {
        self.current_state_cache().check_squares()
    }

    /// check_squares のキャッシュを取得する（テスト用エイリアス）。
    #[cfg(test)]
    #[must_use]
    #[inline]
    pub(crate) fn check_squares_cache(&self) -> &CheckSquares {
        self.check_squares()
    }

    /// 指定した駒種の check_squares を取得する。
    #[must_use]
    #[inline]
    pub fn check_square(&self, piece_type: PieceType) -> Bitboard {
        self.current_state_cache().check_square(piece_type)
    }

    /// ピンされている駒の Bitboard を返す（キャッシュ済み）。
    #[must_use]
    pub fn pinned_pieces(&self, c: Color) -> Bitboard {
        self.blockers_for_king(c).and(self.bitboards.color_pieces(c))
    }

    /// 回避対象を除外してピンされている駒を返す。
    #[must_use]
    pub fn pinned_pieces_avoid(&self, c: Color, avoid: Square) -> Bitboard {
        use crate::board::attack_tables::{
            bishop_step_attacks, lance_step_attacks, rook_step_attacks,
        };

        let king_sq = self.king_square(c);
        if king_sq.is_none() {
            return Bitboard::EMPTY;
        }

        let them = c.flip();
        let rook_like = self.bitboards().pieces_for(PieceType::ROOK, them)
            | self.bitboards().pieces_for(PieceType::DRAGON, them);
        let bishop_like = self.bitboards().pieces_for(PieceType::BISHOP, them)
            | self.bitboards().pieces_for(PieceType::HORSE, them);
        let lance_like = self.bitboards().pieces_for(PieceType::LANCE, them);

        let avoid_bb =
            if avoid.is_none() { Bitboard::ALL } else { Bitboard::from_square(avoid).not() };
        let rook_snipers = rook_like & rook_step_attacks(king_sq) & avoid_bb;
        let bishop_snipers = bishop_like & bishop_step_attacks(king_sq) & avoid_bb;
        let lance_snipers = lance_like & lance_step_attacks(king_sq, c) & avoid_bb;

        let snipers = rook_snipers | bishop_snipers | lance_snipers;
        let occupancy = self.bitboards().occupied() & avoid_bb;
        let mut result = Bitboard::EMPTY;

        for sniper_sq in &snipers {
            let between = Bitboard::between(sniper_sq, king_sq) & occupancy;
            if between.count() == 1 {
                result |= between & self.bitboards.color_pieces(c);
            }
        }

        result
    }

    /// 玉を守っているブロッカー（pin候補を含む）を返す
    #[must_use]
    #[inline]
    pub fn blockers_for_king(&self, c: Color) -> Bitboard {
        self.current_state_cache().blockers_for_king(c)
    }

    /// 玉に対してピンしている敵の大駒を返す
    #[must_use]
    #[inline]
    pub fn pinners(&self, c: Color) -> Bitboard {
        self.current_state_cache().pinners(c)
    }

    /// 現局面で王手がかかっているかを返す。
    #[must_use]
    pub fn is_in_check(&self) -> bool {
        !self.checkers().is_empty()
    }

    /// 指し手が空き王手になる候補かを判定する。
    #[must_use]
    pub fn is_discovery_check_on_king(&self, c: Color, mv: crate::types::Move32) -> bool {
        self.blockers_for_king(c).test(mv.to_move().from_sq())
    }

    /// check_squares を計算する。
    /// 各駒種について、その駒を配置すると敵玉に王手となるマスのビットボードを計算する。
    ///
    /// 全駒種の王手候補とピン情報を計算する。
    /// 条件分岐によるスライダー計算スキップは、典型的な局面では分岐コストが
    /// 計算コストを上回るため行わない。
    #[inline]
    pub(crate) fn compute_check_squares(&self) -> CheckSquares {
        match self.turn() {
            Color::BLACK => self.compute_check_squares_for::<true>(),
            Color::WHITE => self.compute_check_squares_for::<false>(),
        }
    }

    /// `compute_check_squares` の手番定数化版。
    #[inline]
    pub(crate) fn compute_check_squares_for<const BLACK_TURN: bool>(&self) -> CheckSquares {
        use crate::board::attack_tables::{
            GOLD_ATTACKS, KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS, SILVER_ATTACKS,
        };
        use crate::board::attack_tables::{bishop_attacks, lance_step_attacks, rook_attacks};

        let mut check_squares = CheckSquares::EMPTY;

        let them = if BLACK_TURN { Color::WHITE } else { Color::BLACK };
        let ksq = self.king_square(them);
        if ksq.is_none() {
            return check_squares;
        }

        let occupied = self.bitboards().occupied();

        // 各駒種について、敵玉から逆向きに攻撃範囲を計算（無条件）
        let pawn = PAWN_ATTACKS[ksq][them.to_index()];
        let knight = KNIGHT_ATTACKS[ksq][them.to_index()];
        let silver = SILVER_ATTACKS[ksq][them.to_index()];
        let gold = GOLD_ATTACKS[ksq][them.to_index()];
        let bishop = bishop_attacks(ksq, occupied);
        let rook = rook_attacks(ksq, occupied);

        // 香は飛車の利きを香のstep attacksでマスク
        let lance = rook & lance_step_attacks(ksq, them);
        let king = KING_ATTACKS[ksq];
        let horse = bishop | king;
        let dragon = rook | king;

        check_squares.set(CHECK_SQ_PAWN, pawn);
        check_squares.set(CHECK_SQ_LANCE, lance);
        check_squares.set(CHECK_SQ_KNIGHT, knight);
        check_squares.set(CHECK_SQ_SILVER, silver);
        check_squares.set(CHECK_SQ_BISHOP, bishop);
        check_squares.set(CHECK_SQ_ROOK, rook);
        check_squares.set(CHECK_SQ_GOLD, gold);
        check_squares.set(CHECK_SQ_HORSE, horse);
        check_squares.set(CHECK_SQ_DRAGON, dragon);

        check_squares
    }

    pub(super) fn compute_checkers_for(&self, color: Color) -> Bitboard {
        let us = color;
        let them = us.flip();
        let occupied = self.bitboards.occupied();

        let king_sq = self.king_square(us);
        if king_sq.is_none() {
            return Bitboard::EMPTY;
        }

        self.attackers_to_color_fast(them, king_sq, occupied)
    }

    pub(super) fn recompute_caches(&mut self) {
        let mut state = StateInfo::default();
        self.compute_caches_for_state(&mut state);
        self.sync_caches_from_state(&state);
    }

    pub(crate) fn compute_caches_for_state(&self, state: &mut StateInfo) {
        let (black_blockers, black_pinners) = self.compute_slider_info_for::<true>();
        let (white_blockers, white_pinners) = self.compute_slider_info_for::<false>();
        let (checkers, check_squares) = match self.turn() {
            Color::BLACK => {
                (self.compute_checkers_for(Color::BLACK), self.compute_check_squares_for::<true>())
            }
            Color::WHITE => {
                (self.compute_checkers_for(Color::WHITE), self.compute_check_squares_for::<false>())
            }
        };
        state.replace_tactical_cache(TacticalCache::new(
            checkers,
            [black_pinners, white_pinners],
            [black_blockers, white_blockers],
            check_squares,
        ));

        self.write_partial_keys_to_state(state);
    }

    pub(crate) fn sync_caches_from_state(&mut self, state: &StateInfo) {
        self.state_stack.sync_caches_from_state(self.st_index, state);
    }
}
