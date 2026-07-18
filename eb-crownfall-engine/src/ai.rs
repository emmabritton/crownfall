//! A minimax (negamax) game-playing AI for local/offline vs-computer play.
//!
//! The search is allocation-free: legal moves live in fixed stack buffers,
//! and candidate positions are explored by mutating a single `Game` in place
//! (`Game::apply_action_quiet`) and rolling it back by replaying the move's
//! cell journal in reverse (see `impls::CellJournal`) — a handful of byte
//! stores plus a `Vec::truncate`, with no whole-board copy in either
//! direction. This matters on the GBA's ARM7TDMI, where a per-node heap
//! clone of the history would dwarf the actual search work.
use crate::impls::MoveScratch;
use crate::{
    CrownfallBoardCell, CrownfallBoardState, CrownfallBoardVariant, CrownfallGame,
    CrownfallGameState, CrownfallPiece, CrownfallPieceKind, CrownfallPlayerAction,
    CrownfallPlayerKind, CrownfallRules, PieceCache,
};
use crate::{CrownfallRuleset, hash, tables};
use alloc::vec;
use alloc::vec::Vec;

/// Sized so scores fit `TtEntry`'s i16 storage with room to spare, while
/// still dwarfing every positional term (evaluation magnitudes top out
/// around ±2,500) - a win/loss always dominates any material swing.
const VICTORY_SCORE: i32 = 30000;

/// "Contempt" for draws: a Draw terminal is scored as mildly worse than a
/// neutral position instead of flat 0, so that when the search finds a
/// drawing line and a roughly-even non-drawing line at the same depth, it
/// prefers the latter instead of being indifferent. Kept below the cheapest
/// piece weight (`Weights::spy`/`archer`, 15-25 depending on personality) so
/// the AI is nudged away from repetition/shuffling but never sacrifices a
/// whole piece purely to dodge a draw - it only breaks ties among otherwise
/// similar continuations. Applied uniformly regardless of which player the
/// score is relative to, so it doesn't disturb negamax's zero-sum recursion
/// the way an asymmetric bonus would.
const DRAW_SCORE: i32 = -14;

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

/// Transposition-table size: 4096 entries at 8 bytes each is 32 KiB,
/// living for the searcher's lifetime. Small enough to leave most of the
/// GBA's 256 KiB EWRAM free, large enough that depth-4/5 searches on every
/// board size see high hit rates; on collisions the newer node simply
/// replaces the older (iterative deepening revisits shallow entries with
/// deeper ones anyway).
const TT_SIZE: usize = 1 << 12;

const TT_EMPTY: u8 = 0;
/// The stored score is the node's true negamax value within the window.
const TT_EXACT: u8 = 1;
/// The search cut off (score >= beta): the true value is at least `score`.
const TT_LOWER: u8 = 2;
/// No move raised alpha: the true value is at most `score`.
const TT_UPPER: u8 = 3;

/// One transposition-table slot, packed to 8 bytes so a probe or store
/// moves half the bytes a naive layout would - the table lives in the GBA's
/// EWRAM, where every 16-bit bus access costs 3 waitstates. `tag` is the
/// position hash's upper 16 bits (the slot index consumes the low 12), so a
/// probe verifies ~28 bits of the key; a tag collision can at worst promote
/// a wrong move to the front of move ordering (it's still legality-checked
/// by `apply_action_quiet`) or return a bounded-wrong score - the standard
/// small-TT trade every engine makes. `score` narrows to i16, which
/// `VICTORY_SCORE` is sized for. Positions are keyed by the same
/// incrementally-maintained hash the repetition rule uses
/// (`game.history.last()`), so probing costs no extra hashing at all.
#[derive(Clone, Copy)]
struct TtEntry {
    tag: u16,
    score: i16,
    mv: (u8, u8),
    depth: u8,
    flag: u8,
}

const TT_UNSET: TtEntry = TtEntry {
    tag: 0,
    score: 0,
    mv: (0, 0),
    depth: 0,
    flag: TT_EMPTY,
};

