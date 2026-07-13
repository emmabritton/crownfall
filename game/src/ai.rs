//! A minimax (negamax) game-playing AI for local/offline vs-computer play.
use crate::{Cell, Game, GameState, PieceKind, PlayerAction, PlayerKind};

const CROWN_VALUE: i32 = 1000;
const KNIGHT_VALUE: i32 = 30;
const SPY_VALUE: i32 = 20;
const MOBILITY_WEIGHT: i32 = 1;
const VICTORY_SCORE: i32 = 1000000;

/// Recommended search depth for a reasonably strong opponent that still
/// responds quickly (7x7 board, ~9 pieces/side keeps branching factor low).
pub const DEFAULT_DEPTH: u8 = 3;

/// Returns the best move for `player` in `game`, or `None` if they have no
/// legal moves (shouldn't happen given the attrition/crown-loss rules end
/// the game before a player is left immobile, but handled defensively).
pub fn best_move(game: &Game, player: PlayerKind, depth: u8) -> Option<PlayerAction> {
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

fn negamax(game: &Game, player: PlayerKind, depth: u8, mut alpha: i32, beta: i32) -> i32 {
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
        return evaluate(game, player);
    }

    let moves = legal_moves(game, player);
    if moves.is_empty() {
        return evaluate(game, player);
    }

    let mut best = i32::MIN + 1;
    for action in moves {
        let Ok((next, _)) = game.clone().handle_player_action(action) else {
            continue;
        };
        let score = -negamax(&next, player.opposite(), depth - 1, -beta, -alpha);
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

fn evaluate(game: &Game, player: PlayerKind) -> i32 {
    let mut score = 0;
    for piece in game.board.cells.iter().flatten() {
        let value = match piece.kind {
            PieceKind::Crown => CROWN_VALUE,
            PieceKind::Knight => KNIGHT_VALUE,
            PieceKind::Spy => SPY_VALUE,
        };
        score += if piece.player == player {
            value
        } else {
            -value
        };
    }

    let mobility =
        legal_moves(game, player).len() as i32 - legal_moves(game, player.opposite()).len() as i32;
    score + MOBILITY_WEIGHT * mobility
}
