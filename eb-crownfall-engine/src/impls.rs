use crate::CrownfallPieceKind::Archer;
use crate::errors::CrownfallError;
use crate::hash;
use crate::hash::position_hash;
use crate::tables;
use crate::*;
use alloc::vec::Vec;

/// Formats a cell as its `(x,y)` board coordinate rather than the raw
/// index `Debug` prints, for move/capture log lines.
#[cfg(feature = "log")]
struct LogCoord(CrownfallBoardCell, CrownfallBoardVariant);

#[cfg(feature = "log")]
impl core::fmt::Display for LogCoord {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let (x, y) = self.0.to_coord(self.1);
        write!(f, "({x},{y})")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CaptureKind {
    Spy,
    Knight,
}

/// One capture detected by `check_piece_captures` - kept `Copy` so a move's
/// worth of captures fits in a stack array (see `PieceCaptures`).
#[derive(Clone, Copy)]
struct PieceCapture {
    target: CrownfallBoardCell,
    kind: CaptureKind,
    attackers: (CrownfallBoardCell, CrownfallBoardCell),
}

/// The capture scan looks at a mover's 4 orthogonal neighbours plus, for a
/// Knight, its 3-cell capture-arc - so a single move can never threaten
/// more than 6 cells (the straight-ahead arc cell overlaps an orthogonal
/// one), and each threatened cell yields at most one ordinary capture.
const MAX_SCAN_CELLS: usize = 6;

/// An Archer's ranged reach is at most the 4 cells exactly two orthogonal
/// tiles away.
const MAX_ARCHER_TARGETS: usize = 4;

type ScanCells = ([u8; MAX_SCAN_CELLS], usize);
type PieceCaptures = ([PieceCapture; MAX_SCAN_CELLS], usize);
/// (target, ally-that-satisfied-adjacency) pairs found by an Archer's shot.
type ArcherCaptures = (
    [(CrownfallBoardCell, CrownfallBoardCell); MAX_ARCHER_TARGETS],
    usize,
);

/// Number of times a position (board + player to move) must occur for the
/// game to be declared a draw (matches chess's threefold repetition rule).
const REPETITION_LIMIT: usize = 3;

/// Turns without a capture before the no-progress draw rule fires (chess's
/// 50-move rule, adapted - Crownfall has no pawn-equivalent, so "no capture"
/// is the sole progress signal). This is the `Normal` board's value - `Mini`
/// halves it and `Grand` doubles it (see `CrownfallBoardVariant::no_progress_limit`),
/// since a smaller/larger board reaches a stale position proportionally sooner/later.
const NO_PROGRESS_LIMIT: u16 = 40;

/// Absolute turn-count safety net: the game is drawn if it's still going
/// after this many turns, regardless of repetition or progress. This is the
/// `Normal` board's value - `Mini` halves it and `Grand` doubles it (see
/// `CrownfallBoardVariant::total_turn_limit`).
const TOTAL_TURN_LIMIT: u16 = 200;

impl CrownfallBoardVariant {
    /// Turns without a capture before the no-progress draw rule fires,
    /// scaled to board size - `Mini` at half `NO_PROGRESS_LIMIT`, `Normal`
    /// at the base value, `Grand` at double.
    fn no_progress_limit(self) -> u16 {
        match self {
            CrownfallBoardVariant::Mini => NO_PROGRESS_LIMIT / 2,
            CrownfallBoardVariant::Normal => NO_PROGRESS_LIMIT,
            CrownfallBoardVariant::Grand => NO_PROGRESS_LIMIT * 2,
        }
    }

    /// Absolute turn-count safety net, scaled to board size - `Mini` at half
    /// `TOTAL_TURN_LIMIT`, `Normal` at the base value, `Grand` at double.
    fn total_turn_limit(self) -> u16 {
        match self {
            CrownfallBoardVariant::Mini => TOTAL_TURN_LIMIT / 2,
            CrownfallBoardVariant::Normal => TOTAL_TURN_LIMIT,
            CrownfallBoardVariant::Grand => TOTAL_TURN_LIMIT * 2,
        }
    }
}

/// Records the position that's about to be played from and returns the
/// resulting `GameState` - `Draw` if this exact position has now recurred
/// `REPETITION_LIMIT` times, if `NO_PROGRESS_LIMIT` turns have passed since
/// the last capture, or if `TOTAL_TURN_LIMIT` turns have been played in
/// total; otherwise `Playing` with `next_player` to move.
///
/// `hash_delta` is the XOR of the Zobrist keys of every cell change the
/// action made (see `hash::piece_key`), letting the new position hash be
/// derived from the previous history entry in a few XORs instead of a full
/// board scan - this runs on every applied move, including every node of
/// the AI search, where it was the scan that dominated. The player-to-move
/// key flips on every action, so it's folded in here rather than by callers.
fn resolve_continuation(
    board: &CrownfallBoardState,
    board_variant: CrownfallBoardVariant,
    next_player: CrownfallPlayerKind,
    history: &mut Vec<u32>,
    moves_since_capture: &mut u16,
    captured: bool,
    hash_delta: u32,
) -> CrownfallGameState {
    if captured {
        *moves_since_capture = 0;
    } else {
        *moves_since_capture += 1;
    }

    // Repetition only ever compares hashes *within* one game, so a caller
    // that seeded `history` with something other than a true position hash
    // (tests do) stays correct too: every later entry then carries the same
    // constant XOR offset, which preserves equality. Parity with a full
    // recompute (for games built via `CrownfallGame::new`) is covered by
    // the `incremental_hash_matches_full_recompute` unit test.
    let key = match history.last() {
        Some(&previous) => previous ^ hash_delta ^ hash::side_to_move_toggle(),
        // A game deserialized without its history has no previous hash to
        // build on - fall back to the full scan.
        None => position_hash(board, next_player),
    };
    history.push(key);
    // A position can only recur among the entries since the last capture
    // (inclusive) - every earlier position had more pieces on the board -
    // so the scan window is `moves_since_capture + 1` entries, newest first,
    // not the whole ever-growing history. This runs on every applied move
    // (including AI-search nodes), where the bounded window is what keeps
    // deep searches from going quadratic in game length. Entries at odd
    // distance from the newest have the other player to move (their hash
    // differs by `side_to_move_toggle`), so only every second entry can
    // match - stepping by 2 halves the scan and drops those entries'
    // collision-only false positives.
    let mut repeats = 0;
    for &hash in history
        .iter()
        .rev()
        .take(*moves_since_capture as usize + 1)
        .step_by(2)
    {
        if hash == key {
            repeats += 1;
            if repeats >= REPETITION_LIMIT {
                break;
            }
        }
    }
    let turns_played = (history.len() - 1) as u16;

    if repeats >= REPETITION_LIMIT {
        CrownfallGameState::Draw(DrawReason::Repetition)
    } else if *moves_since_capture >= board_variant.no_progress_limit() {
        CrownfallGameState::Draw(DrawReason::NoProgress)
    } else if turns_played >= board_variant.total_turn_limit() {
        CrownfallGameState::Draw(DrawReason::TurnLimit)
    } else {
        CrownfallGameState::Playing(CrownfallPlayState::WaitingForInput {
            player: next_player,
        })
    }
}

/// Upper bound on cell writes one applied action can make. The worst case
/// is `all_captures_processed`: the move itself (2), a captured Crown (1),
/// the self-spy-trapped mover (1), up to `MAX_SCAN_CELLS` capture targets
/// each costing a Knight sacrifice (12), and up to `MAX_ARCHER_TARGETS`
/// archer shots (4) - 20 in total, with headroom.
const MAX_JOURNAL: usize = 24;

/// The old cell values overwritten by one applied action, in write order -
/// the AI's make/unmake rolls a move back by replaying these in *reverse*
/// (so a cell written twice, e.g. moved-into then trap-removed, ends up
/// with its original content) instead of restoring a whole-board copy.
pub(crate) struct CellJournal {
    entries: [(u8, Option<CrownfallPiece>); MAX_JOURNAL],
    len: u8,
}

impl CellJournal {
    fn record(&mut self, index: usize, old: Option<CrownfallPiece>) {
        debug_assert!(
            (self.len as usize) < MAX_JOURNAL,
            "one action can never write more than MAX_JOURNAL cells"
        );
        self.entries[self.len as usize] = (index as u8, old);
        self.len += 1;
    }

    /// Restores every journaled cell, newest first.
    pub(crate) fn undo(&self, board: &mut CrownfallBoardState) {
        let cells = board.cells_mut();
        for &(index, old) in self.entries[..self.len as usize].iter().rev() {
            cells[index as usize] = old;
        }
    }
}

/// Per-action scratch state threaded through the apply path: the XOR of the
/// Zobrist keys of every cell change (see `resolve_continuation`) and the
/// journal of overwritten values. Callers that don't undo (real gameplay)
/// pass a throwaway; the AI search keeps it to roll the move back.
pub(crate) struct MoveScratch {
    hash_delta: u32,
    pub(crate) journal: CellJournal,
}

impl MoveScratch {
    pub(crate) const fn new() -> MoveScratch {
        MoveScratch {
            hash_delta: 0,
            journal: CellJournal {
                entries: [(0, None); MAX_JOURNAL],
                len: 0,
            },
        }
    }

