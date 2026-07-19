//! 対局結果の型定義
//!
//! [`GameResult`] は将棋の対局結果を表す列挙型である。
//! 通常の勝敗に加え、入玉宣言勝ち、千日手、持将棋、反則負け、
//! 時間切れ、中断など、多様な終局パターンを網羅する。
//!
//! # 分類体系
//!
//! 内部値の下位 2 ビットで大分類される。
//!
//! | 下位 2 bit | 分類 |
//! |:----------:|------|
//! | `0` | 先手勝ち |
//! | `1` | 後手勝ち |
//! | `2` | 引き分け |
//! | `3` | エラー / 無効 / 中断 |
//!
//! # 判定メソッド
//!
//! - [`is_black_win()`](GameResult::is_black_win) / [`is_white_win()`](GameResult::is_white_win) - 勝敗判定
//! - [`is_draw()`](GameResult::is_draw) - 引き分け判定
//! - [`is_win()`](GameResult::is_win) - どちらかの勝利か
//! - [`winner_color()`](GameResult::winner_color) - 勝者の手番を取得
//! - [`to_score(color)`](GameResult::to_score) - 指定手番から見たスコア（勝ち=1, 負け=0, 引き分け=0.5）

use super::Color;

/// 対局結果
///
/// 下位2bitで分類：
/// - `value & 3 == 0`: 先手勝ち
/// - `value & 3 == 1`: 後手勝ち  
/// - `value & 3 == 2`: 引き分け
/// - `value & 3 == 3`: エラー/無効
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GameResult {
    /// 先手勝ち
    BlackWin = 0,
    /// 後手勝ち
    WhiteWin = 1,
    /// 千日手による引き分け
    DrawByRepetition = 2,
    /// エラー
    Error = 3,

    /// 先手の入玉宣言勝ち
    BlackWinByDeclaration = 4,
    /// 後手の入玉宣言勝ち
    WhiteWinByDeclaration = 5,
    /// 最大手数による引き分け
    DrawByMaxPlies = 6,
    /// 無効な結果
    Invalid = 7,

    /// 先手の不戦勝
    BlackWinByForfeit = 8,
    /// 後手の不戦勝
    WhiteWinByForfeit = 9,
    /// 持将棋（点数による引き分け）
    DrawByImpasse = 10,
    /// 中断
    Paused = 11,
    /// 後手の反則負け（先手勝ち）
    /// 連続王手の千日手を含む
    BlackWinByIllegalMove = 12,
    /// 先手の反則負け（後手勝ち）
    /// 連続王手の千日手を含む
    WhiteWinByIllegalMove = 13,
    // 14-15: 予約。下位2bit分類を維持したまま次の勝敗ブロックを使うために空けている。
    /// 後手の時間切れ（先手勝ち）
    BlackWinByTimeout = 16,
    /// 先手の時間切れ（後手勝ち）
    WhiteWinByTimeout = 17,
    // 18-19: 予約。20-23 ブロックを try-rule 系に使うため空けている。
    /// 先手のトライルール勝ち
    BlackWinByTryRule = 20,
    /// 後手のトライルール勝ち
    WhiteWinByTryRule = 21,
}

const _: () = {
    assert!(core::mem::size_of::<GameResult>() == 1);
    assert!(core::mem::align_of::<GameResult>() == 1);
};

impl GameResult {
    /// 先手勝ちか？
    #[must_use]
    pub const fn is_black_win(self) -> bool {
        matches!(
            self,
            Self::BlackWin
                | Self::BlackWinByDeclaration
                | Self::BlackWinByForfeit
                | Self::BlackWinByIllegalMove
                | Self::BlackWinByTryRule
                | Self::BlackWinByTimeout
        )
    }

    /// 後手勝ちか？
    #[must_use]
    pub const fn is_white_win(self) -> bool {
        matches!(
            self,
            Self::WhiteWin
                | Self::WhiteWinByDeclaration
                | Self::WhiteWinByForfeit
                | Self::WhiteWinByIllegalMove
                | Self::WhiteWinByTryRule
                | Self::WhiteWinByTimeout
        )
    }

    /// 引き分けか？
    #[must_use]
    pub const fn is_draw(self) -> bool {
        matches!(self, Self::DrawByRepetition | Self::DrawByMaxPlies | Self::DrawByImpasse)
    }

    /// どちらかが勝利したか？
    #[must_use]
    pub const fn is_win(self) -> bool {
        self.is_black_win() || self.is_white_win()
    }

