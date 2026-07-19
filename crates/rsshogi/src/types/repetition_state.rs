//! 千日手（同一局面反復）の状態定義
//!
//! 将棋では同一局面が 4 回出現すると千日手が成立する。
//! ただし、連続王手による千日手は王手を掛けている側の反則負けとなる。
//!
//! # バリアント
//!
//! | 状態 | 意味 |
//! |------|------|
//! | `None` | 千日手ではない |
//! | `Draw` | 通常の千日手（引き分け） |
//! | `Win` | 連続王手の千日手で相手の反則負け（自分の勝ち） |
//! | `Lose` | 連続王手の千日手で自分の反則負け |
//! | `Superior` | 優等局面（持ち駒が過去局面より多い） |
//! | `Inferior` | 劣等局面（持ち駒が過去局面より少ない） |

// ANCHOR: repetition_state_enum
/// 千日手の状態を表す enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RepetitionState {
    /// 千日手でない
    #[default]
    None,
    /// 連続王手の千日手で相手が負け（自分の勝ち）
    Win,
    /// 連続王手の千日手で自分が負け
    Lose,
    /// 通常の千日手（引き分け）
    Draw,
    /// 優等局面（持ち駒が過去局面より優れている）
    Superior,
    /// 劣等局面（持ち駒が過去局面より劣っている）
    Inferior,
}
// ANCHOR_END: repetition_state_enum

/// 千日手状態の総数。
pub(crate) const REPETITION_COUNT: usize = 6;

impl RepetitionState {
    /// 千日手状態の総数
    pub const COUNT: usize = REPETITION_COUNT;

    /// USI 拡張文字列を返す。
    #[must_use]
    pub const fn to_usi(self) -> &'static str {
        match self {
            Self::None => "rep_none",
            Self::Win => "rep_win",
            Self::Lose => "rep_lose",
            Self::Draw => "rep_draw",
            Self::Superior => "rep_sup",
            Self::Inferior => "rep_inf",
        }
    }
}

impl std::fmt::Display for RepetitionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_usi())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repetition_state_derives() {
        // Cloneのテスト
        let state = RepetitionState::Win;
        let cloned = state;
        assert_eq!(state, cloned);

        // Copyのテスト
        let state2 = RepetitionState::Lose;
        let copied = state2;
        assert_eq!(state2, copied);

        // PartialEqのテスト
        assert_eq!(RepetitionState::None, RepetitionState::None);
        assert_ne!(RepetitionState::Win, RepetitionState::Lose);

        // Debugのテスト
        let debug_str = format!("{:?}", RepetitionState::Draw);
        assert_eq!(debug_str, "Draw");
    }

    #[test]
    fn test_all_states_are_unique() {
        let states = [
            RepetitionState::None,
            RepetitionState::Win,
            RepetitionState::Lose,
            RepetitionState::Draw,
            RepetitionState::Superior,
            RepetitionState::Inferior,
        ];

        // 全ての状態が異なることを確認
        for (i, state1) in states.iter().enumerate() {
            for (j, state2) in states.iter().enumerate() {
                if i == j {
                    assert_eq!(state1, state2);
                } else {
                    assert_ne!(state1, state2);
                }
            }
        }
    }

    #[test]
    fn test_default_is_none() {
        let state = RepetitionState::default();
        assert_eq!(state, RepetitionState::None);
    }

    #[test]
    fn test_repetition_state_usi_string() {
        assert_eq!(RepetitionState::None.to_usi(), "rep_none");
        assert_eq!(RepetitionState::Win.to_usi(), "rep_win");
        assert_eq!(RepetitionState::Lose.to_usi(), "rep_lose");
        assert_eq!(RepetitionState::Draw.to_usi(), "rep_draw");
        assert_eq!(RepetitionState::Superior.to_usi(), "rep_sup");
        assert_eq!(RepetitionState::Inferior.to_usi(), "rep_inf");
    }
}