    /// Ready for the next action without re-zeroing the entries array -
    /// entries past `len` are never read, so the search reuses one scratch
    /// per node instead of stack-initialising ~50 bytes per move tried.
    pub(crate) fn reset(&mut self) {
        self.hash_delta = 0;
        self.journal.len = 0;
    }
}

impl PieceCache {
    /// A freshly counted cache for `board` - one full scan, run once per
    /// game (or once after deserialization) rather than per move.
    pub(crate) fn rebuild(board: &CrownfallBoardState) -> PieceCache {
        let mut cache = PieceCache {
            counts: [0; 8],
            crowns: [None, None],
            valid: true,
        };
        for (index, cell) in board.cells().iter().enumerate() {
            if let Some(piece) = cell {
                cache.add(index, *piece);
            }
        }
        cache
    }

    fn add(&mut self, index: usize, piece: CrownfallPiece) {
        self.counts[piece.code()] += 1;
        if piece.kind() == CrownfallPieceKind::Crown {
            self.crowns[piece.player() as usize] = Some(CrownfallBoardCell::new_index(index));
        }
    }

    fn remove(&mut self, piece: CrownfallPiece) {
        self.counts[piece.code()] -= 1;
        if piece.kind() == CrownfallPieceKind::Crown {
            self.crowns[piece.player() as usize] = None;
        }
    }

    pub(crate) fn count(&self, kind: CrownfallPieceKind, player: CrownfallPlayerKind) -> u8 {
        self.counts[kind as usize | ((player as usize) << 2)]
    }

    /// `player`'s Crown cell, `None` once captured (or before `valid`).
    pub(crate) fn crown(&self, player: CrownfallPlayerKind) -> Option<CrownfallBoardCell> {
        self.crowns[player as usize]
    }

    /// A player is only out of the fight once both their Knights and Spies
    /// are depleted - Spy Capture works independently of Knights, so
    /// holding spies alone is still a real offensive threat (README "Losing
    /// the Game" - Attrition). Archers don't factor into attrition.
    fn attrition_defeated(&self, player: CrownfallPlayerKind) -> bool {
        self.count(CrownfallPieceKind::Knight, player) <= 1
            && self.count(CrownfallPieceKind::Spy, player) <= 1
    }
}

impl CrownfallGame {
    /// Rebuilds the derived cache if it isn't known-valid - games built by
    /// `new`/`from_parts` start valid; deserialized ones self-heal here on
    /// their first applied action (and in `ai::CrownfallSearcher::search`).
    pub(crate) fn ensure_cache(&mut self) {
        if !self.cache.valid {
            self.cache = PieceCache::rebuild(&self.board);
        }
    }

    /// The single funnel for every board mutation on the applied-move path:
    /// journals the overwritten value (for the AI's make/unmake), folds the
    /// Zobrist keys of both the removed and added contents into the hash
    /// delta, and keeps the derived piece cache in sync. A no-op write
    /// never journals or double-XORs, which is what keeps overlapping
    /// removals (possible under `all_captures_processed`, e.g. a
    /// self-trapped Knight that would also be sacrificed) correct.
    fn write_cell(&mut self, index: usize, new: Option<CrownfallPiece>, scratch: &mut MoveScratch) {
        let cells = self.board.cells_mut();
        let old = cells[index];
        if old == new {
            return;
        }
        scratch.journal.record(index, old);
        if let Some(piece) = old {
            scratch.hash_delta ^= hash::piece_key(index, piece);
            self.cache.remove(piece);
        }
        if let Some(piece) = new {
            scratch.hash_delta ^= hash::piece_key(index, piece);
            self.cache.add(index, piece);
        }
        cells[index] = new;
    }

    /// Removes and returns the piece at `index` (if any) via `write_cell`.
    fn take_cell(&mut self, index: usize, scratch: &mut MoveScratch) -> Option<CrownfallPiece> {
        let old = self.board.cells()[index];
        if old.is_some() {
            self.write_cell(index, None, scratch);
        }
        old
    }
}

/// Shorthand for building a starting layout.
fn p(kind: CrownfallPieceKind, player: CrownfallPlayerKind) -> Option<CrownfallPiece> {
    Some(CrownfallPiece::new(kind, player))
}

/// The Standard (7x7) starting layout: 6 Knights and 1 Spy in a single row
/// in front of each Crown, flanked by two more Spies.
pub fn standard_layout() -> CrownfallBoardState {
    use CrownfallPieceKind::{Crown, Knight, Spy};
    use CrownfallPlayerKind::{Black, White};
    CrownfallBoardState::Normal {
        cells: [
            // Row A (y=0)
            None,
            None,
            p(Spy, Black),
            p(Crown, Black),
            p(Spy, Black),
            None,
            None,
            // Row B (y=1)
            p(Knight, Black),
            p(Knight, Black),
            p(Knight, Black),
            p(Spy, Black),
            p(Knight, Black),
            p(Knight, Black),
            p(Knight, Black),
            // Row C (y=2)
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            // Row D (y=3)
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            // Row E (y=4)
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            // Row F (y=5)
            p(Knight, White),
            p(Knight, White),
            p(Knight, White),
            p(Spy, White),
            p(Knight, White),
            p(Knight, White),
            p(Knight, White),
            // Row G (y=6)
            None,
            None,
            p(Spy, White),
            p(Crown, White),
            p(Spy, White),
            None,
            None,
        ],
    }
}

pub fn standard_archers_layout() -> CrownfallBoardState {
    use CrownfallPieceKind::{Crown, Spy};
    use CrownfallPlayerKind::{Black, White};
    CrownfallBoardState::Normal {
        cells: [
            // Row A (y=0)
            p(Archer, Black),
            p(Archer, Black),
            p(Archer, Black),
            p(Crown, Black),
            p(Archer, Black),
            p(Archer, Black),
            p(Archer, Black),
            // Row B (y=1)
            None,
            None,
            p(Archer, Black),
            p(Spy, Black),
            p(Archer, Black),
            None,
            None,
            // Row C (y=2)
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            // Row D (y=3)
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            // Row E (y=4)
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            // Row F (y=5)
            None,
            None,
            p(Archer, White),
            p(Spy, White),
            p(Archer, White),
            None,
            None,
            // Row G (y=6)
            p(Archer, White),
            p(Archer, White),
            p(Archer, White),
            p(Crown, White),
            p(Archer, White),
            p(Archer, White),
            p(Archer, White),
        ],
    }
}

/// The Mini (5x5) starting layout: 3 Knights in a single row in front of
/// each Crown, which is flanked by two Spies.
pub fn mini_layout() -> CrownfallBoardState {
    use CrownfallPieceKind::{Crown, Knight, Spy};
    use CrownfallPlayerKind::{Black, White};
    CrownfallBoardState::Mini {
        cells: [
            // Row A (y=0)
            None,
            p(Spy, Black),
            p(Crown, Black),
            p(Spy, Black),
            None,
            // Row B (y=1)
            None,
            p(Knight, Black),
            p(Knight, Black),
            p(Knight, Black),
            None,
            // Row C (y=2)
            None,
            None,
            None,
            None,
            None,
            // Row D (y=3)
            None,
            p(Knight, White),
            p(Knight, White),
            p(Knight, White),
            None,
            // Row E (y=4)
            None,
            p(Spy, White),
            p(Crown, White),
            p(Spy, White),
            None,
        ],
    }
}

pub fn mini_archers_layout() -> CrownfallBoardState {
    use CrownfallPieceKind::{Crown, Spy};
    use CrownfallPlayerKind::{Black, White};
    CrownfallBoardState::Mini {
        cells: [
            // Row A (y=0)
            p(Archer, Black),
            p(Archer, Black),
            p(Crown, Black),
            p(Archer, Black),
            p(Archer, Black),
            // Row B (y=1)
            None,
            None,
            p(Spy, Black),
            None,
            None,
            // Row C (y=2)
            None,
            None,
            None,
            None,
            None,
            // Row D (y=3)
            None,
            None,
            p(Spy, White),
            None,
            None,
            // Row E (y=4)
            p(Archer, White),
            p(Archer, White),
            p(Crown, White),
            p(Archer, White),
            p(Archer, White),
        ],
    }
}

/// The Grand (9x9) starting layout: 8 Knights and 1 Spy in a single row in
/// front of each Crown, flanked by 2 Spies and 2 Archers.
pub fn grand_layout() -> CrownfallBoardState {
    use CrownfallPieceKind::{Archer, Crown, Knight, Spy};
    use CrownfallPlayerKind::{Black, White};
    CrownfallBoardState::Grand {
        cells: [
            // Row A (y=0)
            None,
            None,
            p(Archer, Black),
            p(Spy, Black),
            p(Crown, Black),
            p(Spy, Black),
            p(Archer, Black),
            None,
            None,
            // Row B (y=1)
            p(Knight, Black),
            p(Knight, Black),
            p(Knight, Black),
            p(Knight, Black),
            p(Spy, Black),
            p(Knight, Black),
            p(Knight, Black),
            p(Knight, Black),
            p(Knight, Black),
            // Row C (y=2)
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            // Row D (y=3)
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            // Row E (y=4)
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            // Row F (y=5)
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            // Row G (y=6)
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            // Row H (y=7)
            p(Knight, White),
            p(Knight, White),
            p(Knight, White),
            p(Knight, White),
            p(Spy, White),
            p(Knight, White),
            p(Knight, White),
            p(Knight, White),
            p(Knight, White),
            // Row I (y=8)
            None,
            None,
            p(Archer, White),
            p(Spy, White),
            p(Crown, White),
            p(Spy, White),
            p(Archer, White),
            None,
            None,
        ],
    }
}

pub fn grand_archers_layout() -> CrownfallBoardState {
    use CrownfallPieceKind::{Archer, Crown, Spy};
    use CrownfallPlayerKind::{Black, White};
    CrownfallBoardState::Grand {
        cells: [
            // Row A (y=0)
            p(Archer, Black),
            p(Archer, Black),
            p(Archer, Black),
            p(Archer, Black),
            p(Crown, Black),
            p(Archer, Black),
            p(Archer, Black),
            p(Archer, Black),
            p(Archer, Black),
            // Row B (y=1)
            None,
            None,
            p(Archer, Black),
            p(Archer, Black),
            p(Spy, Black),
            p(Archer, Black),
            p(Archer, Black),
            None,
            None,
            // Row C (y=2)
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            // Row D (y=3)
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            // Row E (y=4)
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            // Row F (y=5)
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            // Row G (y=6)
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            // Row H (y=7)
            None,
            None,
            p(Archer, White),
            p(Archer, White),
            p(Spy, White),
            p(Archer, White),
            p(Archer, White),
            None,
            None,
            // Row I (y=8)
            p(Archer, White),
            p(Archer, White),
            p(Archer, White),
            p(Archer, White),
            p(Crown, White),
            p(Archer, White),
            p(Archer, White),
            p(Archer, White),
            p(Archer, White),
        ],
    }
}

impl CrownfallGame {
    /// Builds a fresh game for the given ruleset: the matching starting
    /// board layout, White to move, empty history.
    pub fn new(rules: CrownfallRules) -> CrownfallGame {
        let is_archers = matches!(rules.ruleset, CrownfallRuleset::Archers);
        let board = match (rules.board, is_archers) {
            (CrownfallBoardVariant::Mini, false) => mini_layout(),
            (CrownfallBoardVariant::Normal, false) => standard_layout(),
            (CrownfallBoardVariant::Grand, false) => grand_layout(),
            (CrownfallBoardVariant::Mini, true) => mini_archers_layout(),
            (CrownfallBoardVariant::Normal, true) => standard_archers_layout(),
            (CrownfallBoardVariant::Grand, true) => grand_archers_layout(),
        };
        // One fixed allocation for the game's life: the turn-limit draw
        // bounds how far `history` can grow (plus a little headroom for the
        // AI search pushing past the current turn), so reserving it up
        // front avoids doubling reallocations on the GBA's simple
        // allocator.
        let mut history = Vec::with_capacity(rules.board.total_turn_limit() as usize + 1 + 8);
        history.push(position_hash(&board, CrownfallPlayerKind::White));
        CrownfallGame {
            cache: PieceCache::rebuild(&board),
            board,
            state: CrownfallGameState::Playing(CrownfallPlayState::WaitingForInput {
                player: CrownfallPlayerKind::White,
            }),
            rules,
            history,
            moves_since_capture: 0,
        }
    }