    /// 宣言勝ちか？
    #[must_use]
    pub const fn is_win_by_declaration(self) -> bool {
        matches!(self, Self::BlackWinByDeclaration | Self::WhiteWinByDeclaration)
    }

    /// トライルール勝ちか？
    #[must_use]
    pub const fn is_win_by_try_rule(self) -> bool {
        matches!(self, Self::BlackWinByTryRule | Self::WhiteWinByTryRule)
    }

    /// 勝利した手番を返す（勝敗が付かない場合はNone）
    #[must_use]
    pub const fn winner_color(self) -> Option<Color> {
        if self.is_black_win() {
            Some(Color::BLACK)
        } else if self.is_white_win() {
            Some(Color::WHITE)
        } else {
            None
        }
    }

    /// 指定した手番から見たスコアを返す。
    ///
    /// 勝ちを1、負けを0、引き分けを0.5として返す。中断局などは-1。
    #[must_use]
    pub fn to_score(self, color: Color) -> f32 {
        if color == Color::BLACK {
            if self.is_black_win() {
                return 1.0;
            }
            if self.is_white_win() {
                return 0.0;
            }
        } else if color == Color::WHITE {
            if self.is_white_win() {
                return 1.0;
            }
            if self.is_black_win() {
                return 0.0;
            }
        }
        if self.is_draw() {
            return 0.5;
        }
        -1.0
    }

    /// 勝利した手番から結果を生成
    #[must_use]
    pub const fn win_from_color(color: Color) -> Self {
        if color.raw() == Color::BLACK.raw() { Self::BlackWin } else { Self::WhiteWin }
    }

    /// 宣言勝ちの結果を生成
    #[must_use]
    pub const fn win_by_declaration_from_color(color: Color) -> Self {
        if color.raw() == Color::BLACK.raw() {
            Self::BlackWinByDeclaration
        } else {
            Self::WhiteWinByDeclaration
        }
    }

    /// 不戦勝の結果を生成
    #[must_use]
    pub const fn win_by_forfeit_from_color(color: Color) -> Self {
        if color.raw() == Color::BLACK.raw() {
            Self::BlackWinByForfeit
        } else {
            Self::WhiteWinByForfeit
        }
    }

    /// 反則勝ちの結果を生成
    #[must_use]
    pub const fn win_by_illegal_move_from_color(color: Color) -> Self {
        if color.raw() == Color::BLACK.raw() {
            Self::BlackWinByIllegalMove
        } else {
            Self::WhiteWinByIllegalMove
        }
    }

    /// 時間切れ勝ちの結果を生成
    #[must_use]
    pub const fn win_by_timeout_from_color(color: Color) -> Self {
        if color.raw() == Color::BLACK.raw() {
            Self::BlackWinByTimeout
        } else {
            Self::WhiteWinByTimeout
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_result_winner() {
        assert_eq!(GameResult::BlackWin.winner_color(), Some(Color::BLACK));
        assert_eq!(GameResult::WhiteWinByTimeout.winner_color(), Some(Color::WHITE));
        assert_eq!(GameResult::DrawByRepetition.winner_color(), None);
    }

    #[test]
    fn test_game_result_flags() {
        assert!(GameResult::BlackWin.is_black_win());
        assert!(GameResult::WhiteWin.is_white_win());
        assert!(GameResult::DrawByMaxPlies.is_draw());
        assert!(!GameResult::Paused.is_draw());
        assert!(GameResult::BlackWinByDeclaration.is_win_by_declaration());
        assert!(!GameResult::Paused.is_win());
    }

    #[test]
    fn test_game_result_score() {
        assert_eq!(GameResult::BlackWin.to_score(Color::BLACK), 1.0);
        assert_eq!(GameResult::BlackWin.to_score(Color::WHITE), 0.0);
        assert_eq!(GameResult::DrawByRepetition.to_score(Color::BLACK), 0.5);
        assert_eq!(GameResult::Paused.to_score(Color::BLACK), -1.0);
    }

    #[test]
    fn test_game_result_numeric_values() {
        assert_eq!(GameResult::DrawByImpasse as u8, 10);
        assert_eq!(GameResult::Paused as u8, 11);
        assert_eq!(GameResult::BlackWinByTryRule as u8, 20);
        assert_eq!(GameResult::WhiteWinByTryRule as u8, 21);
    }
}
