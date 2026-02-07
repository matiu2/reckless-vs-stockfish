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