/// The verification tag for a position hash: the bits the slot index
/// doesn't already pin down.
#[inline]
fn tt_tag(key: u32) -> u16 {
    (key >> 16) as u16
}

/// Killer-move plies tracked: `negamax` indexes killers by remaining depth,
/// which `best_move` bounds by the requested search depth - anything beyond
/// this (only reachable through `examples/simulate.rs` probing absurd
/// depths) just goes without killers rather than growing the array.
const KILLER_PLIES: usize = 32;

/// The two most recent beta-cutoff moves per remaining-depth level. A move
/// that just refuted one sibling line very often refutes the next - trying
/// killers straight after the TT move (and ahead of the static tiers) makes
/// those cutoffs land on the first or second try. `(0, 0)` (a move from a
/// cell to itself) never matches a generated move, so the unset sentinel
/// can't accidentally promote anything.
type Killers = [[(u8, u8); 2]; KILLER_PLIES];

/// Everything needed to roll a `Game` back after `apply_action_quiet`,
/// minus the board: cell changes are undone by replaying the applied move's
/// journal in reverse (`restore`'s `scratch` argument — a handful of byte
/// stores instead of re-copying the whole board), the history is
/// append-only so restoring it is just a truncate (which keeps the Vec's
/// capacity — no churn), and the piece cache is a few bytes of `Copy` data.
struct Undo {
    state: CrownfallGameState,
    moves_since_capture: u16,
    history_len: usize,
    cache: PieceCache,
}

fn snapshot(game: &CrownfallGame) -> Undo {
    Undo {
        state: game.state,
        moves_since_capture: game.moves_since_capture,
        history_len: game.history.len(),
        cache: game.cache,
    }
}

fn restore(game: &mut CrownfallGame, undo: &Undo, scratch: &MoveScratch) {
    scratch.journal.undo(&mut game.board);
    game.state = undo.state;
    game.moves_since_capture = undo.moves_since_capture;
    game.cache = undo.cache;
    game.history.truncate(undo.history_len);
}

/// The movement-candidate tables for one board variant/ruleset, resolved
/// once per call of the hot loops (move generation and evaluation, both run
/// at every search node) - the same selection
/// `CrownfallBoardState::move_candidates` makes, but without re-matching
/// the variant and ruleset per piece.
struct MoveTables {
    ortho: &'static [tables::CellList],
    /// Per-player Knight movement rows, indexed by `player as usize`.
    knight: [&'static [tables::CellList]; 2],
}

impl MoveTables {
    fn new(variant: CrownfallBoardVariant, diagonal_knights: bool) -> MoveTables {
        let knight_for = |player| {
            if diagonal_knights {
                tables::knight_diagonal_moves_table(variant, player)
            } else {
                tables::knight_moves_table(variant, player)
            }
        };
        MoveTables {
            ortho: tables::ortho_table(variant),
            knight: [
                knight_for(CrownfallPlayerKind::White),
                knight_for(CrownfallPlayerKind::Black),
            ],
        }
    }

