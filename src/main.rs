//! Reckless vs Stockfish - Chess Engine Battle
//!
//! Runs games between Stockfish and Reckless chess engines via UCI protocol.

use clap::Parser;
use color_eyre::eyre::Result;
use game::{GameResult, GameRunner};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;
use tracing_error::ErrorLayer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

mod engine;
mod game;

/// Run chess engine matches between Stockfish and Reckless
#[derive(Parser, Debug, Clone)]
#[command(version, about)]
struct Args {
    /// Number of games to play
    #[arg(short, long, default_value = "1000000")]
    games: u64,

    /// Path to stockfish engine
    #[arg(long, default_value = "stockfish")]
    stockfish_path: String,

    /// Path to reckless engine
    #[arg(long, default_value = "reckless")]
    reckless_path: String,

    /// Time limit per move in milliseconds
    #[arg(long, default_value = "100")]
    movetime_ms: u64,

    /// Maximum moves per game before declaring a draw
    #[arg(long, default_value = "500")]
    max_moves: u32,

    /// Number of parallel workers (engine pairs)
    #[arg(short, long, default_value = "12")]
    workers: usize,
}

/// Statistics for a match (thread-safe).
#[derive(Debug, Default)]
struct MatchStats {
    stockfish_wins: AtomicU64,
    reckless_wins: AtomicU64,
    draws: AtomicU64,
    stockfish_white_wins: AtomicU64,
    stockfish_black_wins: AtomicU64,
    reckless_white_wins: AtomicU64,
    reckless_black_wins: AtomicU64,
    games_completed: AtomicU64,
}

impl MatchStats {
    fn record(&self, result: GameResult, stockfish_is_white: bool) {
        match result {
            GameResult::WhiteWins => {
                if stockfish_is_white {
                    self.stockfish_wins.fetch_add(1, Ordering::Relaxed);
                    self.stockfish_white_wins.fetch_add(1, Ordering::Relaxed);
                } else {
                    self.reckless_wins.fetch_add(1, Ordering::Relaxed);
                    self.reckless_white_wins.fetch_add(1, Ordering::Relaxed);
                }
            }
            GameResult::BlackWins => {
                if stockfish_is_white {
                    self.reckless_wins.fetch_add(1, Ordering::Relaxed);
                    self.reckless_black_wins.fetch_add(1, Ordering::Relaxed);
                } else {
                    self.stockfish_wins.fetch_add(1, Ordering::Relaxed);
                    self.stockfish_black_wins.fetch_add(1, Ordering::Relaxed);
                }
            }
            GameResult::Draw => {
                self.draws.fetch_add(1, Ordering::Relaxed);
            }
        }
        self.games_completed.fetch_add(1, Ordering::Relaxed);
    }

    fn total_games(&self) -> u64 {
        self.games_completed.load(Ordering::Relaxed)
    }

    #[allow(clippy::cast_precision_loss)]
    fn print_summary(&self) {
        let total = self.total_games();
        let stockfish_wins = self.stockfish_wins.load(Ordering::Relaxed);
        let reckless_wins = self.reckless_wins.load(Ordering::Relaxed);
        let draws = self.draws.load(Ordering::Relaxed);

        if total == 0 {
            tracing::info!("No games played");
            return;
        }

        let stockfish_pct = (stockfish_wins as f64 / total as f64) * 100.0;
        let reckless_pct = (reckless_wins as f64 / total as f64) * 100.0;
        let draw_pct = (draws as f64 / total as f64) * 100.0;

        tracing::info!("=== Match Results ===");
        tracing::info!(
            "Total games: {total}, Stockfish: {stockfish_wins} ({stockfish_pct:.1}%), Reckless: {reckless_wins} ({reckless_pct:.1}%), Draws: {draws} ({draw_pct:.1}%)"
        );
        tracing::info!(
            "Stockfish as White: {} wins, as Black: {} wins",
            self.stockfish_white_wins.load(Ordering::Relaxed),
            self.stockfish_black_wins.load(Ordering::Relaxed)
        );
        tracing::info!(
            "Reckless as White: {} wins, as Black: {} wins",
            self.reckless_white_wins.load(Ordering::Relaxed),
            self.reckless_black_wins.load(Ordering::Relaxed)
        );
    }

