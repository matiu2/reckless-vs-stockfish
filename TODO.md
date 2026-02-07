# Reckless vs Stockfish - Chess Engine Battle

## Goal
Run 1 million games between Stockfish and Reckless chess engines, alternating colors, to determine which engine performs better.

## Tasks

### Setup
- [x] Initialize Rust project
- [x] Add dependencies (UCI protocol, async runtime, etc.)

### Core Implementation
- [x] UCI protocol communication module
- [x] Engine process management (spawn, communicate, terminate)
- [x] Game state management
- [x] Move parsing and validation
- [x] Game result detection (checkmate, stalemate, draw conditions)

### Match Runner
- [x] Alternating color assignment
- [x] Game loop (get moves from each engine)
- [x] Result tracking and statistics
- [x] Progress reporting
- [ ] Resume capability (in case of crashes)

### Output
- [x] Statistics summary (wins, losses, draws per side)
- [ ] Optional: PGN output of games
- [ ] Optional: ELO estimation

## Current Status
- [x] Basic working version - can run games and track statistics
- [ ] TODO: Add resume capability for long runs
- [ ] TODO: Consider parallel games for faster completion
