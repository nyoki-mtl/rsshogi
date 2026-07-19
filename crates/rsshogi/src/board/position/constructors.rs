use super::Position;
use super::types::BoardArray;
use crate::board::bitboard_set::BitboardSet;
use crate::board::state_info::{StateInfo, StateStack};
use crate::board::zobrist::ZobristKey;
use crate::types::{Color, EnteringKingRule, Hand, Square};

impl Position {
    /// 新しい空の盤面を作成
    #[must_use]
    pub fn empty() -> Self {
        Self {
            board: BoardArray::empty(),
            bitboards: BitboardSet::new(),
            hands: [Hand::ZERO; Color::COUNT],
            zobrist: ZobristKey::default(),
            board_key: ZobristKey::default(),
            hand_key: ZobristKey::default(),
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

    pub(super) fn reset_state_stack_to_current_position(&mut self) {
        let mut state = StateInfo::default();
        state.write_root_hot(self.board_key, self.hand_key, self.hand(self.turn()));
        self.compute_caches_for_state(&mut state);

        let stack = self.state_stack_mut();
        stack.reset_sfen();
        let st_index = stack.current_index();
        stack.write_current_state(state);
        self.st_index = st_index;
        self.debug_assert_partial_keys_consistent();
    }
}
