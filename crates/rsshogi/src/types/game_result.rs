use super::Color;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GameResult {
    BlackWin = 0,
    WhiteWin = 1,
    DrawByRepetition = 2,
    Error = 3,
    BlackWinByDeclaration = 4,
    WhiteWinByDeclaration = 5,
    DrawByMaxPlies = 6,
    #[default]
    Invalid = 7,
    BlackWinByForfeit = 8,
    WhiteWinByForfeit = 9,
    DrawByImpasse = 10,
    Paused = 11,
    BlackWinByIllegalMove = 12,
    WhiteWinByIllegalMove = 13,
    BlackWinByTimeout = 16,
    WhiteWinByTimeout = 17,
    BlackWinByTryRule = 20,
    WhiteWinByTryRule = 21,
}

impl GameResult {
    pub const fn is_black_win(self) -> bool {
        matches!(
            self,
            Self::BlackWin
                | Self::BlackWinByDeclaration
                | Self::BlackWinByForfeit
                | Self::BlackWinByIllegalMove
                | Self::BlackWinByTimeout
                | Self::BlackWinByTryRule
        )
    }
    pub const fn is_white_win(self) -> bool {
        matches!(
            self,
            Self::WhiteWin
                | Self::WhiteWinByDeclaration
                | Self::WhiteWinByForfeit
                | Self::WhiteWinByIllegalMove
                | Self::WhiteWinByTimeout
                | Self::WhiteWinByTryRule
        )
    }
    pub const fn is_draw(self) -> bool {
        matches!(self, Self::DrawByRepetition | Self::DrawByMaxPlies | Self::DrawByImpasse)
    }
    pub const fn is_win(self) -> bool {
        self.is_black_win() || self.is_white_win()
    }
    pub const fn is_win_by_declaration(self) -> bool {
        matches!(self, Self::BlackWinByDeclaration | Self::WhiteWinByDeclaration)
    }
    pub const fn is_win_by_try_rule(self) -> bool {
        matches!(self, Self::BlackWinByTryRule | Self::WhiteWinByTryRule)
    }
    pub const fn winner_color(self) -> Option<Color> {
        if self.is_black_win() {
            Some(Color::BLACK)
        } else if self.is_white_win() {
            Some(Color::WHITE)
        } else {
            None
        }
    }
    pub const fn to_score(self) -> i8 {
        if self.is_black_win() {
            1
        } else if self.is_white_win() {
            -1
        } else {
            0
        }
    }
    pub const fn win_from_color(color: Color) -> Self {
        match color {
            Color::BLACK => Self::BlackWin,
            Color::WHITE => Self::WhiteWin,
        }
    }
    pub const fn win_by_declaration_from_color(color: Color) -> Self {
        match color {
            Color::BLACK => Self::BlackWinByDeclaration,
            Color::WHITE => Self::WhiteWinByDeclaration,
        }
    }
    pub const fn win_by_forfeit_from_color(color: Color) -> Self {
        match color {
            Color::BLACK => Self::BlackWinByForfeit,
            Color::WHITE => Self::WhiteWinByForfeit,
        }
    }
    pub const fn win_by_illegal_move_from_color(color: Color) -> Self {
        match color {
            Color::BLACK => Self::BlackWinByIllegalMove,
            Color::WHITE => Self::WhiteWinByIllegalMove,
        }
    }
    pub const fn win_by_timeout_from_color(color: Color) -> Self {
        match color {
            Color::BLACK => Self::BlackWinByTimeout,
            Color::WHITE => Self::WhiteWinByTimeout,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GameResult;

    #[test]
    fn result_raw_values_and_classification() {
        let raw_values = [
            (GameResult::BlackWin, 0),
            (GameResult::WhiteWin, 1),
            (GameResult::DrawByRepetition, 2),
            (GameResult::Error, 3),
            (GameResult::BlackWinByDeclaration, 4),
            (GameResult::WhiteWinByDeclaration, 5),
            (GameResult::DrawByMaxPlies, 6),
            (GameResult::Invalid, 7),
            (GameResult::BlackWinByForfeit, 8),
            (GameResult::WhiteWinByForfeit, 9),
            (GameResult::DrawByImpasse, 10),
            (GameResult::Paused, 11),
            (GameResult::BlackWinByIllegalMove, 12),
            (GameResult::WhiteWinByIllegalMove, 13),
            (GameResult::BlackWinByTimeout, 16),
            (GameResult::WhiteWinByTimeout, 17),
            (GameResult::BlackWinByTryRule, 20),
            (GameResult::WhiteWinByTryRule, 21),
        ];
        for (result, expected) in raw_values {
            assert_eq!(result as u8, expected);
        }
        assert_eq!(GameResult::WhiteWinByTryRule.to_score(), -1);
        assert!(GameResult::DrawByImpasse.is_draw());
        assert_eq!(GameResult::BlackWinByForfeit.winner_color(), Some(super::Color::BLACK));
    }
}
