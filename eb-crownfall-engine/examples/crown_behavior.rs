//! Analysis harness: how do different difficulties/personalities actually
//! use their Crown - does it ever move, does it advance toward the enemy,
//! does it ever act as a Knight-Capture pincer partner? Self-play only
//! (negamax vs itself, symmetric difficulty/personality per game), reporting
//! per-(personality, difficulty) aggregates across both sides. Run with:
//!   cargo run --package eb-crownfall-engine --example crown_behavior --release
use eb_crownfall_engine::ai::{self, CrownfallDifficulty, CrownfallPersonality};
use eb_crownfall_engine::*;

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn gen_range(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

fn legal_moves(game: &CrownfallGame, player: CrownfallPlayerKind) -> Vec<CrownfallPlayerAction> {
    let mut moves = Vec::new();
    for index in 0..game.board.cells().len() {
        if let Some(piece) = game.board.cells()[index]
            && piece.player() == player
        {
            let from = CrownfallBoardCell::new_index(index);
            for to in game.board.get_valid_destinations_for(from, game.rules) {
                moves.push(CrownfallPlayerAction::Move { player, from, to });
            }
        }
    }
    moves
}

fn current_player(game: &CrownfallGame) -> Option<CrownfallPlayerKind> {
    match &game.state {
        CrownfallGameState::Playing(CrownfallPlayState::WaitingForInput { player }) => {
            Some(*player)
        }
        CrownfallGameState::Playing(CrownfallPlayState::MustRemoveKnight { player, .. }) => {
            Some(*player)
        }
        CrownfallGameState::Victory(..) | CrownfallGameState::Draw(_) => None,
    }
}

/// Per-player Crown-behavior accumulator for one game.
#[derive(Clone, Copy, Default)]
struct CrownTrack {
    start: Option<(usize, usize)>,
    moves_made: u32,
    captures_participated: u32,
    max_forward: i32,
    final_pos: Option<(usize, usize)>,
    captured: bool,
}

impl CrownTrack {
    /// Forward progress for `player`: White advances toward lower y, Black
    /// toward higher y (matches the Knight forward-arc orientation used
    /// throughout the engine - see `tables::build_arcs`'s `forward_down`).
    fn forward_progress(player: CrownfallPlayerKind, start: (usize, usize), now: (usize, usize)) -> i32 {
        match player {
            CrownfallPlayerKind::White => start.1 as i32 - now.1 as i32,
            CrownfallPlayerKind::Black => now.1 as i32 - start.1 as i32,
        }
    }
}

struct GameCrownStats {
    white: CrownTrack,
    black: CrownTrack,
}

fn find_crown(game: &CrownfallGame, player: CrownfallPlayerKind) -> Option<(usize, usize)> {
    let variant = game.board.variant();
    game.board.cells().iter().enumerate().find_map(|(i, cell)| {
        let piece = (*cell)?;
        (piece.player() == player && piece.kind() == CrownfallPieceKind::Crown)
            .then(|| CrownfallBoardCell::new_index(i).to_coord(variant))
    })
}

fn play_game(
    seed: u64,
    depth: u8,
    personality: CrownfallPersonality,
    random_opening_plies: usize,
) -> GameCrownStats {
    let mut game = CrownfallGame::default();
    let mut rng = Rng::new(seed);
    let mut turns = 0usize;

    let mut white = CrownTrack {
        start: find_crown(&game, CrownfallPlayerKind::White),
        ..Default::default()
    };
    let mut black = CrownTrack {
        start: find_crown(&game, CrownfallPlayerKind::Black),
        ..Default::default()
    };

    loop {
        let Some(player) = current_player(&game) else {
            break;
        };

        let action = if turns < random_opening_plies {
            let moves = legal_moves(&game, player);
            if moves.is_empty() {
                break;
            }
            moves[rng.gen_range(moves.len())]
        } else {
            match ai::best_move(&game, player, depth, personality) {
                Some(action) => action,
                None => break,
            }
        };

        let CrownfallPlayerAction::Move { from, to, .. } = action else {
            unreachable!("legal_moves/best_move only ever produce Move actions here")
        };
        let moved_piece = game.board.cells()[from.to_index()];
        let is_crown_move =
            matches!(moved_piece, Some(p) if p.kind() == CrownfallPieceKind::Crown);

        let (next, turn_result) = game
            .clone()
            .handle_player_action(action)
            .expect("AI/random move generator only produces legal moves");
        turns += 1;

        if is_crown_move {
            let track = match player {
                CrownfallPlayerKind::White => &mut white,
                CrownfallPlayerKind::Black => &mut black,
            };
            track.moves_made += 1;
            if let Some(start) = track.start {
                let now = to.to_coord(game.board.variant());
                track.max_forward = track.max_forward.max(CrownTrack::forward_progress(player, start, now));
            }
            if matches!(turn_result, Some(CrownfallTurnResult::Capture { .. })) {
                track.captures_participated += 1;
            }
        }

        // Track whether either Crown just got captured (the other side's
        // capture removed it) - `removed` is the target cell, so check
        // whether the pre-move occupant there was a Crown.
        if let Some(CrownfallTurnResult::Capture { removed, .. }) = turn_result {
            if let Some(piece) = game.board.cells()[removed.to_index()]
                && piece.kind() == CrownfallPieceKind::Crown
            {
                let track = match piece.player() {
                    CrownfallPlayerKind::White => &mut white,
                    CrownfallPlayerKind::Black => &mut black,
                };
                track.captured = true;
            }
        }

        game = next;

        if matches!(
            game.state,
            CrownfallGameState::Victory(..) | CrownfallGameState::Draw(_)
        ) {
            break;
        }
    }

    white.final_pos = find_crown(&game, CrownfallPlayerKind::White);
    black.final_pos = find_crown(&game, CrownfallPlayerKind::Black);

    GameCrownStats { white, black }
}

struct Agg {
    games: usize,
    crowns_tracked: usize,
    crowns_that_ever_moved: usize,
    total_moves: u64,
    crowns_that_captured: usize,
    total_captures: u64,
    crowns_that_advanced: usize,
    total_max_forward: i64,
}

impl Agg {
    fn new() -> Self {
        Agg {
            games: 0,
            crowns_tracked: 0,
            crowns_that_ever_moved: 0,
            total_moves: 0,
            crowns_that_captured: 0,
            total_captures: 0,
            crowns_that_advanced: 0,
            total_max_forward: 0,
        }
    }

    fn record(&mut self, track: &CrownTrack) {
        self.crowns_tracked += 1;
        if track.moves_made > 0 {
            self.crowns_that_ever_moved += 1;
        }
        self.total_moves += track.moves_made as u64;
        if track.captures_participated > 0 {
            self.crowns_that_captured += 1;
        }
        self.total_captures += track.captures_participated as u64;
        if track.max_forward > 0 {
            self.crowns_that_advanced += 1;
        }
        self.total_max_forward += track.max_forward as i64;
    }

    fn print(&self, label: &str) {
        let n = self.crowns_tracked.max(1) as f64;
        println!(
            "  {label}: moved in {:.1}% of games (avg {:.2} Crown-moves/game), \
captured/pincered in {:.1}% of games ({:.2} avg), advanced past its start row \
in {:.1}% of games (avg max forward {:.2} tiles) [{} crown-sides over {} games]",
            100.0 * self.crowns_that_ever_moved as f64 / n,
            self.total_moves as f64 / n,
            100.0 * self.crowns_that_captured as f64 / n,
            self.total_captures as f64 / n,
            100.0 * self.crowns_that_advanced as f64 / n,
            self.total_max_forward as f64 / n,
            self.crowns_tracked,
            self.games,
        );
    }
}

fn main() {
    println!("Crownfall Crown-usage analysis (negamax AI vs itself, symmetric matchups)\n");

    let difficulties = [
        CrownfallDifficulty::Easy,
        CrownfallDifficulty::Medium,
        CrownfallDifficulty::Hard,
        CrownfallDifficulty::VeryHard,
    ];
    let personalities = [
        CrownfallPersonality::Defensive,
        CrownfallPersonality::Balanced,
        CrownfallPersonality::Aggressive,
    ];

    for personality in personalities {
        println!("== {personality:?} ==");
        for difficulty in difficulties {
            let mut agg = Agg::new();
            let depth = difficulty.depth();
            for seed in 0..40u64 {
                let stats = play_game(seed + 1, depth, personality, 4);
                agg.games += 1;
                agg.record(&stats.white);
                agg.record(&stats.black);
            }
            agg.print(&format!("{difficulty:?} (depth {depth})"));
        }
        println!();
    }
}