    /// Assembles a game from an explicit board, state and rules - for tests
    /// and tools that set up hand-built positions (the derived piece cache
    /// is private, so a struct literal can't). `history` starts empty and
    /// `moves_since_capture` at 0; both are public fields, so callers can
    /// overwrite them afterwards.
    pub fn from_parts(
        board: CrownfallBoardState,
        state: CrownfallGameState,
        rules: CrownfallRules,
    ) -> CrownfallGame {
        CrownfallGame {
            cache: PieceCache::rebuild(&board),
            board,
            state,
            rules,
            history: Vec::new(),
            moves_since_capture: 0,
        }
    }
}

impl Default for CrownfallGame {
    fn default() -> Self {
        CrownfallGame::new(CrownfallRules::standard())
    }
}

impl CrownfallPlayerAction {
    pub const fn player(&self) -> CrownfallPlayerKind {
        match self {
            CrownfallPlayerAction::Move { player, .. } => *player,
            CrownfallPlayerAction::KnightRemoval { player, .. } => *player,
            CrownfallPlayerAction::Surrender { player } => *player,
        }
    }
}

impl CrownfallPlayState {
    pub const fn player(&self) -> CrownfallPlayerKind {
        match self {
            CrownfallPlayState::WaitingForInput { player } => *player,
            CrownfallPlayState::MustRemoveKnight { player, .. } => *player,
        }
    }
}

impl CrownfallBoardState {
    /// Move-candidate cells for the piece at `cell`, ignoring occupancy.
    /// Crown/Spy/Archer always use the plain orthogonal table. Knights use
    /// the ortho-minus-backward table normally, or the diagonal-forward
    /// table under `rules.knights_move_diagonally` (variant 6) - a straight
    /// ROM lookup either way: no coordinate math, no bounds branches, no
    /// allocation.
    pub(crate) fn move_candidates(
        &self,
        cell: CrownfallBoardCell,
        rules: CrownfallRules,
    ) -> &'static [u8] {
        let knights_move_diagonally_enabled = if let CrownfallRuleset::Custom {
            knights_move_diagonally,
            ..
        } = rules.ruleset
        {
            knights_move_diagonally
        } else {
            false
        };
        let variant = self.variant();
        match self.cells()[cell.to_index()] {
            Some(piece) if piece.kind() == CrownfallPieceKind::Knight => {
                if knights_move_diagonally_enabled {
                    tables::knight_diagonal_moves(variant, piece.player(), cell.to_index())
                } else {
                    tables::knight_moves(variant, piece.player(), cell.to_index())
                }
            }
            Some(_) => tables::ortho(variant, cell.to_index()),
            None => &[],
        }
    }

    /// Legal move destinations for the piece at `cell`. Allocates the result
    /// for UI callers; the AI and move validation use `move_candidates`
    /// directly and never build this `Vec`.
    pub fn get_valid_destinations_for(
        &self,
        cell: CrownfallBoardCell,
        rules: CrownfallRules,
    ) -> Vec<CrownfallBoardCell> {
        self.move_candidates(cell, rules)
            .iter()
            .filter(|&&index| self.cells()[index as usize].is_none())
            .map(|&index| CrownfallBoardCell::new_index(index as usize))
            .collect()
    }

    /// Previews what the candidate move `from -> to` would capture, without
    /// applying it to `self` - `None` if there's no piece at `from`, or `to`
    /// isn't a legal destination for it (unoccupied and within
    /// `move_candidates`; unlike a real move, this doesn't check
    /// `rules.mandatory_capture`, since that's a whole-turn constraint
    /// across every candidate move, not a property of this one). Mirrors the
    /// priority order `CrownfallGame::apply_move`/`apply_move_sequential`/
    /// `apply_move_all_captures_processed` use when actually resolving a
    /// move (crown-loss first, then enemy-crown capture, then Spy Capture of
    /// the mover, then ordinary piece captures, then an Archer's ranged
    /// shot), so a UI can show what a move *would* do before the player
    /// commits to it via `CrownfallPlayerAction::Move`.
    pub fn preview_move_captures(
        &self,
        from: CrownfallBoardCell,
        to: CrownfallBoardCell,
        rules: CrownfallRules,
    ) -> Option<MoveCapturePreview> {
        let from_index = from.to_index();
        let to_index = to.to_index();
        let piece = self.cells()[from_index]?;
        if self.cells()[to_index].is_some()
            || !self
                .move_candidates(from, rules)
                .contains(&(to_index as u8))
        {
            return None;
        }

        let player = piece.player();
        let mut scratch = *self;
        scratch.cells_mut()[from_index] = None;
        scratch.cells_mut()[to_index] = Some(piece);

        // Crown-loss takes priority over every other check, own Crown
        // included, and pre-empts anything else the move would otherwise do
        // - matches `CrownfallGame::apply_move`.
        if scratch.check_own_crown_trap(to, player, rules) {
            return Some(MoveCapturePreview {
                captured: Vec::new(),
                mover_captured: true,
            });
        }

        let all_captures_processed_enabled = matches!(
            rules.ruleset,
            CrownfallRuleset::Custom {
                all_captures_processed: true,
                ..
            }
        );

        let mut preview = MoveCapturePreview::default();

        if let Some(surrounded_crown) = scratch.check_crown_capture(to, player, rules) {
            preview.captured.push(surrounded_crown);
            if !all_captures_processed_enabled {
                return Some(preview);
            }
        }

        if scratch.check_self_spy_trap(to, player, rules) {
            preview.mover_captured = true;
            if !all_captures_processed_enabled {
                return Some(preview);
            }
        }

        let (piece_captures, count) = scratch.check_piece_captures(to, player, rules);
        if count > 0 {
            preview
                .captured
                .extend(piece_captures[..count].iter().map(|c| c.target));
            if !all_captures_processed_enabled {
                return Some(preview);
            }
        }

        if piece.kind() == CrownfallPieceKind::Archer {
            let (archer_captures, archer_count) = scratch.check_archer_capture(to, player);
            preview.captured.extend(
                archer_captures[..archer_count]
                    .iter()
                    .map(|(target, _)| *target),
            );
        }

        Some(preview)
    }

