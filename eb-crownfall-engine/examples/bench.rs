//! Temporary micro-benchmark for optimisation work: times full AI searches
//! from a few reproducible positions across board sizes.

use eb_crownfall_engine::ai::{CrownfallPersonality, CrownfallSearcher};
use eb_crownfall_engine::{CrownfallGame, CrownfallGameState, CrownfallRules};
use std::time::Instant;

fn drive(rules: CrownfallRules, plies: u32, depth: u8, label: &str) {
    let mut game = CrownfallGame::new(rules);
    // Persistent across moves, like the client's vs-AI loop.
    let mut searcher = CrownfallSearcher::new();
    let start = Instant::now();
    let mut moves_made = 0u32;
    for _ in 0..plies {
        let CrownfallGameState::Playing(play_state) = game.state else {
            break;
        };
        let player = play_state.player();
        let Some(action) = searcher.best_move(&game, player, depth, CrownfallPersonality::Balanced)
        else {
            break;
        };
        game.apply_action(action).expect("AI move must be legal");
        moves_made += 1;
    }
    let elapsed = start.elapsed();
    println!(
        "{label}: {moves_made} moves at depth {depth} in {:?} ({:?}/move)",
        elapsed,
        elapsed / moves_made.max(1)
    );
}

fn main() {
    for _ in 0..3 {
        drive(CrownfallRules::standard(), 30, 4, "standard d4");
        drive(CrownfallRules::grand(), 20, 4, "grand    d4");
        drive(CrownfallRules::mini(), 30, 5, "mini     d5");
        drive(
            CrownfallRules::standard_mandatory_capture(),
            20,
            4,
            "std-mc   d4",
        );
        println!("--");
    }
}