    fn print_progress(&self) {
        let total = self.total_games();
        let stockfish_wins = self.stockfish_wins.load(Ordering::Relaxed);
        let reckless_wins = self.reckless_wins.load(Ordering::Relaxed);
        let draws = self.draws.load(Ordering::Relaxed);

        tracing::info!(
            games = total,
            stockfish = stockfish_wins,
            reckless = reckless_wins,
            draws = draws,
            "Progress"
        );
    }
}

/// Message sent from workers to aggregator.
struct GameCompleted {
    result: GameResult,
    stockfish_is_white: bool,
}

/// Run a worker that plays games continuously.
async fn run_worker(
    worker_id: usize,
    args: Args,
    game_counter: Arc<AtomicU64>,
    total_games: u64,
    tx: mpsc::Sender<GameCompleted>,
) -> Result<()> {
    use crate::engine::UciEngine;

    // Each worker has its own engine pair
    let mut stockfish =
        UciEngine::new(&args.stockfish_path, &format!("stockfish-{worker_id}")).await?;
    let mut reckless =
        UciEngine::new(&args.reckless_path, &format!("reckless-{worker_id}")).await?;

    let runner = GameRunner::new(args.movetime_ms, args.max_moves);

    loop {
        // Atomically claim a game number
        let game_num = game_counter.fetch_add(1, Ordering::Relaxed);
        if game_num >= total_games {
            break;
        }

        // Alternate colors based on game number
        let stockfish_is_white = game_num % 2 == 0;

        let result = if stockfish_is_white {
            runner.play_game(&mut stockfish, &mut reckless).await
        } else {
            runner.play_game(&mut reckless, &mut stockfish).await
        };

        match result {
            Ok(result) => {
                if tx
                    .send(GameCompleted {
                        result,
                        stockfish_is_white,
                    })
                    .await
                    .is_err()
                {
                    // Receiver dropped, stop
                    break;
                }
            }
            Err(e) => {
                tracing::warn!(worker = worker_id, error = %e, "Game failed, restarting engines");
                // Restart engines on failure
                stockfish.quit().await.ok();
                reckless.quit().await.ok();
                stockfish =
                    UciEngine::new(&args.stockfish_path, &format!("stockfish-{worker_id}")).await?;
                reckless =
                    UciEngine::new(&args.reckless_path, &format!("reckless-{worker_id}")).await?;
            }
        }
    }

    // Cleanup
    stockfish.quit().await.ok();
    reckless.quit().await.ok();

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .with(ErrorLayer::default())
        .init();

    color_eyre::install()?;

    let args = Args::parse();
    tracing::info!(?args, "Starting chess engine battle");

    let stats = Arc::new(MatchStats::default());
    let game_counter = Arc::new(AtomicU64::new(0));
    let total_games = args.games;

    // Channel for workers to report completed games
    let (tx, mut rx) = mpsc::channel::<GameCompleted>(args.workers * 2);

    // Spawn workers
    let mut worker_handles = Vec::new();
    for worker_id in 0..args.workers {
        let args_clone = args.clone();
        let game_counter_clone = Arc::clone(&game_counter);
        let tx_clone = tx.clone();

        let handle = tokio::spawn(async move {
            if let Err(e) = run_worker(
                worker_id,
                args_clone,
                game_counter_clone,
                total_games,
                tx_clone,
            )
            .await
            {
                tracing::error!(worker = worker_id, error = %e, "Worker failed");
            }
        });
        worker_handles.push(handle);
    }

    // Drop our sender so rx will end when all workers are done
    drop(tx);

    // Progress reporting task
    let stats_clone = Arc::clone(&stats);
    let progress_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
        loop {
            interval.tick().await;
            stats_clone.print_progress();
            if stats_clone.total_games() >= total_games {
                break;
            }
        }
    });

    // Collect results from workers
    while let Some(msg) = rx.recv().await {
        stats.record(msg.result, msg.stockfish_is_white);
    }

    // Cancel progress reporter
    progress_handle.abort();

    // Wait for all workers to finish
    for handle in worker_handles {
        handle.await.ok();
    }

    stats.print_summary();

    Ok(())
}
