//! UCI protocol implementation for chess engine communication.

use color_eyre::eyre::{ContextCompat, Result, eyre};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

/// A UCI chess engine process.
pub struct UciEngine {
    name: String,
    _process: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl UciEngine {
    /// Spawn a new UCI engine process.
    ///
    /// # Errors
    /// Returns an error if the engine cannot be spawned or doesn't respond to UCI.
    pub async fn new(path: &str, name: &str) -> Result<Self> {
        let mut process = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let stdin = process.stdin.take().context("Failed to get stdin")?;
        let stdout = process.stdout.take().context("Failed to get stdout")?;
        let stdout = BufReader::new(stdout);

        let mut engine = Self {
            name: name.to_string(),
            _process: process,
            stdin,
            stdout,
        };

        // Initialize UCI protocol
        engine.send("uci").await?;
        engine.wait_for("uciok").await?;

        Ok(engine)
    }

    /// Send a command to the engine.
    async fn send(&mut self, command: &str) -> Result<()> {
        tracing::trace!(engine = %self.name, %command, "Sending command");
        self.stdin.write_all(command.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;
        Ok(())
    }

    /// Wait for a specific response line from the engine.
    async fn wait_for(&mut self, expected: &str) -> Result<()> {
        let mut line = String::new();
        loop {
            line.clear();
            let bytes_read = self.stdout.read_line(&mut line).await?;
            if bytes_read == 0 {
                return Err(eyre!("Engine {} closed unexpectedly", self.name));
            }
            let trimmed = line.trim();
            tracing::trace!(engine = %self.name, response = %trimmed, "Received");
            if trimmed == expected {
                return Ok(());
            }
        }
    }

    /// Tell the engine we're ready to start a new game.
    ///
    /// # Errors
    /// Returns an error if the engine doesn't respond.
    pub async fn new_game(&mut self) -> Result<()> {
        self.send("ucinewgame").await?;
        self.send("isready").await?;
        self.wait_for("readyok").await
    }

    /// Set the position using a list of UCI moves from the starting position.
    ///
    /// # Errors
    /// Returns an error if sending the command fails.
    pub async fn set_position(&mut self, moves: &[String]) -> Result<()> {
        let command = if moves.is_empty() {
            "position startpos".to_string()
        } else {
            format!("position startpos moves {}", moves.join(" "))
        };
        self.send(&command).await
    }

    /// Get the best move from the engine with a time limit.
    ///
    /// # Errors
    /// Returns an error if the engine fails to respond or returns an invalid move.
    pub async fn get_best_move(&mut self, movetime_ms: u64) -> Result<String> {
        self.send(&format!("go movetime {movetime_ms}")).await?;

        let mut line = String::new();
        loop {
            line.clear();
            let bytes_read = self.stdout.read_line(&mut line).await?;
            if bytes_read == 0 {
                return Err(eyre!("Engine {} closed unexpectedly", self.name));
            }
            let trimmed = line.trim();
            tracing::trace!(engine = %self.name, response = %trimmed, "Received");

            if let Some(rest) = trimmed.strip_prefix("bestmove ") {
                // bestmove format: "bestmove e2e4" or "bestmove e2e4 ponder d7d5"
                let best_move = rest.split_whitespace().next().context("Empty bestmove")?;
                return Ok(best_move.to_string());
            }
        }
    }

    /// Quit the engine gracefully.
    pub async fn quit(&mut self) -> Result<()> {
        self.send("quit").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stockfish_uci_init() {
        let mut engine = UciEngine::new("stockfish", "stockfish")
            .await
            .expect("Failed to init stockfish");
        engine.quit().await.expect("Failed to quit stockfish");
    }

    #[tokio::test]
    async fn test_reckless_uci_init() {
        let mut engine = UciEngine::new("reckless", "reckless")
            .await
            .expect("Failed to init reckless");
        engine.quit().await.expect("Failed to quit reckless");
    }

    #[tokio::test]
    async fn test_stockfish_new_game() {
        let mut engine = UciEngine::new("stockfish", "stockfish")
            .await
            .expect("Failed to init stockfish");
        engine.new_game().await.expect("Failed new_game");
        engine.quit().await.expect("Failed to quit");
    }

    #[tokio::test]
    async fn test_stockfish_get_move() {
        let mut engine = UciEngine::new("stockfish", "stockfish")
            .await
            .expect("Failed to init stockfish");
        engine.new_game().await.expect("Failed new_game");
        engine.set_position(&[]).await.expect("Failed set_position");
        let best_move = engine.get_best_move(100).await.expect("Failed to get move");
        assert!(!best_move.is_empty());
        // Valid UCI move format: 4-5 chars like e2e4 or e7e8q
        assert!(best_move.len() >= 4 && best_move.len() <= 5);
        engine.quit().await.expect("Failed to quit");
    }

    #[tokio::test]
    async fn test_reckless_get_move() {
        let mut engine = UciEngine::new("reckless", "reckless")
            .await
            .expect("Failed to init reckless");
        engine.new_game().await.expect("Failed new_game");
        engine.set_position(&[]).await.expect("Failed set_position");
        let best_move = engine.get_best_move(100).await.expect("Failed to get move");
        assert!(!best_move.is_empty());
        assert!(best_move.len() >= 4 && best_move.len() <= 5);
        engine.quit().await.expect("Failed to quit");
    }
}
