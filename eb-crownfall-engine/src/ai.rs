//! A minimax (negamax) game-playing AI for local/offline vs-computer play.
//!
//! The search is allocation-free: legal moves live in fixed stack buffers,
//! and candidate positions are explored by mutating a single `Game` in place
//! (`Game::apply_action_quiet`) and rolling it back from a small snapshot —
//! the board is a plain `Copy` and the position history only ever grows, so
//! undo is a couple of stores plus a `Vec::truncate`. This matters on the
//! GBA's ARM7TDMI, where a per-node heap clone of the history would dwarf
//! the actual search work.
use crate::tables::{CELL_COUNT, DIST};
use crate::{
    BOARD_LENGTH, CrownfallBoardCell, CrownfallBoardState, CrownfallGame, CrownfallGameState,
    CrownfallPieceKind, CrownfallPlayerAction, CrownfallPlayerKind,
};

/// Manhattan distance is at most 2*(BOARD_LENGTH-1) (opposite corners);
/// subtracting it from this yields a "closer is bigger" score.
const MAX_DISTANCE: i32 = 2 * (BOARD_LENGTH as i32 - 1);
const VICTORY_SCORE: i32 = 1000000;

/// Upper bound on one side's legal moves: at most 10 pieces (1 Crown, 6
/// Knights, 3 Spies — pieces are only ever lost), each with at most 4
/// destinations.
const MAX_MOVES: usize = 40;

/// A side's legal moves as packed (from, to) cell indices — 2 bytes per move
/// on the stack, so recursion depth stays cheap on the GBA's small stack.
struct MoveList {
    moves: [(u8, u8); MAX_MOVES],
    len: usize,
}

/// Everything needed to roll a `Game` back after `apply_action_quiet`: the
/// history is append-only, so restoring it is just a truncate (which keeps
/// the Vec's capacity — no churn).
struct Undo {
    board: CrownfallBoardState,
    state: CrownfallGameState,
    moves_since_capture: u16,
    history_len: usize,
}

fn snapshot(game: &CrownfallGame) -> Undo {
    Undo {
        board: game.board,
        state: game.state,
        moves_since_capture: game.moves_since_capture,
        history_len: game.history.len(),
    }
}

fn restore(game: &mut CrownfallGame, undo: &Undo) {
    game.board = undo.board;
    game.state = undo.state;
    game.moves_since_capture = undo.moves_since_capture;
    game.history.truncate(undo.history_len);
}

/// How many plies the AI searches ahead. Higher sees further into forced
/// sequences at the cost of move time (7x7 board, ~9 pieces/side keeps the
/// branching factor low enough for `VeryHard` to still respond quickly).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CrownfallDifficulty {
    Easy,
    Medium,
    Hard,
    VeryHard,
}

impl CrownfallDifficulty {
    pub const fn depth(self) -> u8 {
        match self {
            CrownfallDifficulty::Easy => 1,
            CrownfallDifficulty::Medium => 2,
            CrownfallDifficulty::Hard => 3,
            CrownfallDifficulty::VeryHard => 4,
        }
    }
}

/// Shapes *how* the AI weighs a position, independent of how far ahead it
/// looks. Implemented as a symmetric scaling of the existing evaluation
/// terms (never an asymmetric bonus for one side's advance vs. the other's
/// defense) so `evaluate(game, player) == -evaluate(game, player.opposite())`
/// still holds — negamax's `-negamax(...)` recursion depends on that.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CrownfallPersonality {
    /// Values holding onto material over board position — trades and
    /// advances reluctantly, happy to sit back on a material edge.
    Defensive,
    Balanced,
    /// Discounts material a little and leans hard on mobility/advancement —
    /// trades and pushes forward readily to chase attacking positions.
    Aggressive,
}

#[derive(Clone, Copy, Debug)]
struct Weights {
    crown: i32,
    knight: i32,
    spy: i32,
    mobility: i32,
    crown_proximity: i32,
}

impl CrownfallPersonality {
    const fn weights(self) -> Weights {
        match self {
            CrownfallPersonality::Defensive => Weights {
                crown: 1000,
                knight: 35,
                spy: 25,
                mobility: 1,
                crown_proximity: 1,
            },
            CrownfallPersonality::Balanced => Weights {
                crown: 1000,
                knight: 30,
                spy: 20,
                mobility: 1,
                crown_proximity: 2,
            },
            CrownfallPersonality::Aggressive => Weights {
                crown: 1000,
                knight: 25,
                spy: 15,
                mobility: 2,
                crown_proximity: 4,
            },
        }
    }
}