    /// The table used to find valid Knight *attacker* positions relative to
    /// a target: the diagonal-forward arc normally, or the ortho-minus-
    /// backward shape under variant 6 (where movement and capture-arc swap
    /// roles relative to Standard).
    fn knight_capture_shape(
        &self,
        player: CrownfallPlayerKind,
        index: usize,
        rules: CrownfallRules,
    ) -> &'static [u8] {
        let knights_move_diagonally_enabled = if let CrownfallRuleset::Custom {
            knights_move_diagonally,
            ..
        } = rules.ruleset
        {
            knights_move_diagonally
        } else {
            false
        };
        if knights_move_diagonally_enabled {
            tables::knight_moves(self.variant(), player, index)
        } else {
            tables::knight_arcs(self.variant(), player, index)
        }
    }

    /// True if `attacker_cell` is in the "exposed" (non-straight) subset of
    /// `attacker`'s capture shape toward `target` - the two diagonal cells
    /// under Standard rules, or the left/right ortho cells (excluding
    /// straight-ahead) under variant 6. A Knight that just moved must land
    /// here (not merely anywhere in the capture shape) to be the piece
    /// completing a Knight Capture pincer - see
    /// `check_piece_captures`/`moved_knight_completes_pincer`.
    fn is_capture_landing_spot_of(
        &self,
        target: CrownfallBoardCell,
        attacker_cell: CrownfallBoardCell,
        attacker: CrownfallPlayerKind,
        rules: CrownfallRules,
    ) -> bool {
        let variant = self.variant();
        let (ax, _) = tables::coord(variant, attacker_cell.to_index());
        let (tx, _) = tables::coord(variant, target.to_index());
        if ax == tx {
            // Straight-ahead is never the "exposed" cell, in either shape.
            return false;
        }
        self.knight_capture_shape(attacker.opposite(), target.to_index(), rules)
            .contains(&(attacker_cell.to_index() as u8))
    }

    /// True if at least 3 of the 5 cells forming the extended Knight-mass
    /// arc around `target` (the 3-cell forward Knight-capture shape used by
    /// `find_attacking_pair`, plus the two orthogonal flank cells - same
    /// row, one column either side) are occupied by `attacker`-owned
    /// Knights. Only consulted when `CrownfallRuleset::Custom`'s
    /// `knight_mass_capture` toggle is enabled (see `lib.rs`), to waive the
    /// usual Knight Capture self-sacrifice when the attacker locally
    /// outnumbers the defender 3-to-1 in Knights.
    fn has_mass_knight_arc(
        &self,
        target: CrownfallBoardCell,
        attacker: CrownfallPlayerKind,
        rules: CrownfallRules,
    ) -> bool {
        let variant = self.variant();
        let target_index = target.to_index();
        let (_, target_y) = tables::coord(variant, target_index);
        let is_attacker_knight = |cell: u8| matches!(self.cells()[cell as usize], Some(piece) if piece.player() == attacker && piece.kind() == CrownfallPieceKind::Knight);
        let flank_count = tables::ortho(variant, target_index)
            .iter()
            .filter(|&&cell| {
                tables::coord(variant, cell as usize).1 == target_y && is_attacker_knight(cell)
            })
            .count();
        let forward_count = self
            .knight_capture_shape(attacker.opposite(), target_index, rules)
            .iter()
            .filter(|&&cell| is_attacker_knight(cell))
            .count();
        flank_count + forward_count >= 3
    }

    /// First pair among `attackers` whose piece kinds form a valid capture,
    /// in the order the attackers were gathered.
    fn first_capturing_pair(
        &self,
        attackers: &[u8],
    ) -> Option<((CrownfallBoardCell, CrownfallBoardCell), CaptureKind)> {
        for i in 0..attackers.len() {
            for j in (i + 1)..attackers.len() {
                let pair = (
                    CrownfallBoardCell::new_index(attackers[i] as usize),
                    CrownfallBoardCell::new_index(attackers[j] as usize),
                );
                if let Some(kind) = self.capture_kind(pair) {
                    return Some((pair, kind));
                }
            }
        }
        None
    }

    /// Finds a valid capturing pincer against `target` occupied by `attacker`-owned
    /// pieces. Crown and Spy attackers only need plain orthogonal adjacency (any of
    /// the 4 sides); Knight attackers additionally need `target` to fall within their
    /// own capture shape (see `knight_capture_shape`) - a Knight standing outside that
    /// shape cannot form a Knight Capture pincer. Whether the just-moved piece
    /// specifically must land in the shape's exposed subset (not just anywhere in it)
    /// is enforced by callers via `is_capture_landing_spot_of`, not here - this only
    /// finds *some* valid pair. Archers are never valid attackers here (see
    /// `capture_kind` - no pair including an Archer ever matches). Extra
    /// attacker-owned pieces also adjacent to `target` (of any kind) must not block a
    /// genuine pincer formed by two others.
    fn find_attacking_pair(
        &self,
        target: CrownfallBoardCell,
        attacker: CrownfallPlayerKind,
        rules: CrownfallRules,
    ) -> Option<((CrownfallBoardCell, CrownfallBoardCell), CaptureKind)> {
        let variant = self.variant();
        // At most 4 orthogonal non-Knight attackers + 3 shape-matching Knights.
        let mut attackers = [0u8; 7];
        let mut len = 0;
        for &neighbour in tables::ortho(variant, target.to_index()) {
            if matches!(self.cells()[neighbour as usize], Some(piece) if piece.player() == attacker && piece.kind() != CrownfallPieceKind::Knight)
            {
                attackers[len] = neighbour;
                len += 1;
            }
        }
        for &neighbour in self.knight_capture_shape(attacker.opposite(), target.to_index(), rules) {
            if matches!(self.cells()[neighbour as usize], Some(piece) if piece.player() == attacker && piece.kind() == CrownfallPieceKind::Knight)
            {
                attackers[len] = neighbour;
                len += 1;
            }
        }
        self.first_capturing_pair(&attackers[..len])
    }

    /// Determines which capture rule (if any) the attacking pair satisfies.
    ///
    /// The Crown may only stand in for a Knight, never a Spy (README "Crown" section),
    /// so a Crown+Spy pair does not form a valid capture. Archers never form a valid
    /// pair here (they have their own ranged mechanic, see `check_archer_capture`) -
    /// every arm involving `Archer` falls through to `_ => None`.
    fn capture_kind(
        &self,
        attackers: (CrownfallBoardCell, CrownfallBoardCell),
    ) -> Option<CaptureKind> {
        let a = self.cells()[attackers.0.to_index()]?.kind();
        let b = self.cells()[attackers.1.to_index()]?.kind();
        match (a, b) {
            (CrownfallPieceKind::Spy, CrownfallPieceKind::Spy) => Some(CaptureKind::Spy),
            (CrownfallPieceKind::Knight, CrownfallPieceKind::Knight) => Some(CaptureKind::Knight),
            (CrownfallPieceKind::Crown, CrownfallPieceKind::Knight)
            | (CrownfallPieceKind::Knight, CrownfallPieceKind::Crown) => Some(CaptureKind::Knight),
            _ => None,
        }
    }

    /// Finds a valid capturing pincer against the Crown at `target`, occupied by
    /// `attacker`-owned pieces. Any of the Crown's orthogonally adjacent tiles counts
    /// unconditionally (any side, whether that piece just moved or was already in
    /// place) - Crown captures are not bound by the Knight capture-shape restriction the
    /// way ordinary Knight Captures are. However, a Knight can *also* attack from the
    /// exposed subset of its capture shape (outside plain orthogonal adjacency) if -
    /// and only if - `moved` is that Knight: that reach only activates for the
    /// Knight that's actively moving into it this turn, never for one that was
    /// already sitting there (see README "Captures" - "invalid" example).
    fn find_crown_attacking_pair(
        &self,
        target: CrownfallBoardCell,
        attacker: CrownfallPlayerKind,
        moved: CrownfallBoardCell,
        rules: CrownfallRules,
    ) -> Option<((CrownfallBoardCell, CrownfallBoardCell), CaptureKind)> {
        let variant = self.variant();
        // At most 4 orthogonal attackers + the just-moved exposed-shape Knight.
        let mut attackers = [0u8; 5];
        let mut len = 0;
        for &neighbour in tables::ortho(variant, target.to_index()) {
            if matches!(self.cells()[neighbour as usize], Some(piece) if piece.player() == attacker)
            {
                attackers[len] = neighbour;
                len += 1;
            }
        }

        if self.is_capture_landing_spot_of(target, moved, attacker, rules)
            && matches!(self.cells()[moved.to_index()], Some(piece) if piece.player() == attacker && piece.kind() == CrownfallPieceKind::Knight)
        {
            attackers[len] = moved.to_index() as u8;
            len += 1;
        }

        self.first_capturing_pair(&attackers[..len])
    }

    /// The attacker's own piece other than `moved` in an attacking pair.
    fn other_attacker(
        attackers: (CrownfallBoardCell, CrownfallBoardCell),
        moved: CrownfallBoardCell,
    ) -> CrownfallBoardCell {
        if attackers.0 == moved {
            attackers.1
        } else {
            attackers.0
        }
    }

    /// True if the piece just moved to `at` (owned by `mover`) is captured by a
    /// pre-existing enemy Spy pair - the Spy Capture rule applies "even if the enemy
    /// moved there" (README "Spy Capture"). The Crown is exempt: its own capture is
    /// governed exclusively by the higher-priority crown-loss check.
    fn check_self_spy_trap(
        &self,
        at: CrownfallBoardCell,
        mover: CrownfallPlayerKind,
        rules: CrownfallRules,
    ) -> bool {
        match self.cells()[at.to_index()] {
            Some(piece) if piece.player() == mover && piece.kind() != CrownfallPieceKind::Crown => {
                self.find_attacking_pair(at, mover.opposite(), rules)
                    .map(|(_, kind)| kind)
                    == Some(CaptureKind::Spy)
            }
            _ => false,
        }
    }

    /// True if the Crown just moved to `at` (owned by `mover`) walked into a
    /// pre-existing enemy attacking pair. Crown-loss has the highest priority of any
    /// capture (README "Losing the Game"), so this must be checked before any other
    /// capture the same move might otherwise complete.
    fn check_own_crown_trap(
        &self,
        at: CrownfallBoardCell,
        mover: CrownfallPlayerKind,
        rules: CrownfallRules,
    ) -> bool {
        match self.cells()[at.to_index()] {
            Some(piece) if piece.player() == mover && piece.kind() == CrownfallPieceKind::Crown => {
                self.find_crown_attacking_pair(at, mover.opposite(), at, rules)
                    .is_some()
            }
            _ => false,
        }
    }

    /// Cells a just-moved piece at `to` might now be threatening as an attacker: its
    /// plain orthogonal neighbours, plus - if it's a Knight - its capture shape, since
    /// a Knight's capture reach extends beyond plain adjacency (see
    /// `knight_capture_shape`).
    fn capture_scan_cells(
        &self,
        to: CrownfallBoardCell,
        mover: CrownfallPlayerKind,
        rules: CrownfallRules,
    ) -> ScanCells {
        let variant = self.variant();
        let mut cells = [0u8; MAX_SCAN_CELLS];
        let mut len = 0;
        for &neighbour in tables::ortho(variant, to.to_index()) {
            cells[len] = neighbour;
            len += 1;
        }
        if matches!(self.cells()[to.to_index()], Some(piece) if piece.kind() == CrownfallPieceKind::Knight)
        {
            for &cell in self.knight_capture_shape(mover, to.to_index(), rules) {
                if !cells[..len].contains(&cell) {
                    cells[len] = cell;
                    len += 1;
                }
            }
        }
        (cells, len)
    }

    /// True unless `mover_piece` is a Knight that just moved to `to` and would be
    /// completing the pincer against `target` by landing in the shape's non-exposed
    /// (straight-ahead) cell. A Knight can only be the piece that *springs* a Knight
    /// Capture pincer if it lands in the exposed subset of its capture shape - a
    /// partner Knight already in place may sit straight-ahead, but the just-moved
    /// piece may not (see `is_capture_landing_spot_of`). Non-Knight movers (Crown,
    /// Spy, Archer) are unrestricted.
    fn moved_knight_completes_pincer(
        &self,
        to: CrownfallBoardCell,
        target: CrownfallBoardCell,
        attacker: CrownfallPlayerKind,
        rules: CrownfallRules,
    ) -> bool {
        match self.cells()[to.to_index()] {
            Some(piece) if piece.kind() == CrownfallPieceKind::Knight => {
                self.is_capture_landing_spot_of(target, to, attacker, rules)
            }
            _ => true,
        }
    }

    fn check_crown_capture(
        &self,
        to: CrownfallBoardCell,
        attacker: CrownfallPlayerKind,
        rules: CrownfallRules,
    ) -> Option<CrownfallBoardCell> {
        let (cells, len) = self.capture_scan_cells(to, attacker, rules);
        for &index in &cells[..len] {
            let Some(piece) = self.cells()[index as usize] else {
                continue;
            };
            if piece.player() == attacker || piece.kind() != CrownfallPieceKind::Crown {
                continue;
            }
            let neighbour = CrownfallBoardCell::new_index(index as usize);
            if self
                .find_crown_attacking_pair(neighbour, attacker, to, rules)
                .is_some()
            {
                return Some(neighbour);
            }
        }
        None
    }

    fn check_piece_captures(
        &self,
        to: CrownfallBoardCell,
        attacker: CrownfallPlayerKind,
        rules: CrownfallRules,
    ) -> PieceCaptures {
        let placeholder = PieceCapture {
            target: CrownfallBoardCell { index: 0 },
            kind: CaptureKind::Spy,
            attackers: (
                CrownfallBoardCell { index: 0 },
                CrownfallBoardCell { index: 0 },
            ),
        };
        let mut captures = [placeholder; MAX_SCAN_CELLS];
        let mut count = 0;
        let (cells, len) = self.capture_scan_cells(to, attacker, rules);
        for &index in &cells[..len] {
            let Some(piece) = self.cells()[index as usize] else {
                continue;
            };
            if piece.player() == attacker || piece.kind() == CrownfallPieceKind::Crown {
                continue;
            }
            let target = CrownfallBoardCell::new_index(index as usize);
            // Cheap arc check first - the pair search is the expensive part.
            if !self.moved_knight_completes_pincer(to, target, attacker, rules) {
                continue;
            }
            let Some((attackers, kind)) = self.find_attacking_pair(target, attacker, rules) else {
                continue;
            };
            captures[count] = PieceCapture {
                target,
                kind,
                attackers,
            };
            count += 1;
        }
        (captures, count)
    }

    /// An Archer's ranged capture: for each enemy piece exactly 2 orthogonal
    /// tiles from `to` (the Archer that just moved), the shot lands if any
    /// allied Crown/Knight/Spy (never another Archer) is orthogonally
    /// adjacent to that target. No attacker sacrifice, no pincer partner -
    /// only called when the piece at `to` is itself an Archer.
    fn check_archer_capture(
        &self,
        to: CrownfallBoardCell,
        attacker: CrownfallPlayerKind,
    ) -> ArcherCaptures {
        let variant = self.variant();
        let mut captures = [(
            CrownfallBoardCell { index: 0 },
            CrownfallBoardCell { index: 0 },
        ); MAX_ARCHER_TARGETS];
        let mut count = 0;
        for &index in tables::archer_range(variant, to.to_index()) {
            let Some(piece) = self.cells()[index as usize] else {
                continue;
            };
            if piece.player() == attacker || piece.kind() == CrownfallPieceKind::Crown {
                continue;
            }
            let ally = tables::ortho(variant, index as usize).iter().find(|&&n| {
                matches!(self.cells()[n as usize], Some(p) if p.player() == attacker && p.kind() != CrownfallPieceKind::Archer)
            });
            if let Some(&ally) = ally {
                captures[count] = (
                    CrownfallBoardCell::new_index(index as usize),
                    CrownfallBoardCell::new_index(ally as usize),
                );
                count += 1;
            }
        }
        (captures, count)
    }

    /// True if `player` has at least one legal move this turn that results in
    /// a capture of an enemy piece (crown capture, ordinary Knight/Spy
    /// capture, or an Archer's ranged shot) - used to enforce
    /// `rules.mandatory_capture`. Does not count a move that merely walks
    /// the mover into a trap of their own pieces.
    pub(crate) fn has_available_capture(
        &self,
        player: CrownfallPlayerKind,
        rules: CrownfallRules,
    ) -> bool {
        // One board copy for the whole scan; each candidate move is played
        // as two cell writes and undone the same way, instead of re-copying
        // the entire board per candidate.
        let mut scratch = *self;
        for (index, cell) in self.cells().iter().enumerate() {
            let Some(piece) = cell else { continue };
            if piece.player() != player {
                continue;
            }
            let from = CrownfallBoardCell::new_index(index);
            for &dest in self.move_candidates(from, rules) {
                if self.cells()[dest as usize].is_some() {
                    continue;
                }
                let to = CrownfallBoardCell::new_index(dest as usize);
                scratch.cells_mut()[index] = None;
                scratch.cells_mut()[dest as usize] = Some(*piece);
                let captures = scratch.move_captures_something(to, player, piece.kind(), rules);
                scratch.cells_mut()[dest as usize] = None;
                scratch.cells_mut()[index] = Some(*piece);
                if captures {
                    return true;
                }
            }
        }
        false
    }

    /// True if the piece that just moved to `to` (of `kind`, owned by
    /// `player`) captured at least one enemy piece - ignores traps the
    /// mover walked into against their own pieces, since those aren't a
    /// capture *by* the mover. Used both by `has_available_capture` and by
    /// `apply_move`'s mandatory-capture check on the attempted move itself.
    pub(crate) fn move_captures_something(
        &self,
        to: CrownfallBoardCell,
        player: CrownfallPlayerKind,
        kind: CrownfallPieceKind,
        rules: CrownfallRules,
    ) -> bool {
        if self.check_crown_capture(to, player, rules).is_some() {
            return true;
        }
        if self.check_piece_captures(to, player, rules).1 > 0 {
            return true;
        }
        if kind == CrownfallPieceKind::Archer && self.check_archer_capture(to, player).1 > 0 {
            return true;
        }
        false
    }
}

