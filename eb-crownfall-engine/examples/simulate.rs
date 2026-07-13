//! Self-play harness for balance/viability analysis.
//!
//! Runs the existing negamax AI against itself many times and reports
//! win rates, how games end, and game length. Run with:
//!   cargo run --package game --example simulate --release

use eb_crownfall_engine::ai;
use eb_crownfall_engine::{
    Cell, DrawReason, Game, GameState, PlayState, PlayerAction, PlayerKind, TurnResult,
};

/// Small deterministic xorshift64 PRNG so results are reproducible from a seed
/// without pulling in a `rand` dependency for a one-off analysis tool.
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

fn legal_moves(game: &Game, player: PlayerKind) -> Vec<PlayerAction> {
    let mut moves = Vec::new();
    for index in 0..game.board.cells.len() {
        if let Some(piece) = game.board.cells[index]
            && piece.player == player
        {
            let from = Cell::new_index(index);
            for to in game.board.get_valid_destinations_for(from) {
                moves.push(PlayerAction::Move { player, from, to });
            }
        }
    }
    moves
}

fn current_player(game: &Game) -> Option<PlayerKind> {
    match &game.state {
        GameState::Playing(PlayState::WaitingForInput { player }) => Some(*player),
        GameState::Playing(PlayState::MustRemoveKnight { player, .. }) => Some(*player),
        GameState::Victory(_) | GameState::Draw(_) => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reason {
    CrownCapture,
    Attrition,
    Repetition,
    NoProgress,
    TurnLimit,
    MutualKnightExhaustion,
}

impl From<DrawReason> for Reason {
    fn from(reason: DrawReason) -> Self {
        match reason {
            DrawReason::Repetition => Reason::Repetition,
            DrawReason::NoProgress => Reason::NoProgress,
            DrawReason::TurnLimit => Reason::TurnLimit,
            DrawReason::MutualKnightExhaustion => Reason::MutualKnightExhaustion,
        }
    }
}

struct GameResult {
    winner: Option<PlayerKind>,
    reason: Reason,
    turns: usize,
}

fn play_game(
    seed: u64,
    white_depth: u8,
    black_depth: u8,
    random_opening_plies: usize,
) -> GameResult {
    let mut game = Game::default();
    let mut rng = Rng::new(seed);
    let mut turns = 0usize;

    loop {
        let Some(player) = current_player(&game) else {
            let (winner, reason) = match game.state {
                GameState::Victory(w) => (Some(w), Reason::CrownCapture),
                GameState::Draw(reason) => (None, reason.into()),
                GameState::Playing(_) => {
                    unreachable!("current_player only returns None for Victory/Draw")
                }
            };
            return GameResult {
                winner,
                reason,
                turns,
            };
        };

        let action = if turns < random_opening_plies {
            let moves = legal_moves(&game, player);
            if moves.is_empty() {
                return GameResult {
                    winner: Some(player.opposite()),
                    reason: Reason::Attrition,
                    turns,
                };
            }
            moves[rng.gen_range(moves.len())]
        } else {
            let depth = if player == PlayerKind::White {
                white_depth
            } else {
                black_depth
            };
            match ai::best_move(&game, player, depth, ai::Personality::Balanced) {
                Some(action) => action,
                None => {
                    return GameResult {
                        winner: Some(player.opposite()),
                        reason: Reason::Attrition,
                        turns,
                    };
                }
            }
        };

        let (next, turn_result) = game
            .clone()
            .handle_player_action(action)
            .expect("AI/random move generator only produces legal moves");
        turns += 1;

        if let GameState::Victory(winner) = next.state {
            let reason = match turn_result {
                Some(TurnResult::Victory { .. }) => Reason::CrownCapture,
                _ => Reason::Attrition,
            };
            return GameResult {
                winner: Some(winner),
                reason,
                turns,
            };
        }
        if let GameState::Draw(reason) = next.state {
            return GameResult {
                winner: None,
                reason: reason.into(),
                turns,
            };
        }

        game = next;
    }
}

struct BatchStats {
    label: String,
    games: usize,
    white_wins: usize,
    black_wins: usize,
    draws: usize,
    crown_captures: usize,
    attritions: usize,
    repetitions: usize,
    no_progress: usize,
    turn_limits: usize,
    mutual_knight_exhaustions: usize,
    total_turns: usize,
    min_turns: usize,
    max_turns: usize,
}

impl BatchStats {
    fn new(label: impl Into<String>) -> Self {
        BatchStats {
            label: label.into(),
            games: 0,
            white_wins: 0,
            black_wins: 0,
            draws: 0,
            crown_captures: 0,
            attritions: 0,
            repetitions: 0,
            no_progress: 0,
            turn_limits: 0,
            mutual_knight_exhaustions: 0,
            total_turns: 0,
            min_turns: usize::MAX,
            max_turns: 0,
        }
    }

    fn record(&mut self, result: &GameResult) {
        self.games += 1;
        self.total_turns += result.turns;
        self.min_turns = self.min_turns.min(result.turns);
        self.max_turns = self.max_turns.max(result.turns);
        match result.winner {
            Some(PlayerKind::White) => self.white_wins += 1,
            Some(PlayerKind::Black) => self.black_wins += 1,
            None => self.draws += 1,
        }
        match result.reason {
            Reason::CrownCapture => self.crown_captures += 1,
            Reason::Attrition => self.attritions += 1,
            Reason::Repetition => self.repetitions += 1,
            Reason::NoProgress => self.no_progress += 1,
            Reason::TurnLimit => self.turn_limits += 1,
            Reason::MutualKnightExhaustion => self.mutual_knight_exhaustions += 1,
        }
    }

    fn print(&self) {
        let g = self.games.max(1) as f64;
        println!("== {} ({} games) ==", self.label, self.games);
        println!(
            "  White wins: {} ({:.1}%)  Black wins: {} ({:.1}%)  Draws: {} ({:.1}%)",
            self.white_wins,
            100.0 * self.white_wins as f64 / g,
            self.black_wins,
            100.0 * self.black_wins as f64 / g,
            self.draws,
            100.0 * self.draws as f64 / g
        );
        println!(
            "  Ended by: crown capture {} ({:.1}%), attrition {} ({:.1}%), repetition {} ({:.1}%), no progress {} ({:.1}%), turn limit {} ({:.1}%), mutual knight exhaustion {} ({:.1}%)",
            self.crown_captures,
            100.0 * self.crown_captures as f64 / g,
            self.attritions,
            100.0 * self.attritions as f64 / g,
            self.repetitions,
            100.0 * self.repetitions as f64 / g,
            self.no_progress,
            100.0 * self.no_progress as f64 / g,
            self.turn_limits,
            100.0 * self.turn_limits as f64 / g,
            self.mutual_knight_exhaustions,
            100.0 * self.mutual_knight_exhaustions as f64 / g
        );
        println!(
            "  Turns: avg {:.1}, min {}, max {}",
            self.total_turns as f64 / g,
            self.min_turns,
            self.max_turns
        );
        println!();
    }
}

fn main() {
    println!("Crownfall self-play analysis (negamax AI vs itself)\n");

    // Batch A: symmetric strength, randomized 4-ply openings for variety.
    // Measures baseline first-move advantage and how games typically end.
    let mut symmetric = BatchStats::new("Symmetric depth 3 vs 3, randomized 4-ply openings");
    for seed in 0..200u64 {
        let result = play_game(seed + 1, 3, 3, 4);
        symmetric.record(&result);
    }
    symmetric.print();

    // Batch B: same but deeper (depth 4) to see if the imbalance shifts with strength.
    let mut symmetric_deep = BatchStats::new("Symmetric depth 4 vs 4, randomized 4-ply openings");
    for seed in 0..60u64 {
        let result = play_game(seed + 10_000, 4, 4, 4);
        symmetric_deep.record(&result);
    }
    symmetric_deep.print();

    // Batch C: does search depth matter? Fixed opening, asymmetric depths.
    let matchups: [(&str, u8, u8); 6] = [
        ("White depth 4 vs Black depth 2", 4, 2),
        ("White depth 2 vs Black depth 4", 2, 4),
        ("White depth 3 vs Black depth 1", 3, 1),
        ("White depth 1 vs Black depth 3", 1, 3),
        ("White depth 5 vs Black depth 3", 5, 3),
        ("White depth 3 vs Black depth 5", 3, 5),
    ];
    for (label, wd, bd) in matchups {
        let mut stats = BatchStats::new(label);
        for seed in 0..30u64 {
            let result = play_game(seed + 20_000, wd, bd, 4);
            stats.record(&result);
        }
        stats.print();
    }

    // Batch D: does more opening randomization (breaking the board's mirror
    // symmetry harder) reduce the repetition-draw rate seen above?
    for opening_plies in [8usize, 12, 20] {
        let mut stats = BatchStats::new(format!(
            "Symmetric depth 3 vs 3, randomized {opening_plies}-ply openings"
        ));
        for seed in 0..100u64 {
            let result = play_game(
                seed + 30_000 + opening_plies as u64 * 1000,
                3,
                3,
                opening_plies,
            );
            stats.record(&result);
        }
        stats.print();
    }
}