/// Returns the best move for `player` in `game`, or `None` if they have no
/// legal moves (shouldn't happen given the attrition/crown-loss rules end
/// the game before a player is left immobile, but handled defensively).
/// `depth` is typically `Difficulty::depth()`, kept as a raw `u8` here so
/// analysis tools (see `examples/simulate.rs`) can probe arbitrary depths.
pub fn best_move(
    game: &CrownfallGame,
    player: CrownfallPlayerKind,
    depth: u8,
    personality: CrownfallPersonality,
) -> Option<CrownfallPlayerAction> {
    // The one clone of the whole search — every node below mutates this copy
    // in place and rolls it back.
    let mut game = game.clone();
    let moves = collect_moves(&game.board, player);
    let mut best: Option<CrownfallPlayerAction> = None;
    let mut best_score = i32::MIN;
    let mut alpha = i32::MIN + 1;
    let beta = i32::MAX - 1;
    for &(from, to) in &moves.moves[..moves.len] {
        let action = CrownfallPlayerAction::Move {
            player,
            from: CrownfallBoardCell::new_index(from as usize),
            to: CrownfallBoardCell::new_index(to as usize),
        };
        let undo = snapshot(&game);
        // `apply_action_quiet` leaves the game untouched on Err, so no
        // rollback is needed to skip the move.
        if game.apply_action_quiet(action).is_err() {
            continue;
        }
        let score = -negamax(
            &mut game,
            player.opposite(),
            depth.saturating_sub(1),
            personality,
            -beta,
            -alpha,
        );
        restore(&mut game, &undo);
        if best.is_none() || score > best_score {
            best_score = score;
            best = Some(action);
        }
        if score > alpha {
            alpha = score;
        }
    }
    best
}

fn negamax(
    game: &mut CrownfallGame,
    player: CrownfallPlayerKind,
    depth: u8,
    personality: CrownfallPersonality,
    mut alpha: i32,
    beta: i32,
) -> i32 {
    match game.state {
        CrownfallGameState::Victory(winner) => {
            return if winner == player {
                VICTORY_SCORE
            } else {
                -VICTORY_SCORE
            };
        }
        CrownfallGameState::Draw(_) => return 0,
        CrownfallGameState::Playing(_) => {}
    }
    if depth == 0 {
        return evaluate(game, player, personality);
    }

    let moves = collect_moves(&game.board, player);
    if moves.len == 0 {
        return evaluate(game, player, personality);
    }

    let mut best = i32::MIN + 1;
    for &(from, to) in &moves.moves[..moves.len] {
        let action = CrownfallPlayerAction::Move {
            player,
            from: CrownfallBoardCell::new_index(from as usize),
            to: CrownfallBoardCell::new_index(to as usize),
        };
        let undo = snapshot(game);
        if game.apply_action_quiet(action).is_err() {
            continue;
        }
        let score = -negamax(
            game,
            player.opposite(),
            depth - 1,
            personality,
            -beta,
            -alpha,
        );
        restore(game, &undo);
        if score > best {
            best = score;
        }
        if best > alpha {
            alpha = best;
        }
        if alpha >= beta {
            break;
        }
    }
    best
}

fn collect_moves(board: &CrownfallBoardState, player: CrownfallPlayerKind) -> MoveList {
    let mut list = MoveList {
        moves: [(0, 0); MAX_MOVES],
        len: 0,
    };
    for index in 0..CELL_COUNT {
        if let Some(piece) = board.cells[index]
            && piece.player == player
        {
            for &to in board.move_candidates(CrownfallBoardCell::new_index(index)) {
                if board.cells[to as usize].is_none() {
                    list.moves[list.len] = (index as u8, to);
                    list.len += 1;
                }
            }
        }
    }
    list
}

fn crown_index(board: &CrownfallBoardState, player: CrownfallPlayerKind) -> Option<usize> {
    board.cells.iter().position(|piece| {
        matches!(piece, Some(piece) if piece.player == player && piece.kind == CrownfallPieceKind::Crown)
    })
}

/// Static evaluation from `player`'s point of view. Three symmetric terms:
/// material (piece values), mobility (legal-move-count difference), and
/// crown proximity (each non-Crown piece scores `MAX_DISTANCE` minus its
/// `tables::DIST` distance to the enemy Crown, rewarding massing pieces near
/// it — the enemy's advance on ours counts against us the same way, keeping
/// the term zero-sum for negamax). Every term is per-piece additive, so the
/// whole thing is a single pass over the board (evaluate runs at every leaf
/// of the search — this is the hottest loop in the crate after
/// `apply_action` itself).
fn evaluate(
    game: &CrownfallGame,
    player: CrownfallPlayerKind,
    personality: CrownfallPersonality,
) -> i32 {
    let weights = personality.weights();
    let board = &game.board;
    // Proximity to a Crown that's already been captured is meaningless, so a
    // missing Crown zeroes that side's proximity term.
    let own_crown = crown_index(board, player);
    let enemy_crown = crown_index(board, player.opposite());

    let mut material = 0;
    let mut mobility = 0;
    let mut proximity = 0;
    for (index, &cell) in board.cells.iter().enumerate() {
        let Some(piece) = cell else {
            continue;
        };
        let mine = piece.player == player;
        let sign = if mine { 1 } else { -1 };

        material += sign
            * match piece.kind {
                CrownfallPieceKind::Crown => weights.crown,
                CrownfallPieceKind::Knight => weights.knight,
                CrownfallPieceKind::Spy => weights.spy,
            };

        for &to in board.move_candidates(CrownfallBoardCell::new_index(index)) {
            if board.cells[to as usize].is_none() {
                mobility += sign;
            }
        }

        if piece.kind != CrownfallPieceKind::Crown {
            let target_crown = if mine { enemy_crown } else { own_crown };
            if let Some(crown) = target_crown {
                proximity += sign * (MAX_DISTANCE - DIST[crown][index] as i32);
            }
        }
    }

    material + weights.mobility * mobility + weights.crown_proximity * proximity
}