impl CrownfallGame {
    /// Turns remaining before the board's turn-limit safety-net draw fires,
    /// regardless of repetition or recent captures.
    pub fn turns_remaining(&self) -> u16 {
        let turns_played = (self.history.len() - 1) as u16;
        self.rules
            .board
            .total_turn_limit()
            .saturating_sub(turns_played)
    }

    /// Turns remaining before the no-progress draw fires if no capture
    /// happens in the meantime (chess's 50-move rule, adapted).
    pub fn turns_remaining_before_no_progress_draw(&self) -> u16 {
        self.rules
            .board
            .no_progress_limit()
            .saturating_sub(self.moves_since_capture)
    }

    pub fn handle_player_action(
        mut self,
        action: CrownfallPlayerAction,
    ) -> Result<(CrownfallGame, Option<CrownfallTurnResult>), CrownfallError> {
        let result = self.apply_action(action)?;
        Ok((self, result))
    }

    /// In-place equivalent of `handle_player_action`: applies `action`
    /// directly to this game instead of consuming and returning it, so
    /// callers that would otherwise `clone()` first (every node of the AI
    /// search, most importantly) don't have to copy the ever-growing
    /// position `history`. On `Err` the game is guaranteed untouched -
    /// every validation runs before the first mutation.
    pub fn apply_action(
        &mut self,
        action: CrownfallPlayerAction,
    ) -> Result<Option<CrownfallTurnResult>, CrownfallError> {
        // Real gameplay never rolls a move back, so the journal is discarded.
        self.apply_action_with_logging(action, true, true, &mut MoveScratch::new())
    }

