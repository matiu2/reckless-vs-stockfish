//! Game runner - plays a single game between two engines.

use crate::engine::UciEngine;
use crate::game::GameResult;
use color_eyre::eyre::{Result, eyre};
use shakmaty::{Chess, Color, Position, uci::UciMove};

/// Runs chess games between two UCI engines.
pub struct GameRunner {
    movetime_ms: u64,
    max_moves: u32,
}

impl GameRunner {
    /// Create a new game runner.
    #[must_use]
    pub const fn new(movetime_ms: u64, max_moves: u32) -> Self {
        Self {
            movetime_ms,
            max_moves,
        }
    }

    /// Play a single game between white and black engines.
    ///
    /// # Errors
    /// Returns an error if engine communication fails or produces invalid moves.
    pub async fn play_game(
        &self,
        white: &mut UciEngine,
        black: &mut UciEngine,
    ) -> Result<GameResult> {
        let mut position = Chess::default();
        let mut moves: Vec<String> = Vec::new();

        // Initialize both engines for a new game
        white.new_game().await?;
        black.new_game().await?;

        for move_num in 0..self.max_moves {
            let is_white_turn = position.turn() == Color::White;

            // Set position and get best move from the current player
            let uci_move_str = if is_white_turn {
                white.set_position(&moves).await?;
                white.get_best_move(self.movetime_ms).await?
            } else {
                black.set_position(&moves).await?;
                black.get_best_move(self.movetime_ms).await?
            };

            // Handle special case: engine resigns or can't move
            if uci_move_str == "(none)" || uci_move_str.is_empty() {
                return Ok(if position.turn() == Color::White {
                    GameResult::BlackWins
                } else {
                    GameResult::WhiteWins
                });
            }

            // Parse and validate the move
            let uci_move: UciMove = uci_move_str
                .parse()
                .map_err(|e| eyre!("Invalid UCI move '{uci_move_str}': {e}"))?;

            let chess_move = uci_move
                .to_move(&position)
                .map_err(|e| eyre!("Illegal move '{uci_move_str}': {e}"))?;

            // Apply the move
            position = position
                .play(&chess_move)
                .map_err(|e| eyre!("Failed to apply move '{uci_move_str}': {e}"))?;
            moves.push(uci_move_str);

            tracing::trace!(
                move_num,
                last_move = %moves.last().unwrap_or(&String::new()),
                "Move played"
            );

            // Check for game end
            if position.is_checkmate() {
                // The side to move is checkmated, so the other side wins
                return Ok(if position.turn() == Color::White {
                    GameResult::BlackWins
                } else {
                    GameResult::WhiteWins
                });
            }

            if position.is_stalemate() || position.is_insufficient_material() {
                return Ok(GameResult::Draw);
            }

            // Check for draw by repetition or 50-move rule
            // shakmaty handles halfmoves counter for 50-move rule
            if position.halfmoves() >= 100 {
                return Ok(GameResult::Draw);
            }
        }

        // Max moves reached - declare draw
        tracing::debug!(
            "Game reached max moves ({}) - declaring draw",
            self.max_moves
        );
        Ok(GameResult::Draw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_play_single_game() {
        let mut stockfish = UciEngine::new("stockfish", "stockfish")
            .await
            .expect("Failed to init stockfish");
        let mut reckless = UciEngine::new("reckless", "reckless")
            .await
            .expect("Failed to init reckless");

        let runner = GameRunner::new(50, 200);
        let result = runner.play_game(&mut stockfish, &mut reckless).await;

        assert!(result.is_ok(), "Game failed: {result:?}");
        let result = result.unwrap();
        tracing::info!(?result, "Game completed");

        stockfish.quit().await.ok();
        reckless.quit().await.ok();
    }

    #[tokio::test]
    async fn test_play_game_reversed_colors() {
        let mut stockfish = UciEngine::new("stockfish", "stockfish")
            .await
            .expect("Failed to init stockfish");
        let mut reckless = UciEngine::new("reckless", "reckless")
            .await
            .expect("Failed to init reckless");

        let runner = GameRunner::new(50, 200);
        // This time reckless plays white
        let result = runner.play_game(&mut reckless, &mut stockfish).await;

        assert!(result.is_ok(), "Game failed: {result:?}");
        let result = result.unwrap();
        tracing::info!(?result, "Game completed (reckless as white)");

        stockfish.quit().await.ok();
        reckless.quit().await.ok();
    }
}
