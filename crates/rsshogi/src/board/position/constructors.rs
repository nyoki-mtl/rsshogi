use super::Position;
use super::types::BoardArray;
use crate::board::bitboard_set::BitboardSet;
use crate::board::state_info::{StateInfo, StateStack};
use crate::types::{Color, EnteringKingRule, Hand, Square};

impl Position {
    /// 新しい空の盤面を作成
    #[must_use]
    pub fn empty() -> Self {
        Self {
            board: BoardArray::empty(),
            bitboards: BitboardSet::new(),
            hands: [Hand::ZERO; Color::COUNT],
            side_to_move: Color::BLACK,
            entering_king_rule: EnteringKingRule::None,
            entering_king_point: [0, 0],
            ply: 1,
            st_index: 0,
            state_stack: StateStack::new(),
            king_square: [Square::NONE; Color::COUNT],
        }
    }

    /// スタックを初期化する（探索開始時などに使用）
    pub fn init_stack(&mut self) {
        self.reset_state_stack_to_current_position();
    }

    /// 現在の盤面から root state を作り直し、state stack をそこにリセットする。
    ///
    /// キーの単一の真実点は state 側にあるため、ここで全再計算して書き込む。
    pub(super) fn reset_state_stack_to_current_position(&mut self) {
        let keys = self.compute_keys();
        let mut state = StateInfo::default();
        state.write_root_hot(keys.board_key, keys.key, self.hand(self.turn()));
        self.compute_caches_for_state(&mut state);

        let stack = self.state_stack_mut();
        stack.reset_sfen();
        let st_index = stack.current_index();
        stack.write_current_state(state);
        self.st_index = st_index;
        self.debug_assert_partial_keys_consistent();
    }
}