    /// Applies `action` without logging the move/capture, used by the AI's search
    /// (`ai::best_move`/`negamax`) to explore candidate positions - those simulated
    /// moves aren't real turns and would otherwise drown out actual gameplay in the
    /// log (see `game::ai`, which calls this instead of `apply_action`).
    ///
    /// Also skips re-validating `rules.mandatory_capture`: the AI only ever
    /// applies moves from `ai::collect_moves`, which has already filtered the
    /// list down to capturing moves (using the same `move_captures_something`
    /// machinery `apply_move` would re-run) whenever any exist - so the
    /// re-check could never fail, and it costs a full own-moves capture scan
    /// per quiet move applied.
    /// `scratch` collects the journal of overwritten cells, which is what
    /// lets the search undo the move without a whole-board copy (see
    /// `CellJournal::undo`).
    pub(crate) fn apply_action_quiet(
        &mut self,
        action: CrownfallPlayerAction,
        scratch: &mut MoveScratch,
    ) -> Result<Option<CrownfallTurnResult>, CrownfallError> {
        self.apply_action_with_logging(action, false, false, scratch)
    }

    fn apply_action_with_logging(
        &mut self,
        action: CrownfallPlayerAction,
        log_moves: bool,
        validate_mandatory_capture: bool,
        scratch: &mut MoveScratch,
    ) -> Result<Option<CrownfallTurnResult>, CrownfallError> {
        if log_moves {
            // Real gameplay actions always recount rather than trusting the
            // cache: `board` is a public field and at least one UI caller
            // writes cells directly (the client's optimistic drag moves),
            // which the cache can't observe. One board scan per real move is
            // what the old per-capture counting cost anyway. The AI's quiet
            // path skips this - the search owns its game clone, so nothing
            // can touch the board behind the incrementally-maintained cache.
            self.cache = PieceCache::rebuild(&self.board);
        } else {
            self.ensure_cache();
        }
        match &self.state {
            CrownfallGameState::Playing(play_state) => {
                if play_state.player() != action.player() {
                    return Err(CrownfallError::NotYourTurn(action.player()));
                }
            }
            CrownfallGameState::Victory(_, _) => {
                return Err(CrownfallError::GameOver(action.player()));
            }
            CrownfallGameState::Draw(_) => return Err(CrownfallError::GameOver(action.player())),
        }
        let result = match action {
            CrownfallPlayerAction::Move { player, from, to } => self.apply_move(
                player,
                from,
                to,
                log_moves,
                validate_mandatory_capture,
                scratch,
            ),
            CrownfallPlayerAction::KnightRemoval { player, at } => {
                self.apply_knight_removal(player, at, scratch)
            }
            CrownfallPlayerAction::Surrender { player } => {
                self.state = CrownfallGameState::Victory(player.opposite(), WinReason::Surrender);
                Ok(None)
            }
        };
        #[cfg(feature = "log")]
        if log_moves {
            match self.state {
                CrownfallGameState::Victory(player, reason) => {
                    log::info!("game over: {player:?} wins ({})", reason.description());
                }
                CrownfallGameState::Draw(reason) => {
                    log::info!("game over: draw ({})", reason.description());
                }
                CrownfallGameState::Playing(_) => {}
            }
        }
        result
    }

    #[cfg_attr(not(feature = "log"), allow(unused_variables))]
    fn apply_move(
        &mut self,
        player: CrownfallPlayerKind,
        from: CrownfallBoardCell,
        to: CrownfallBoardCell,
        log_moves: bool,
        validate_mandatory_capture: bool,
        scratch: &mut MoveScratch,
    ) -> Result<Option<CrownfallTurnResult>, CrownfallError> {
        let from_index = from.to_index();
        let to_index = to.to_index();
        let cell_count = tables::cell_count(self.rules.board);
        // `Cell` is just a deserialized index - reject out-of-range ones here
        // rather than panicking on a board access (and `to` must be checked
        // before it's truncated to `u8` for the candidate-table comparison).
        if from_index >= cell_count {
            return Err(CrownfallError::EmptyMove(player, from));
        }
        if to_index >= cell_count {
            return Err(CrownfallError::InvalidDestination(player, from, to));
        }
        let piece =
            self.board.cells()[from_index].ok_or(CrownfallError::EmptyMove(player, from))?;
        if piece.player() != player {
            return Err(CrownfallError::EnemyMove(player, from));
        }
        if self.board.cells()[to_index].is_some()
            || !self
                .board
                .move_candidates(from, self.rules)
                .contains(&(to_index as u8))
        {
            return Err(CrownfallError::InvalidDestination(player, from, to));
        }

        let must_capture_rule_enabled = if let CrownfallRuleset::Custom {
            mandatory_capture, ..
        } = self.rules.ruleset
        {
            mandatory_capture
        } else {
            false
        };
        if must_capture_rule_enabled && validate_mandatory_capture {
            let mut probe = self.board;
            probe.cells_mut()[from_index] = None;
            probe.cells_mut()[to_index] = Some(piece);
            let this_move_captures =
                probe.move_captures_something(to, player, piece.kind(), self.rules);
            if !this_move_captures && self.board.has_available_capture(player, self.rules) {
                return Err(CrownfallError::CaptureRequired(player));
            }
        }

        #[cfg(feature = "log")]
        if log_moves {
            log::info!(
                "{player:?} moves {:?} from {} to {}",
                piece.kind(),
                LogCoord(from, self.rules.board),
                LogCoord(to, self.rules.board)
            );
        }

        // Everything from here on mutates through `write_cell`/`take_cell`,
        // which fold each change's Zobrist keys into `scratch.hash_delta`
        // (see `resolve_continuation`) and journal it for make/unmake.
        self.write_cell(from_index, None, scratch);
        self.write_cell(to_index, Some(piece), scratch);

        // Crown-loss has the highest priority of any capture and is checked first,
        // even ahead of a capture this same move would otherwise complete (README
        // "Crown" section - the crown moving into a trap loses the game outright).
        // This holds regardless of `all_captures_processed`: crown loss always ends
        // the game immediately, so there's nothing left to "also process".
        if self.board.check_own_crown_trap(to, player, self.rules) {
            self.take_cell(to_index, scratch);
            #[cfg(feature = "log")]
            if log_moves {
                log::info!(
                    "captured: {player:?} Crown at {}",
                    LogCoord(to, self.rules.board)
                );
            }
            self.state = CrownfallGameState::Victory(player.opposite(), WinReason::CrownCaptured);
            return Ok(Some(CrownfallTurnResult::Victory {
                player: player.opposite(),
                surrounded_crown: to,
            }));
        }

        let all_captures_processed_enabled = if let CrownfallRuleset::Custom {
            all_captures_processed,
            ..
        } = self.rules.ruleset
        {
            all_captures_processed
        } else {
            false
        };
        if all_captures_processed_enabled {
            self.apply_move_all_captures_processed(player, from, to, piece, log_moves, scratch)
        } else {
            self.apply_move_sequential(player, from, to, piece, log_moves, scratch)
        }
    }

    /// Standard capture resolution: the first of these that applies wins,
    /// and later checks never run (README priority order).
    fn apply_move_sequential(
        &mut self,
        player: CrownfallPlayerKind,
        from: CrownfallBoardCell,
        to: CrownfallBoardCell,
        piece: CrownfallPiece,
        log_moves: bool,
        scratch: &mut MoveScratch,
    ) -> Result<Option<CrownfallTurnResult>, CrownfallError> {
        let to_index = to.to_index();

        if let Some(surrounded_crown) = self.board.check_crown_capture(to, player, self.rules) {
            self.take_cell(surrounded_crown.to_index(), scratch);
            #[cfg(feature = "log")]
            if log_moves {
                log::info!(
                    "captured: {:?} Crown at {}",
                    player.opposite(),
                    LogCoord(surrounded_crown, self.rules.board)
                );
            }
            self.state = CrownfallGameState::Victory(player, WinReason::CrownCaptured);
            return Ok(Some(CrownfallTurnResult::Victory {
                player,
                surrounded_crown,
            }));
        }

        // Spy Capture applies "even if the enemy moved there" - the piece just moved
        // can walk straight into a pre-existing enemy Spy pincer and be captured by it.
        if self.board.check_self_spy_trap(to, player, self.rules) {
            self.take_cell(to_index, scratch);
            #[cfg(feature = "log")]
            if log_moves {
                log::info!(
                    "captured: {player:?} {:?} at {}",
                    piece.kind(),
                    LogCoord(to, self.rules.board)
                );
            }
            let attackers = self
                .board
                .find_attacking_pair(to, player.opposite(), self.rules)
                .expect("check_self_spy_trap confirmed an attacking pair");
            self.state = self.resolve_after_removal(player, true, scratch.hash_delta);
            return Ok(Some(CrownfallTurnResult::Capture {
                player,
                last_move_from: from,
                last_move_to: to,
                removed: to,
                second_attacker: attackers.0.1,
            }));
        }

        let (captures, capture_count) = self.board.check_piece_captures(to, player, self.rules);
        let (archer_captures, archer_count) = if piece.kind() == CrownfallPieceKind::Archer {
            self.board.check_archer_capture(to, player)
        } else {
            (
                [(
                    CrownfallBoardCell { index: 0 },
                    CrownfallBoardCell { index: 0 },
                ); MAX_ARCHER_TARGETS],
                0,
            )
        };

        if capture_count > 0 {
            let turn_result = self.apply_piece_captures(
                &captures[..capture_count],
                player,
                from,
                to,
                piece,
                log_moves,
                scratch,
            );
            self.state = self.resolve_after_removal(player, true, scratch.hash_delta);
            return Ok(Some(turn_result));
        }

        if archer_count > 0 {
            let turn_result = self.apply_archer_captures(
                &archer_captures[..archer_count],
                player,
                from,
                to,
                log_moves,
                scratch,
            );
            self.state = self.resolve_after_removal(player, true, scratch.hash_delta);
            return Ok(Some(turn_result));
        }

        self.state = resolve_continuation(
            &self.board,
            self.rules.board,
            player.opposite(),
            &mut self.history,
            &mut self.moves_since_capture,
            false,
            scratch.hash_delta,
        );
        Ok(Some(CrownfallTurnResult::PieceMove { player, from, to }))
    }

