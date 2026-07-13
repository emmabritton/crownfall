//! A minimax (negamax) game-playing AI for local/offline vs-computer play.
use crate::{BOARD_LENGTH, Cell, Game, GameState, PieceKind, PlayerAction, PlayerKind};
use alloc::vec::Vec;

/// Manhattan distance is at most 2*(BOARD_LENGTH-1) (opposite corners);
/// subtracting it from this yields a "closer is bigger" score.
const MAX_DISTANCE: i32 = 2 * (BOARD_LENGTH as i32 - 1);
const VICTORY_SCORE: i32 = 1000000;

/// How many plies the AI searches ahead. Higher sees further into forced
/// sequences at the cost of move time (7x7 board, ~9 pieces/side keeps the
/// branching factor low enough for `VeryHard` to still respond quickly).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
    VeryHard,
}

impl Difficulty {
    pub const fn depth(self) -> u8 {
        match self {
            Difficulty::Easy => 1,
            Difficulty::Medium => 2,
            Difficulty::Hard => 3,
            Difficulty::VeryHard => 4,
        }
    }
}

/// Shapes *how* the AI weighs a position, independent of how far ahead it
/// looks. Implemented as a symmetric scaling of the existing evaluation
/// terms (never an asymmetric bonus for one side's advance vs. the other's
/// defense) so `evaluate(game, player) == -evaluate(game, player.opposite())`
/// still holds — negamax's `-negamax(...)` recursion depends on that.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Personality {
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

impl Personality {
    const fn weights(self) -> Weights {
        match self {
            Personality::Defensive => Weights {
                crown: 1000,
                knight: 35,
                spy: 25,
                mobility: 1,
                crown_proximity: 1,
            },
            Personality::Balanced => Weights {
                crown: 1000,
                knight: 30,
                spy: 20,
                mobility: 1,
                crown_proximity: 2,
            },
            Personality::Aggressive => Weights {
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
    game: &Game,
    player: PlayerKind,
    depth: u8,
    personality: Personality,
) -> Option<PlayerAction> {
    let moves = legal_moves(game, player);
    let mut best: Option<PlayerAction> = None;
    let mut best_score = i32::MIN;
    let mut alpha = i32::MIN + 1;
    let beta = i32::MAX - 1;
    for action in moves {
        let Ok((next, _)) = game.clone().handle_player_action(action.clone()) else {
            continue;
        };
        let score = -negamax(
            &next,
            player.opposite(),
            depth.saturating_sub(1),
            personality,
            -beta,
            -alpha,
        );
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
    game: &Game,
    player: PlayerKind,
    depth: u8,
    personality: Personality,
    mut alpha: i32,
    beta: i32,
) -> i32 {
    match game.state {
        GameState::Victory(winner) => {
            return if winner == player {
                VICTORY_SCORE
            } else {
                -VICTORY_SCORE
            };
        }
        GameState::Draw(_) => return 0,
        GameState::Playing(_) => {}
    }
    if depth == 0 {
        return evaluate(game, player, personality);
    }

    let moves = legal_moves(game, player);
    if moves.is_empty() {
        return evaluate(game, player, personality);
    }

    let mut best = i32::MIN + 1;
    for action in moves {
        let Ok((next, _)) = game.clone().handle_player_action(action) else {
            continue;
        };
        let score = -negamax(&next, player.opposite(), depth - 1, personality, -beta, -alpha);
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

fn manhattan_distance(a: Cell, b: Cell) -> i32 {
    let (ax, ay) = a.to_coord();
    let (bx, by) = b.to_coord();
    (ax as i32 - bx as i32).abs() + (ay as i32 - by as i32).abs()
}

fn crown_cell(game: &Game, player: PlayerKind) -> Option<Cell> {
    game.board
        .cells
        .iter()
        .enumerate()
        .find_map(|(index, piece)| {
            let piece = (*piece)?;
            (piece.player == player && piece.kind == PieceKind::Crown)
                .then(|| Cell::new_index(index))
        })
}

/// Sum, over every non-Crown piece owned by `player`, of how close that
/// piece is to `target`'s Crown — higher when `player`'s pieces are massed
/// nearer the target's Crown. 0 if the target has no Crown on the board
/// (already captured, so proximity to it is meaningless).
fn crown_proximity(game: &Game, player: PlayerKind, target: PlayerKind) -> i32 {
    let Some(enemy_crown) = crown_cell(game, target) else {
        return 0;
    };
    game.board
        .cells
        .iter()
        .enumerate()
        .filter_map(|(index, piece)| {
            let piece = (*piece)?;
            (piece.player == player && piece.kind != PieceKind::Crown)
                .then(|| MAX_DISTANCE - manhattan_distance(Cell::new_index(index), enemy_crown))
        })
        .sum()
}

fn evaluate(game: &Game, player: PlayerKind, personality: Personality) -> i32 {
    let weights = personality.weights();
    let mut score = 0;
    for piece in game.board.cells.iter().flatten() {
        let value = match piece.kind {
            PieceKind::Crown => weights.crown,
            PieceKind::Knight => weights.knight,
            PieceKind::Spy => weights.spy,
        };
        score += if piece.player == player {
            value
        } else {
            -value
        };
    }

    let mobility =
        legal_moves(game, player).len() as i32 - legal_moves(game, player.opposite()).len() as i32;
    score += weights.mobility * mobility;

    // Advancing on the enemy Crown is rewarded; the enemy advancing on ours is
    // penalized the same way, keeping the term zero-sum for negamax.
    let proximity = crown_proximity(game, player, player.opposite())
        - crown_proximity(game, player.opposite(), player);
    score + weights.crown_proximity * proximity
}
