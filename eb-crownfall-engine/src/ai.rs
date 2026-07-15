//! A minimax (negamax) game-playing AI for local/offline vs-computer play.
//!
//! The search is allocation-free: legal moves live in fixed stack buffers,
//! and candidate positions are explored by mutating a single `Game` in place
//! (`Game::apply_action_quiet`) and rolling it back from a small snapshot —
//! the board is a plain `Copy` and the position history only ever grows, so
//! undo is a couple of stores plus a `Vec::truncate`. This matters on the
//! GBA's ARM7TDMI, where a per-node heap clone of the history would dwarf
//! the actual search work.
use crate::{tables, CrownfallRuleset};
use crate::{
    CrownfallBoardCell, CrownfallBoardState, CrownfallGame, CrownfallGameState, CrownfallPieceKind,
    CrownfallPlayerAction, CrownfallPlayerKind, CrownfallRules,
};

const VICTORY_SCORE: i32 = 1000000;

/// Upper bound on one side's legal moves across every board size: Grand's
/// 14 pieces/side (8 Knight + 3 Spy + 1 Crown + 2 Archer), each with at most
/// 4 destinations, is the largest - a flat stack array sized for the worst
/// case costs nothing extra on the smaller boards.
const MAX_MOVES: usize = 64;

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
/// sequences at the cost of move time (the branching factor stays low
/// enough on every supported board size for `VeryHard` to still respond
/// quickly).
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
    archer: i32,
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
                archer: 25,
                mobility: 1,
                crown_proximity: 1,
            },
            CrownfallPersonality::Balanced => Weights {
                crown: 1000,
                knight: 30,
                spy: 20,
                archer: 20,
                mobility: 1,
                crown_proximity: 2,
            },
            CrownfallPersonality::Aggressive => Weights {
                crown: 1000,
                knight: 25,
                spy: 15,
                archer: 15,
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
    let rules = game.rules;
    let mut moves = collect_moves(&game.board, player, rules);
    // Iterative deepening: search depth 1, 2, ... up to the requested depth,
    // rotating each iteration's best root move to the front for the next.
    // The shallow passes cost a small fraction of the final one and their
    // move ordering makes alpha-beta cut far more of the deep tree — the net
    // is faster than a single full-depth pass. `depth == 0` has always meant
    // "evaluate each move's immediate result", i.e. the same tree as 1.
    let mut best: Option<CrownfallPlayerAction> = None;
    for iteration_depth in 1..=depth.max(1) {
        let mut best_slot: Option<usize> = None;
        let mut best_score = i32::MIN;
        let mut alpha = i32::MIN + 1;
        let beta = i32::MAX - 1;
        for slot in 0..moves.len {
            let (from, to) = moves.moves[slot];
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
                iteration_depth - 1,
                personality,
                -beta,
                -alpha,
            );
            restore(&mut game, &undo);
            if best_slot.is_none() || score > best_score {
                best_score = score;
                best_slot = Some(slot);
                best = Some(action);
            }
            if score > alpha {
                alpha = score;
            }
        }
        let Some(slot) = best_slot else {
            // Every root move was rejected — deeper passes can't differ.
            break;
        };
        moves.moves[..=slot].rotate_right(1);
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

    let moves = collect_moves(&game.board, player, game.rules);
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

/// Legal moves for `player`, honoring `rules.mandatory_capture`: when set
/// and at least one capturing move exists anywhere on the board, only
/// capturing moves are included.
fn collect_moves(
    board: &CrownfallBoardState,
    player: CrownfallPlayerKind,
    rules: CrownfallRules,
) -> MoveList {
    let mut list = MoveList {
        moves: [(0, 0); MAX_MOVES],
        len: 0,
    };
    let must_capture_rule_enabled = if let CrownfallRuleset::Custom { mandatory_capture,..} = rules.ruleset {
        mandatory_capture
    } else {
        false
    };
    let cell_count = tables::cell_count(board.variant());
    // Under mandatory capture, each candidate's capture flag is computed
    // once here (against a single scratch board, two cell writes per
    // candidate) and the list is filtered afterwards - not the
    // `has_available_capture` pre-scan plus a second per-move check, which
    // would run the same capture detection twice per candidate.
    let mut captures = [false; MAX_MOVES];
    let mut any_capture = false;
    let mut scratch = *board;
    for index in 0..cell_count {
        if let Some(piece) = board.cells()[index]
            && piece.player == player
        {
            for &to in board.move_candidates(CrownfallBoardCell::new_index(index), rules) {
                if board.cells()[to as usize].is_some() {
                    continue;
                }
                if must_capture_rule_enabled {
                    scratch.cells_mut()[index] = None;
                    scratch.cells_mut()[to as usize] = Some(piece);
                    let to_cell = CrownfallBoardCell::new_index(to as usize);
                    let this_captures =
                        scratch.move_captures_something(to_cell, player, piece.kind, rules);
                    scratch.cells_mut()[to as usize] = None;
                    scratch.cells_mut()[index] = Some(piece);
                    captures[list.len] = this_captures;
                    any_capture |= this_captures;
                }
                list.moves[list.len] = (index as u8, to);
                list.len += 1;
            }
        }
    }
    if any_capture {
        let mut kept = 0;
        for (slot, &this_captures) in captures[..list.len].iter().enumerate() {
            if this_captures {
                list.moves[kept] = list.moves[slot];
                kept += 1;
            }
        }
        list.len = kept;
    }
    order_moves(board, &mut list, player, rules);
    list
}

/// Reorders `list` into three tiers: moves whose destination touches an
/// enemy piece (via plain adjacency, the Knight capture shape, or Archer
/// range) first, then quiet moves that advance toward the enemy Crown
/// (matching the evaluator's proximity term), then the rest. Promising moves
/// resolving early is what makes alpha-beta's cutoffs bite - ordering is a
/// pure heuristic and never changes which moves are searched, only the order
/// (a counting sort keeps relative order within each tier, so tie-breaking
/// stays deterministic).
fn order_moves(
    board: &CrownfallBoardState,
    list: &mut MoveList,
    player: CrownfallPlayerKind,
    rules: CrownfallRules,
) {
    if list.len < 2 {
        return;
    }
    let variant = board.variant();
    let diagonal_knights = matches!(
        rules.ruleset,
        CrownfallRuleset::Custom {
            knights_move_diagonally: true,
            ..
        }
    );
    let enemy_crown = board.cells().iter().position(|cell| {
        matches!(cell, Some(piece) if piece.player != player && piece.kind == CrownfallPieceKind::Crown)
    });

    let mut tiers = [0u8; MAX_MOVES];
    let mut tier_counts = [0usize; 3];
    for (slot, &(from, to)) in list.moves[..list.len].iter().enumerate() {
        let enemy_at = |&n: &u8| {
            matches!(board.cells()[n as usize], Some(piece) if piece.player != player)
        };
        let mut tactical = tables::ortho(variant, to as usize).iter().any(enemy_at);
        if !tactical {
            // `from` is occupied by construction - see collect_moves.
            tactical = match board.cells()[from as usize].map(|piece| piece.kind) {
                Some(CrownfallPieceKind::Knight) => {
                    let arc = if diagonal_knights {
                        tables::knight_moves(variant, player, to as usize)
                    } else {
                        tables::knight_arcs(variant, player, to as usize)
                    };
                    arc.iter().any(enemy_at)
                }
                Some(CrownfallPieceKind::Archer) => tables::archer_range(variant, to as usize)
                    .iter()
                    .any(enemy_at),
                _ => false,
            };
        }
        let tier = if tactical {
            0
        } else if let Some(crown) = enemy_crown
            && tables::dist(variant, crown, to as usize)
                < tables::dist(variant, crown, from as usize)
        {
            1
        } else {
            2
        };
        tiers[slot] = tier;
        tier_counts[tier as usize] += 1;
    }

    let mut next = [
        0,
        tier_counts[0],
        tier_counts[0] + tier_counts[1],
    ];
    let mut ordered = [(0u8, 0u8); MAX_MOVES];
    for (slot, &entry) in list.moves[..list.len].iter().enumerate() {
        let tier = tiers[slot] as usize;
        ordered[next[tier]] = entry;
        next[tier] += 1;
    }
    list.moves[..list.len].copy_from_slice(&ordered[..list.len]);
}

/// Static evaluation from `player`'s point of view. Three symmetric terms:
/// material (piece values), mobility (legal-move-count difference), and
/// crown proximity (each non-Crown piece scores `max_distance` minus its
/// `tables::dist` distance to the enemy Crown, rewarding massing pieces near
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
    let variant = board.variant();
    let diagonal_knights = matches!(
        game.rules.ruleset,
        CrownfallRuleset::Custom {
            knights_move_diagonally: true,
            ..
        }
    );
    // Manhattan distance is at most 2*(board_length-1) (opposite corners);
    // subtracting it from this yields a "closer is bigger" score.
    let max_distance = 2 * (board.board_length() as i32 - 1);
    // Proximity to a Crown that's already been captured is meaningless, so a
    // missing Crown zeroes that side's proximity term. Both Crowns are found
    // in one pass rather than two full `position` scans.
    let mut own_crown = None;
    let mut enemy_crown = None;
    for (index, cell) in board.cells().iter().enumerate() {
        if let Some(piece) = cell
            && piece.kind == CrownfallPieceKind::Crown
        {
            if piece.player == player {
                own_crown = Some(index);
            } else {
                enemy_crown = Some(index);
            }
        }
    }

    let mut material = 0;
    let mut mobility = 0;
    let mut proximity = 0;
    for (index, &cell) in board.cells().iter().enumerate() {
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
                CrownfallPieceKind::Archer => weights.archer,
            };

        // Equivalent to `move_candidates`, but reuses the piece already in
        // hand instead of re-reading the cell and re-matching the ruleset -
        // this loop runs for every piece at every leaf of the search.
        let candidates = if piece.kind == CrownfallPieceKind::Knight {
            if diagonal_knights {
                tables::knight_diagonal_moves(variant, piece.player, index)
            } else {
                tables::knight_moves(variant, piece.player, index)
            }
        } else {
            tables::ortho(variant, index)
        };
        for &to in candidates {
            if board.cells()[to as usize].is_none() {
                mobility += sign;
            }
        }

        if piece.kind != CrownfallPieceKind::Crown {
            let target_crown = if mine { enemy_crown } else { own_crown };
            if let Some(crown) = target_crown {
                proximity += sign * (max_distance - tables::dist(variant, crown, index) as i32);
            }
        }
    }

    material + weights.mobility * mobility + weights.crown_proximity * proximity
}