    /// Variant 5: evaluate crown-capture / self-spy-trap / piece-captures /
    /// archer-captures all against the same immediately-post-move board
    /// snapshot (crown-loss aside, already handled by the caller), then
    /// apply every removal each independently-evaluated check found. A move
    /// that both walks the mover into an enemy pincer and completes the
    /// mover's own pincer resolves both, instead of one pre-empting the
    /// other.
    fn apply_move_all_captures_processed(
        &mut self,
        player: CrownfallPlayerKind,
        from: CrownfallBoardCell,
        to: CrownfallBoardCell,
        piece: CrownfallPiece,
        log_moves: bool,
        scratch: &mut MoveScratch,
    ) -> Result<Option<CrownfallTurnResult>, CrownfallError> {
        let to_index = to.to_index();
        let snapshot = self.board;

        let crown_capture = snapshot.check_crown_capture(to, player, self.rules);
        let self_trapped = snapshot.check_self_spy_trap(to, player, self.rules);
        let (captures, capture_count) = snapshot.check_piece_captures(to, player, self.rules);
        let (archer_captures, archer_count) = if piece.kind() == CrownfallPieceKind::Archer {
            snapshot.check_archer_capture(to, player)
        } else {
            (
                [(
                    CrownfallBoardCell { index: 0 },
                    CrownfallBoardCell { index: 0 },
                ); MAX_ARCHER_TARGETS],
                0,
            )
        };

        let mut any_capture = false;
        let mut turn_result = None;

        if let Some(surrounded_crown) = crown_capture {
            self.take_cell(surrounded_crown.to_index(), scratch);
            #[cfg(feature = "log")]
            if log_moves {
                log::info!(
                    "captured: {:?} Crown at {}",
                    player.opposite(),
                    LogCoord(surrounded_crown, self.rules.board)
                );
            }
            any_capture = true;
            turn_result.get_or_insert(CrownfallTurnResult::Victory {
                player,
                surrounded_crown,
            });
        }

        if self_trapped {
            self.take_cell(to_index, scratch);
            #[cfg(feature = "log")]
            if log_moves {
                log::info!(
                    "captured: {player:?} {:?} at {}",
                    piece.kind(),
                    LogCoord(to, self.rules.board)
                );
            }
            let attackers = snapshot
                .find_attacking_pair(to, player.opposite(), self.rules)
                .expect("check_self_spy_trap confirmed an attacking pair");
            any_capture = true;
            turn_result.get_or_insert(CrownfallTurnResult::Capture {
                player,
                last_move_from: from,
                last_move_to: to,
                removed: to,
                second_attacker: attackers.0.1,
            });
        }

        if capture_count > 0 {
            let result = self.apply_piece_captures(
                &captures[..capture_count],
                player,
                from,
                to,
                piece,
                log_moves,
                scratch,
            );
            any_capture = true;
            turn_result.get_or_insert(result);
        }

        if archer_count > 0 {
            let result = self.apply_archer_captures(
                &archer_captures[..archer_count],
                player,
                from,
                to,
                log_moves,
                scratch,
            );
            any_capture = true;
            turn_result.get_or_insert(result);
        }

        if crown_capture.is_some() {
            // The Crown was captured outright this move - that always ends
            // the game, regardless of what else also happened.
            self.state = CrownfallGameState::Victory(player, WinReason::CrownCaptured);
            return Ok(turn_result);
        }

        if any_capture {
            self.state = self.resolve_after_removal(player, true, scratch.hash_delta);
            return Ok(turn_result);
        }

        self.state = resolve_continuation(
            &self.board,
            self.rules.board,
            player.opposite(),
            &mut self.history,
            &mut self.moves_since_capture,
            false,
            scratch.hash_delta,
        );
        Ok(Some(CrownfallTurnResult::PieceMove { player, from, to }))
    }

    /// Removes every target in `captures`, sacrificing the attacking Knight
    /// where the rule requires it (README "Knight Capture"), and returns
    /// the `TurnResult` for the first capture found (matching the existing
    /// single-result-per-move reporting shape).
    #[cfg_attr(not(feature = "log"), allow(unused_variables))]
    // Private helper threading one move's context through; a params struct
    // would just rename the same eight things.
    #[allow(clippy::too_many_arguments)]
    fn apply_piece_captures(
        &mut self,
        captures: &[PieceCapture],
        player: CrownfallPlayerKind,
        from: CrownfallBoardCell,
        to: CrownfallBoardCell,
        piece: CrownfallPiece,
        log_moves: bool,
        scratch: &mut MoveScratch,
    ) -> CrownfallTurnResult {
        let mut turn_result = None;
        for capture in captures {
            let target_kind = self
                .take_cell(capture.target.to_index(), scratch)
                .map(|target_piece| target_piece.kind());
            #[cfg(feature = "log")]
            if log_moves {
                log::info!(
                    "captured: {:?} {:?} at {}",
                    player.opposite(),
                    target_kind.expect("target held a piece before removal"),
                    LogCoord(capture.target, self.rules.board)
                );
            }
            let second_attacker = CrownfallBoardState::other_attacker(capture.attackers, to);
            // The attacking player only loses one of their own knights when the
            // *captured piece itself* was a Knight (README "Knight Capture") - a
            // Knight+Knight/Knight+Crown pincer capturing a Spy carries no penalty.
            // Under the `knight_mass_capture` toggle, that sacrifice is waived
            // entirely when the attacker has massed 3+ Knights in the extended
            // arc around the target (see `has_mass_knight_arc`).
            let mass_capture_enabled = matches!(
                self.rules.ruleset,
                CrownfallRuleset::Custom {
                    knight_mass_capture: true,
                    ..
                }
            );
            let sacrifice_waived = mass_capture_enabled
                && self
                    .board
                    .has_mass_knight_arc(capture.target, player, self.rules);
            if capture.kind == CaptureKind::Knight
                && target_kind == Some(CrownfallPieceKind::Knight)
                && !sacrifice_waived
            {
                let lost_knight = if piece.kind() == CrownfallPieceKind::Crown {
                    second_attacker
                } else {
                    to
                };
                self.take_cell(lost_knight.to_index(), scratch);
                #[cfg(feature = "log")]
                if log_moves {
                    log::info!(
                        "sacrificed: {player:?} Knight at {}",
                        LogCoord(lost_knight, self.rules.board)
                    );
                }
            }
            let captured_this = CrownfallTurnResult::Capture {
                player,
                last_move_from: from,
                last_move_to: to,
                removed: capture.target,
                second_attacker,
            };
            turn_result.get_or_insert(captured_this);
        }
        turn_result.expect("captures is non-empty")
    }

    /// Removes every Archer-shot target - no attacker sacrifice, the Archer
    /// never moves as part of firing.
    #[cfg_attr(not(feature = "log"), allow(unused_variables))]
    fn apply_archer_captures(
        &mut self,
        captures: &[(CrownfallBoardCell, CrownfallBoardCell)],
        player: CrownfallPlayerKind,
        from: CrownfallBoardCell,
        to: CrownfallBoardCell,
        log_moves: bool,
        scratch: &mut MoveScratch,
    ) -> CrownfallTurnResult {
        let mut turn_result = None;
        for &(target, ally) in captures {
            let target_kind = self.take_cell(target.to_index(), scratch).map(|p| p.kind());
            #[cfg(feature = "log")]
            if log_moves {
                log::info!(
                    "captured: {:?} {:?} at {} (archer shot)",
                    player.opposite(),
                    target_kind.expect("target held a piece before removal"),
                    LogCoord(target, self.rules.board)
                );
            }
            let captured_this = CrownfallTurnResult::Capture {
                player,
                last_move_from: from,
                last_move_to: to,
                removed: target,
                second_attacker: ally,
            };
            turn_result.get_or_insert(captured_this);
        }
        turn_result.expect("captures is non-empty")
    }