    #[inline]
    fn candidates(&self, piece: CrownfallPiece, index: usize) -> &'static [u8] {
        if piece.kind() == CrownfallPieceKind::Knight {
            self.knight[piece.player() as usize][index].as_slice()
        } else {
            self.ortho[index].as_slice()
        }
    }
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
///
/// Convenience wrapper that builds a fresh [`CrownfallSearcher`] per call -
/// callers that ask for moves repeatedly (a vs-AI game loop) should hold a
/// `CrownfallSearcher` instead, which reuses its transposition table across
/// turns (warmer move ordering, and no 32 KiB allocation per move).
pub fn best_move(
    game: &CrownfallGame,
    player: CrownfallPlayerKind,
    depth: u8,
    personality: CrownfallPersonality,
) -> Option<CrownfallPlayerAction> {
    CrownfallSearcher::new().best_move(game, player, depth, personality)
}

/// A reusable AI search: owns the transposition table and killer-move
/// ordering hints so they persist from one [`best_move`](Self::best_move)
/// call to the next. Positions recur heavily between consecutive turns of
/// one game (the opponent's reply changes little of the tree the previous
/// search explored), so a warm table both skips re-searching them and
/// seeds move ordering - and the table's 32 KiB is allocated once for the
/// searcher's lifetime instead of once per move, which is what the GBA
/// build wants.
///
/// Cached scores are only meaningful for the rules and personality they
/// were searched under (personality scales the evaluation weights), so the
/// searcher notes both and self-resets whenever either differs from the
/// previous call - sharing one searcher across games is therefore safe,
/// just wasteful if they alternate rulesets. Use one searcher per AI
/// player.
pub struct CrownfallSearcher {
    tt: Vec<TtEntry>,
    killers: Killers,
    /// What the cached entries were computed under; a mismatch on the next
    /// call clears them.
    context: Option<(CrownfallRules, CrownfallPersonality)>,
}

impl Default for CrownfallSearcher {
    fn default() -> CrownfallSearcher {
        CrownfallSearcher::new()
    }
}

impl CrownfallSearcher {
    pub fn new() -> CrownfallSearcher {
        CrownfallSearcher {
            tt: vec![TT_UNSET; TT_SIZE],
            killers: [[(0, 0); 2]; KILLER_PLIES],
            context: None,
        }
    }

    /// Same contract as the free [`best_move`] function, but carrying the
    /// transposition table and killer moves over from previous calls.
    pub fn best_move(
        &mut self,
        game: &CrownfallGame,
        player: CrownfallPlayerKind,
        depth: u8,
        personality: CrownfallPersonality,
    ) -> Option<CrownfallPlayerAction> {
        let context = (game.rules, personality);
        if self.context != Some(context) {
            self.tt.fill(TT_UNSET);
            self.killers = [[(0, 0); 2]; KILLER_PLIES];
            self.context = Some(context);
        }
        self.search(game, player, depth, personality)
    }

