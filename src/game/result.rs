//! Game result types.

use serde::{Deserialize, Serialize};

/// The result of a single game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameResult {
    /// White won the game
    WhiteWins,
    /// Black won the game
    BlackWins,
    /// The game was a draw
    Draw,
}

impl GameResult {
    /// Returns true if white won.
    #[must_use]
    pub const fn is_white_win(self) -> bool {
        matches!(self, Self::WhiteWins)
    }

    /// Returns true if black won.
    #[must_use]
    pub const fn is_black_win(self) -> bool {
        matches!(self, Self::BlackWins)
    }

    /// Returns true if the game was a draw.
    #[must_use]
    pub const fn is_draw(self) -> bool {
        matches!(self, Self::Draw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_result_predicates() {
        assert!(GameResult::WhiteWins.is_white_win());
        assert!(!GameResult::WhiteWins.is_black_win());
        assert!(!GameResult::WhiteWins.is_draw());

        assert!(!GameResult::BlackWins.is_white_win());
        assert!(GameResult::BlackWins.is_black_win());
        assert!(!GameResult::BlackWins.is_draw());

        assert!(!GameResult::Draw.is_white_win());
        assert!(!GameResult::Draw.is_black_win());
        assert!(GameResult::Draw.is_draw());
    }
}
