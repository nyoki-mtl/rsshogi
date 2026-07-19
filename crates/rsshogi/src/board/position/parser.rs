use super::Position;
use crate::board::parser::{PositionState, SfenError, generate_sfen_with_ply, parse_sfen};
use crate::types::{Color, EnteringKingRule, Piece};

impl Position {
    /// SFEN 文字列から局面を生成する。
    ///
    /// [`position_from_sfen`](crate::board::position_from_sfen) の関連関数形式。
    pub fn from_sfen(sfen: &str) -> Result<Self, SfenError> {
        let mut pos = Self::empty();
        pos.set_sfen(sfen)?;
        Ok(pos)
    }

    /// SFEN文字列で盤面を設定する（既存局面の上書き）
    pub fn set_sfen(&mut self, sfen: &str) -> Result<(), SfenError> {
        let state = parse_sfen(sfen)?;
        self.apply_position_state(&state);
        Ok(())
    }

    /// [`PositionState`] から盤面を設定する
    ///
    /// [`set_sfen()`](Self::set_sfen) が SFEN 文字列を受け取るのに対し、
    /// こちらは構造化済みの `PositionState` を直接受け取る。
    /// エディタ等で個別フィールドを変更した `PositionState` を局面に反映する用途に使用する。
    ///
    /// 盤面キャッシュやハッシュは再構築するが、局面のルール妥当性は検証しない。
    /// import 後に妥当性も確認したい場合は、`validation` feature を有効にして
    /// `validate()` / `validate_all()` を使う。
    ///
    /// # Examples
    ///
    /// ```
    /// use rsshogi::board::{self, Position, parse_sfen};
    ///
    /// board::init();
    /// let mut state = parse_sfen(
    ///     "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"
    /// ).unwrap();
    /// state.ply = 10; // 手数を変更
    ///
    /// let mut pos = Position::empty();
    /// pos.set_position_state(&state);
    /// assert_eq!(pos.game_ply(), 10);
    /// ```
    pub fn set_position_state(&mut self, state: &PositionState) {
        self.apply_position_state(state);
    }

    /// 平手初期局面に設定する
    pub fn set_hirate(&mut self) {
        self.set_sfen(crate::board::STARTPOS_SFEN).expect("Invalid startpos SFEN");
    }

    fn apply_position_state(&mut self, state: &PositionState) {
        self.board = state.board;
        self.hands = state.hands;
        self.set_side_to_move(state.side_to_move);
        self.ply = state.ply;
        self.entering_king_rule = EnteringKingRule::None;
        self.entering_king_point = [0, 0];

        // ビットボードを再構築
        self.rebuild_bitboards();

        // Zobristハッシュを計算
        let keys = self.compute_keys();
        let (board_key, hand_key) = (keys.board_key, keys.hand_key);
        self.board_key = board_key;
        self.hand_key = hand_key;
        self.zobrist = board_key ^ hand_key;

        self.reset_state_stack_to_current_position();
        self.debug_assert_partial_keys_consistent();
    }

    /// SFEN文字列に変換。
    #[must_use]
    /// `None` の場合は現在の手数を使用する。
    /// `Some(x)` で負数を指定した場合は手数を出力しない。
    pub fn to_sfen(&self, game_ply: Option<i32>) -> String {
        let ply = game_ply.unwrap_or_else(|| i32::from(self.ply));
        let ply = if ply < 0 { None } else { Some(ply) };
        generate_sfen_with_ply(self, ply)
    }

    /// 先後反転（盤面を180度回転し、駒色と手番を反転）したSFENを取得
    #[must_use]
    /// `None` の場合は現在の手数を使用する。
    /// `Some(x)` で負数を指定した場合は手数を出力しない。
    pub fn to_sfen_flipped(&self, game_ply: Option<i32>) -> String {
        let mut flipped = Self::empty();

        for (sq, piece) in self.board.iter() {
            if piece.is_empty() {
                continue;
            }
            let flipped_piece = Piece::from_parts(piece.color().flip(), piece.piece_type());
            flipped.board.set(sq.flip(), flipped_piece);
        }

        flipped.hands[Color::BLACK.to_index()] = self.hands[Color::WHITE.to_index()];
        flipped.hands[Color::WHITE.to_index()] = self.hands[Color::BLACK.to_index()];
        flipped.set_side_to_move(self.turn().flip());
        flipped.ply = self.ply;

        let ply = game_ply.unwrap_or_else(|| i32::from(self.ply));
        let ply = if ply < 0 { None } else { Some(ply) };
        generate_sfen_with_ply(&flipped, ply)
    }
}