    fn search(
        &mut self,
        game: &CrownfallGame,
        player: CrownfallPlayerKind,
        depth: u8,
        personality: CrownfallPersonality,
    ) -> Option<CrownfallPlayerAction> {
        let tt = self.tt.as_mut_slice();
        let killers = &mut self.killers;
        // The one clone of the whole search — every node below mutates this
        // copy in place and rolls it back.
        let mut game = game.clone();
        // The whole search reads the piece cache (evaluation, move ordering,
        // attrition checks); making it valid once here means every node
        // below maintains it incrementally.
        game.ensure_cache();
        // Every non-terminal node's position hash is `history.last()` (pushed
        // by `resolve_continuation` on each applied move) - the transposition
        // table keys off it, so a game deserialized without its history needs
        // the root hash seeded here once.
        if game.history.is_empty() {
            game.history.push(hash::position_hash(&game.board, player));
        }
        let rules = game.rules;
        let mut moves = collect_moves(
            &game.board,
            player,
            rules,
            game.cache.crown(player.opposite()),
        );
        // Iterative deepening: search depth 1, 2, ... up to the requested
        // depth, rotating each iteration's best root move to the front for
        // the next. The shallow passes cost a small fraction of the final one
        // and their move ordering makes alpha-beta cut far more of the deep
        // tree — the net is faster than a single full-depth pass. `depth ==
        // 0` has always meant "evaluate each move's immediate result", i.e.
        // the same tree as 1.
        let mut best: Option<CrownfallPlayerAction> = None;
        // Every candidate is rolled back to the same pre-move position, so
        // one snapshot serves the entire search instead of one board copy per
        // move.
        let undo = snapshot(&game);
        let mut previous_score = 0;
        for iteration_depth in 1..=depth.max(1) {
            // Aspiration window: after the first pass, open the window only
            // `ASPIRATION_WINDOW` either side of the previous iteration's
            // score - the true score rarely moves further than that between
            // passes, and the narrow bounds let alpha-beta cut much more of
            // the tree. When the search does land on the window's edge the
            // result is only a bound, so that side is reopened to full width
            // and the pass rerun (the TT keeps the rerun cheap).
            let (mut window_low, mut window_high) = if iteration_depth == 1 {
                (i32::MIN + 1, i32::MAX - 1)
            } else {
                (
                    previous_score - ASPIRATION_WINDOW,
                    previous_score + ASPIRATION_WINDOW,
                )
            };
            let outcome = loop {
                match search_root(
                    &mut game,
                    &moves,
                    player,
                    iteration_depth,
                    personality,
                    window_low,
                    window_high,
                    tt,
                    killers,
                    &undo,
                ) {
                    None => break None,
                    Some((slot, action, score)) => {
                        if score <= window_low && window_low > i32::MIN + 1 {
                            window_low = i32::MIN + 1;
                            continue;
                        }
                        if score >= window_high && window_high < i32::MAX - 1 {
                            window_high = i32::MAX - 1;
                            continue;
                        }
                        break Some((slot, action, score));
                    }
                }
            };
            let Some((slot, action, score)) = outcome else {
                // Every root move was rejected — deeper passes can't differ.
                break;
            };
            best = Some(action);
            previous_score = score;
            moves.moves[..=slot].rotate_right(1);
        }
        best
    }
}

/// How far either side of the previous iteration's score the next
/// iteration's aspiration window opens - roughly one Knight, so ordinary
/// positional drift stays inside the window while a tactic that wins or
/// loses a piece triggers the (rare) full-width research.
const ASPIRATION_WINDOW: i32 = 40;

/// One root-level alpha-beta pass over `moves` at `depth`, returning the
/// best move's slot, action and score - `None` if every root move was
/// rejected. Same principal-variation scheme as `negamax`: the front move
/// (last iteration's best) gets the full window, the rest a null-window
/// probe with a full re-search only on a fail-high.
#[allow(clippy::too_many_arguments)]
fn search_root(
    game: &mut CrownfallGame,
    moves: &MoveList,
    player: CrownfallPlayerKind,
    depth: u8,
    personality: CrownfallPersonality,
    mut alpha: i32,
    beta: i32,
    tt: &mut [TtEntry],
    killers: &mut Killers,
    undo: &Undo,
) -> Option<(usize, CrownfallPlayerAction, i32)> {
    let mut best_slot = None;
    let mut best_score = i32::MIN;
    let mut best_action = None;
    let mut scratch = MoveScratch::new();
    for slot in 0..moves.len {
        let (from, to) = moves.moves[slot];
        let action = CrownfallPlayerAction::Move {
            player,
            from: CrownfallBoardCell::new_index(from as usize),
            to: CrownfallBoardCell::new_index(to as usize),
        };
        // `apply_action_quiet` leaves the game untouched on Err, so no
        // rollback is needed to skip the move.
        scratch.reset();
        if game.apply_action_quiet(action, &mut scratch).is_err() {
            continue;
        }
        let score = if best_slot.is_none() {
            -negamax(
                game,
                player.opposite(),
                depth - 1,
                personality,
                -beta,
                -alpha,
                tt,
                killers,
            )
        } else {
            let probe = -negamax(
                game,
                player.opposite(),
                depth - 1,
                personality,
                -alpha - 1,
                -alpha,
                tt,
                killers,
            );
            if probe > alpha && probe < beta {
                -negamax(
                    game,
                    player.opposite(),
                    depth - 1,
                    personality,
                    -beta,
                    -alpha,
                    tt,
                    killers,
                )
            } else {
                probe
            }
        };
        restore(game, undo, &scratch);
        if best_slot.is_none() || score > best_score {
            best_score = score;
            best_slot = Some(slot);
            best_action = Some(action);
        }
        if score > alpha {
            alpha = score;
        }
        if alpha >= beta {
            break;
        }
    }
    best_slot.map(|slot| {
        (
            slot,
            best_action.expect("set alongside best_slot"),
            best_score,
        )
    })
}

// Search context threaded straight through the recursion; a params struct
// would just rename the same eight things.
#[allow(clippy::too_many_arguments)]
fn negamax(
    game: &mut CrownfallGame,
    player: CrownfallPlayerKind,
    depth: u8,
    personality: CrownfallPersonality,
    mut alpha: i32,
    beta: i32,
    tt: &mut [TtEntry],
    killers: &mut Killers,
) -> i32 {
    match game.state {
        CrownfallGameState::Victory(winner, _) => {
            return if winner == player {
                VICTORY_SCORE
            } else {
                -VICTORY_SCORE
            };
        }
        CrownfallGameState::Draw(_) => return DRAW_SCORE,
        CrownfallGameState::Playing(_) => {}
    }
    if depth == 0 {
        return evaluate(game, player, personality);
    }

    // Transposition probe. The key is the position hash the repetition rule
    // already maintains (side to move included), so equal keys mean the same
    // position from the same player's perspective. A stored result from an
    // equal-or-deeper search settles this node outright when its bound
    // allows; otherwise the stored move still goes to the front of the move
    // list, which is where most of the table's value comes from - alpha-beta
    // cuts hardest when the best move is searched first. (Like most engines,
    // this trades away path-dependence: a position reached via different
    // move orders shares one entry even though the repetition/no-progress
    // clocks differ. Draws only score `DRAW_SCORE`, so the error is bounded
    // and tiny next to the search it buys.)
    let key = game.history.last().copied().unwrap_or(0);
    let slot = (key as usize) & (TT_SIZE - 1);
    let entry = tt[slot];
    let mut tt_move = None;
    if entry.flag != TT_EMPTY && entry.tag == tt_tag(key) {
        if entry.depth >= depth {
            match entry.flag {
                TT_EXACT => return entry.score as i32,
                TT_LOWER if entry.score as i32 >= beta => return entry.score as i32,
                TT_UPPER if entry.score as i32 <= alpha => return entry.score as i32,
                _ => {}
            }
        }
        tt_move = Some(entry.mv);
    }

    let mut moves = collect_moves(
        &game.board,
        player,
        game.rules,
        game.cache.crown(player.opposite()),
    );
    if moves.len == 0 {
        return evaluate(game, player, personality);
    }
    // Front-load the TT move, then this depth's killers behind it.
    let mut front = 0;
    if let Some(mv) = tt_move
        && let Some(found) = moves.moves[..moves.len].iter().position(|&m| m == mv)
    {
        moves.moves[..=found].rotate_right(1);
        front = 1;
    }
    if (depth as usize) < KILLER_PLIES {
        for killer in killers[depth as usize] {
            if Some(killer) == tt_move {
                continue;
            }
            if let Some(found) = moves.moves[front..moves.len]
                .iter()
                .position(|&m| m == killer)
            {
                moves.moves[front..=front + found].rotate_right(1);
                front += 1;
            }
        }
    }

    let alpha_original = alpha;
    let mut best = i32::MIN + 1;
    let mut best_move = None;
    // One snapshot per node - every move tried here rolls back to the same
    // state/history/cache; the board itself is undone by replaying each
    // move's own cell journal.
    let undo = snapshot(game);
    let mut scratch = MoveScratch::new();
    for &(from, to) in &moves.moves[..moves.len] {
        let action = CrownfallPlayerAction::Move {
            player,
            from: CrownfallBoardCell::new_index(from as usize),
            to: CrownfallBoardCell::new_index(to as usize),
        };
        scratch.reset();
        if game.apply_action_quiet(action, &mut scratch).is_err() {
            continue;
        }
        // Principal variation search: only the first (best-ordered) move gets
        // the full window. Later moves are probed with a null window - the
        // cheapest possible refutation of "this is no better than what we
        // have" - and only re-searched at full width in the rare case the
        // probe says they might beat it. With the TT move in front, the
        // probe almost always confirms and the re-search almost never runs.
        let score = if best_move.is_none() {
            -negamax(
                game,
                player.opposite(),
                depth - 1,
                personality,
                -beta,
                -alpha,
                tt,
                killers,
            )
        } else {
            let probe = -negamax(
                game,
                player.opposite(),
                depth - 1,
                personality,
                -alpha - 1,
                -alpha,
                tt,
                killers,
            );
            if probe > alpha && probe < beta {
                -negamax(
                    game,
                    player.opposite(),
                    depth - 1,
                    personality,
                    -beta,
                    -alpha,
                    tt,
                    killers,
                )
            } else {
                probe
            }
        };
        restore(game, &undo, &scratch);
        if score > best {
            best = score;
            best_move = Some((from, to));
        }
        if best > alpha {
            alpha = best;
        }
        if alpha >= beta {
            // Remember the refuting move for sibling nodes at this depth.
            if (depth as usize) < KILLER_PLIES {
                let level = &mut killers[depth as usize];
                if level[0] != (from, to) {
                    level[1] = level[0];
                    level[0] = (from, to);
                }
            }
            break;
        }
    }
    // Store the result (always-replace). `best_move` is only `None` if every
    // move was rejected, in which case there's nothing meaningful to cache.
    if let Some(mv) = best_move {
        let flag = if best <= alpha_original {
            TT_UPPER
        } else if best >= beta {
            TT_LOWER
        } else {
            TT_EXACT
        };
        debug_assert!(
            i16::try_from(best).is_ok(),
            "search scores must stay within TtEntry's i16 storage"
        );
        tt[slot] = TtEntry {
            tag: tt_tag(key),
            score: best as i16,
            mv,
            depth,
            flag,
        };
    }
    best
}

/// Legal moves for `player`, honoring `rules.mandatory_capture`: when set
/// and at least one capturing move exists anywhere on the board, only
/// capturing moves are included. `enemy_crown` is the cached enemy-Crown
/// cell (see `PieceCache`), used by move ordering.
fn collect_moves(
    board: &CrownfallBoardState,
    player: CrownfallPlayerKind,
    rules: CrownfallRules,
    enemy_crown: Option<CrownfallBoardCell>,
) -> MoveList {
    let mut list = MoveList {
        moves: [(0, 0); MAX_MOVES],
        len: 0,
    };
    let must_capture_rule_enabled = if let CrownfallRuleset::Custom {
        mandatory_capture, ..
    } = rules.ruleset
    {
        mandatory_capture
    } else {
        false
    };
    let variant = board.variant();
    let diagonal_knights = matches!(
        rules.ruleset,
        CrownfallRuleset::Custom {
            knights_move_diagonally: true,
            ..
        }
    );
    let cell_count = tables::cell_count(variant);
    let move_tables = MoveTables::new(variant, diagonal_knights);
    let cells = board.cells();
    // Under mandatory capture, each candidate's capture flag is computed
    // once here (against a single scratch board, two cell writes per
    // candidate) and the list is filtered afterwards - not the
    // `has_available_capture` pre-scan plus a second per-move check, which
    // would run the same capture detection twice per candidate. The scratch
    // board copy only exists under that ruleset - every other ruleset would
    // pay for a whole-board copy per search node without ever touching it.
    let mut captures = [false; MAX_MOVES];
    let mut any_capture = false;
    let mut scratch = if must_capture_rule_enabled {
        Some(*board)
    } else {
        None
    };
    for index in 0..cell_count {
        if let Some(piece) = cells[index]
            && piece.player() == player
        {
            for &to in move_tables.candidates(piece, index) {
                if cells[to as usize].is_some() {
                    continue;
                }
                if let Some(scratch) = scratch.as_mut() {
                    scratch.cells_mut()[index] = None;
                    scratch.cells_mut()[to as usize] = Some(piece);
                    let to_cell = CrownfallBoardCell::new_index(to as usize);
                    let this_captures =
                        scratch.move_captures_something(to_cell, player, piece.kind(), rules);
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
    order_moves(board, &mut list, player, rules, enemy_crown);
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
    enemy_crown: Option<CrownfallBoardCell>,
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
    let cells = board.cells();
    // All the per-move lookups below resolve their tables once out here -
    // this runs at every search node, over every generated move.
    let ortho = tables::ortho_table(variant);
    let arcs = if diagonal_knights {
        tables::knight_moves_table(variant, player)
    } else {
        tables::knight_arcs_table(variant, player)
    };
    let archer_range = tables::archer_range_table(variant);
    let crown_dist = enemy_crown.map(|crown| tables::dist_row(variant, crown.to_index()));

    let mut tiers = [0u8; MAX_MOVES];
    let mut tier_counts = [0usize; 3];
    for (slot, &(from, to)) in list.moves[..list.len].iter().enumerate() {
        let enemy_at =
            |&n: &u8| matches!(cells[n as usize], Some(piece) if piece.player() != player);
        let mut tactical = ortho[to as usize].as_slice().iter().any(enemy_at);
        if !tactical {
            // `from` is occupied by construction - see collect_moves.
            tactical = match cells[from as usize].map(|piece| piece.kind()) {
                Some(CrownfallPieceKind::Knight) => {
                    arcs[to as usize].as_slice().iter().any(enemy_at)
                }
                Some(CrownfallPieceKind::Archer) => {
                    archer_range[to as usize].as_slice().iter().any(enemy_at)
                }
                _ => false,
            };
        }
        let tier = if tactical {
            0
        } else if let Some(dist) = crown_dist
            && dist[to as usize] < dist[from as usize]
        {
            1
        } else {
            2
        };
        tiers[slot] = tier;
        tier_counts[tier as usize] += 1;
    }

    let mut next = [0, tier_counts[0], tier_counts[0] + tier_counts[1]];
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
///
/// Two attempts at scaling mobility/crown_proximity up as a draw threshold
/// approaches (favoring forward contact under time pressure) were tried and
/// reverted - both consistently made self-play draw rates *worse* in mirror
/// matchups (same personality/depth on both sides), for a small and
/// inconsistent benefit elsewhere. In hindsight a draw between two
/// identically-matched opponents isn't a defect to engineer away - it's the
/// correct outcome when neither side can force an advantage, the same way
/// chess engines don't try to eliminate draws between equal play. The
/// matchups where a real asymmetry exists (different personality or depth)
/// already resolve more decisively without any special-casing, since the
/// asymmetry itself does that work.
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
    let move_tables = MoveTables::new(variant, diagonal_knights);
    let cells = board.cells();

    // The search always runs with a valid cache (see `search`); the rebuild
    // fallback keeps evaluate correct for any hand-built caller.
    let cache = if game.cache.valid {
        game.cache
    } else {
        PieceCache::rebuild(board)
    };

    // Material comes straight off the incrementally-maintained counts - no
    // per-piece matching in the scan below.
    let mut material = 0;
    for (kind, weight) in [
        (CrownfallPieceKind::Crown, weights.crown),
        (CrownfallPieceKind::Knight, weights.knight),
        (CrownfallPieceKind::Spy, weights.spy),
        (CrownfallPieceKind::Archer, weights.archer),
    ] {
        material += weight
            * (cache.count(kind, player) as i32 - cache.count(kind, player.opposite()) as i32);
    }

    // Proximity to a Crown that's already been captured is meaningless, so a
    // missing Crown zeroes that side's proximity term. Both Crown cells are
    // known up front from the cache, so each side's whole distance row is
    // fetched once and proximity folds into the same single board pass as
    // mobility - no scratch piece list, no second loop.
    let own_crown_dist = cache
        .crown(player)
        .map(|crown| tables::dist_row(variant, crown.to_index()));
    let enemy_crown_dist = cache
        .crown(player.opposite())
        .map(|crown| tables::dist_row(variant, crown.to_index()));

    let mut mobility = 0;
    let mut proximity = 0;
    for (index, &cell) in cells.iter().enumerate() {
        let Some(piece) = cell else {
            continue;
        };
        let mine = piece.player() == player;
        let sign = if mine { 1 } else { -1 };

        for &to in move_tables.candidates(piece, index) {
            if cells[to as usize].is_none() {
                mobility += sign;
            }
        }

        if piece.kind() != CrownfallPieceKind::Crown {
            let target_dist = if mine {
                enemy_crown_dist
            } else {
                own_crown_dist
            };
            if let Some(dist) = target_dist {
                proximity += sign * (max_distance - dist[index] as i32);
            }
        }
    }

    material + weights.mobility * mobility + weights.crown_proximity * proximity
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CrownfallPlayState;

    /// Plays AI self-play with one persistent `CrownfallSearcher` per player
    /// (different personalities, so their caches never share entries) and
    /// asserts every returned move applies legally - the warm-table path has
    /// to produce moves exactly as valid as a fresh search's.
    fn assert_persistent_searcher_selfplay(rules: CrownfallRules) {
        let mut searchers = [CrownfallSearcher::new(), CrownfallSearcher::new()];
        let personalities = [
            CrownfallPersonality::Aggressive,
            CrownfallPersonality::Defensive,
        ];
        let mut game = CrownfallGame::new(rules);
        for _ in 0..60 {
            let CrownfallGameState::Playing(play_state) = game.state else {
                break;
            };
            let player = play_state.player();
            let Some(action) = searchers[player as usize].best_move(
                &game,
                player,
                3,
                personalities[player as usize],
            ) else {
                break;
            };
            game.apply_action(action)
                .expect("persistent searcher must produce legal moves");
        }
    }

    #[test]
    fn persistent_searcher_plays_legal_games() {
        for rules in [
            CrownfallRules::standard(),
            CrownfallRules::mini(),
            CrownfallRules::grand(),
            CrownfallRules::standard_archers(),
            CrownfallRules::standard_mandatory_capture(),
            CrownfallRules::standard_all_captures_processed(),
            CrownfallRules::standard_diagonal_knights(),
        ] {
            assert_persistent_searcher_selfplay(rules);
        }
    }

    /// Applying a move through the quiet path and rolling it back via the
    /// cell journal must restore the game exactly - board, state, history
    /// and the derived piece cache alike (game equality ignores the cache
    /// by design, so its fields are asserted separately).
    #[test]
    fn journal_restore_roundtrips_every_selfplay_move() {
        for rules in [
            CrownfallRules::standard(),
            CrownfallRules::standard_archers(),
            CrownfallRules::standard_all_captures_processed(),
        ] {
            let mut game = CrownfallGame::new(rules);
            for _ in 0..40 {
                let CrownfallGameState::Playing(play_state) = game.state else {
                    break;
                };
                let player = play_state.player();
                let Some(action) = best_move(&game, player, 2, CrownfallPersonality::Balanced)
                else {
                    break;
                };
                let before = game.clone();
                let undo = snapshot(&game);
                let mut scratch = MoveScratch::new();
                game.apply_action_quiet(action, &mut scratch)
                    .expect("AI produces legal moves");
                restore(&mut game, &undo, &scratch);
                assert_eq!(game, before, "restore must roundtrip under {rules:?}");
                assert_eq!(game.cache.counts, before.cache.counts);
                assert_eq!(game.cache.crowns, before.cache.crowns);
                game.apply_action(action).expect("AI produces legal moves");
            }
        }
    }

    /// One searcher fed alternating rules/personality contexts must reset
    /// its cache between them (scores from one context are meaningless in
    /// the other) and still produce legal moves for each.
    #[test]
    fn searcher_reset_on_context_change_stays_legal() {
        let mut searcher = CrownfallSearcher::new();
        let standard = CrownfallGame::new(CrownfallRules::standard());
        let diagonal = CrownfallGame::new(CrownfallRules::standard_diagonal_knights());
        for _ in 0..3 {
            for (game, personality) in [
                (&standard, CrownfallPersonality::Balanced),
                (&diagonal, CrownfallPersonality::Aggressive),
            ] {
                let action = searcher
                    .best_move(game, CrownfallPlayerKind::White, 3, personality)
                    .expect("opening position has legal moves");
                game.clone()
                    .apply_action(action)
                    .expect("searcher must produce a legal move after a context switch");
            }
        }
    }

    /// The `CrownfallPlayState` import is only exercised through
    /// `CrownfallGameState::Playing` pattern matches above.
    #[allow(unused)]
    fn _play_state_used(state: CrownfallPlayState) {}
}