    /// Shared end-of-turn state resolution after one or more pieces were
    /// removed this move: attrition still takes priority over an ordinary
    /// continuation, exactly as under the sequential rules - this variant
    /// only changes *which* pieces get removed, not the priority of the
    /// resulting `GameState`.
    ///
    /// Attrition is skipped entirely under the `Archers` ruleset: it only
    /// counts Knights and Spies, but an Archer-owning side can still capture
    /// (its ranged shot) with none of either left, so being reduced to
    /// Archers alone must not be treated as a loss.
    fn resolve_after_removal(
        &mut self,
        player: CrownfallPlayerKind,
        captured: bool,
        hash_delta: u32,
    ) -> CrownfallGameState {
        debug_assert!(
            self.cache.valid,
            "the apply path runs behind ensure_cache, so the counts are current"
        );
        if !self.rules.has_archers() && self.cache.attrition_defeated(player.opposite()) {
            CrownfallGameState::Victory(player, WinReason::Attrition)
        } else {
            resolve_continuation(
                &self.board,
                self.rules.board,
                player.opposite(),
                &mut self.history,
                &mut self.moves_since_capture,
                captured,
                hash_delta,
            )
        }
    }

    fn apply_knight_removal(
        &mut self,
        player: CrownfallPlayerKind,
        at: CrownfallBoardCell,
        scratch: &mut MoveScratch,
    ) -> Result<Option<CrownfallTurnResult>, CrownfallError> {
        // Same deserialized-index guard as `apply_move`.
        if at.to_index() >= tables::cell_count(self.rules.board) {
            return Err(CrownfallError::EmptyKnightRemoval(player, at));
        }
        match self.board.cells()[at.to_index()] {
            None => Err(CrownfallError::EmptyKnightRemoval(player, at)),
            Some(cell) => {
                if cell.player() == player {
                    self.take_cell(at.to_index(), scratch);
                    self.state = resolve_continuation(
                        &self.board,
                        self.rules.board,
                        player.opposite(),
                        &mut self.history,
                        &mut self.moves_since_capture,
                        true,
                        scratch.hash_delta,
                    );
                    Ok(None)
                } else {
                    Err(CrownfallError::EnemyKnightRemoval(player, at))
                }
            }
        }
    }
}

impl CrownfallPlayerKind {
    #[inline]
    pub const fn opposite(self) -> Self {
        match self {
            CrownfallPlayerKind::White => CrownfallPlayerKind::Black,
            CrownfallPlayerKind::Black => CrownfallPlayerKind::White,
        }
    }

    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            CrownfallPlayerKind::White => "White",
            CrownfallPlayerKind::Black => "Black",
        }
    }

    #[inline]
    pub const fn symbol(self) -> char {
        match self {
            CrownfallPlayerKind::White => 'W',
            CrownfallPlayerKind::Black => 'B',
        }
    }
}

impl CrownfallPieceKind {
    #[inline]
    pub const fn symbol(self) -> char {
        match self {
            CrownfallPieceKind::Crown => 'C',
            CrownfallPieceKind::Knight => 'K',
            CrownfallPieceKind::Spy => 'S',
            CrownfallPieceKind::Archer => 'A',
        }
    }

    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            CrownfallPieceKind::Crown => "Crown",
            CrownfallPieceKind::Knight => "Knight",
            CrownfallPieceKind::Spy => "Spy",
            CrownfallPieceKind::Archer => "Archer",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai;

    /// Plays AI self-play games and asserts that every incrementally
    /// maintained history hash (see `resolve_continuation`) matches a full
    /// `position_hash` recompute of the board it describes. Depth-2 self-play
    /// reliably reaches ordinary captures, Knight sacrifices, spy traps and
    /// Archer shots, which are exactly the code paths that fold removals
    /// into the hash delta.
    fn assert_hash_parity_over_selfplay(rules: CrownfallRules) {
        let mut game = CrownfallGame::new(rules);
        for _ in 0..80 {
            let CrownfallGameState::Playing(play_state) = game.state else {
                break;
            };
            let player = play_state.player();
            let Some(action) =
                ai::best_move(&game, player, 2, ai::CrownfallPersonality::Aggressive)
            else {
                break;
            };
            let history_len = game.history.len();
            game.apply_action(action).expect("AI produces legal moves");
            if game.history.len() > history_len {
                assert_eq!(
                    *game.history.last().unwrap(),
                    position_hash(&game.board, player.opposite()),
                    "incremental hash diverged from full recompute under {rules:?}"
                );
            }
        }
    }

    /// Plays AI self-play through `apply_action_quiet` - the path that
    /// trusts and incrementally maintains the piece cache instead of
    /// recounting per action (unlike `apply_action`) - and asserts the
    /// cache matches a fresh recount after every move. Depth-2 self-play
    /// reaches captures, Knight sacrifices, spy traps and Archer shots,
    /// which are exactly the mutations `write_cell` must track.
    fn assert_cache_parity_over_selfplay(rules: CrownfallRules) {
        let mut game = CrownfallGame::new(rules);
        for _ in 0..80 {
            let CrownfallGameState::Playing(play_state) = game.state else {
                break;
            };
            let player = play_state.player();
            let Some(action) =
                ai::best_move(&game, player, 2, ai::CrownfallPersonality::Aggressive)
            else {
                break;
            };
            game.apply_action_quiet(action, &mut MoveScratch::new())
                .expect("AI produces legal moves");
            let rebuilt = PieceCache::rebuild(&game.board);
            assert!(game.cache.valid, "cache must stay valid under {rules:?}");
            assert_eq!(
                game.cache.counts, rebuilt.counts,
                "piece counts diverged from a full recount under {rules:?}"
            );
            assert_eq!(
                game.cache.crowns, rebuilt.crowns,
                "crown cells diverged from a full recount under {rules:?}"
            );
        }
    }

    #[test]
    fn incremental_cache_matches_full_recount() {
        for rules in [
            CrownfallRules::standard(),
            CrownfallRules::mini(),
            CrownfallRules::grand(),
            CrownfallRules::standard_archers(),
            CrownfallRules::standard_mandatory_capture(),
            CrownfallRules::standard_all_captures_processed(),
            CrownfallRules::standard_diagonal_knights(),
        ] {
            assert_cache_parity_over_selfplay(rules);
        }
    }

    /// A single moved Knight can land in the diagonal-exposed capture-arc
    /// cell of two different enemy pieces at once, each independently
    /// paired with a different already-in-place partner Knight - the move
    /// should complete both pincers (and pay the Knight-Capture sacrifice
    /// once, for whichever of the two targets is itself a Knight), not just
    /// the first one found.
    #[test]
    fn single_move_completes_two_independent_knight_pincers() {
        use CrownfallPlayerKind::*;
        let rules = CrownfallRules::standard();
        fn c(x: usize, y: usize) -> CrownfallBoardCell {
            CrownfallBoardCell::new_coord(x, y, CrownfallBoardVariant::Normal)
        }
        fn mv(
            mut game: CrownfallGame,
            player: CrownfallPlayerKind,
            from: (usize, usize),
            to: (usize, usize),
        ) -> CrownfallGame {
            game.apply_action(CrownfallPlayerAction::Move {
                player,
                from: c(from.0, from.1),
                to: c(to.0, to.1),
            })
            .expect("legal move");
            game
        }
        let mut game = CrownfallGame::new(rules);
        game = mv(game, White, (1, 5), (1, 4));
        game = mv(game, Black, (3, 1), (3, 2));
        game = mv(game, White, (2, 5), (2, 4));
        game = mv(game, Black, (1, 1), (1, 2));
        game = mv(game, White, (0, 5), (0, 4));
        game = mv(game, Black, (5, 1), (5, 2));
        game = mv(game, White, (4, 5), (4, 4));
        game = mv(game, Black, (1, 2), (1, 3));
        game = mv(game, White, (4, 4), (3, 4));
        game = mv(game, Black, (3, 2), (3, 3));
        game = mv(game, White, (2, 4), (2, 3));
        game = mv(game, Black, (2, 1), (2, 2));

        game = mv(game, White, (1, 4), (2, 4));

        let cells = game.board.cells();
        assert!(
            cells[c(1, 3).to_index()].is_none(),
            "Black Knight at (1,3) should be captured"
        );
        assert!(
            cells[c(3, 3).to_index()].is_none(),
            "Black Spy at (3,3) should be captured"
        );
        assert!(
            cells[c(2, 4).to_index()].is_none(),
            "the moved Knight should be sacrificed for capturing the Knight at (1,3)"
        );
        assert!(
            cells[c(3, 4).to_index()].is_some(),
            "the partner Knight at (3,4) that completed the Spy pincer should survive"
        );
        assert!(
            cells[c(0, 4).to_index()].is_some(),
            "the partner Knight at (0,4) that completed the Knight pincer should survive"
        );
    }

    #[test]
    fn incremental_hash_matches_full_recompute() {
        for rules in [
            CrownfallRules::standard(),
            CrownfallRules::mini(),
            CrownfallRules::grand(),
            CrownfallRules::standard_archers(),
            CrownfallRules::mini_archers(),
            CrownfallRules::grand_archers(),
            CrownfallRules::standard_mandatory_capture(),
            CrownfallRules::standard_all_captures_processed(),
            CrownfallRules::standard_diagonal_knights(),
        ] {
            assert_hash_parity_over_selfplay(rules);
        }
    }
}
