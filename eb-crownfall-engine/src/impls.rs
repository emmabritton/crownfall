use alloc::sync::Arc;
use crate::errors::CrownfallError;
use crate::hash::position_hash;
use crate::tables;
use crate::*;
use alloc::vec;
use alloc::vec::Vec;
use crate::CrownfallPieceKind::Archer;

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
/// is the sole progress signal).
const NO_PROGRESS_LIMIT: u16 = 40;

/// Absolute turn-count safety net: the game is drawn if it's still going
/// after this many turns, regardless of repetition or progress.
const TOTAL_TURN_LIMIT: u16 = 200;

/// Records the position that's about to be played from and returns the
/// resulting `GameState` - `Draw` if this exact position has now recurred
/// `REPETITION_LIMIT` times, if `NO_PROGRESS_LIMIT` turns have passed since
/// the last capture, or if `TOTAL_TURN_LIMIT` turns have been played in
/// total; otherwise `Playing` with `next_player` to move.
fn resolve_continuation(
    board: &CrownfallBoardState,
    next_player: CrownfallPlayerKind,
    history: &mut Vec<u64>,
    moves_since_capture: &mut u16,
    captured: bool,
) -> CrownfallGameState {
    if captured {
        *moves_since_capture = 0;
    } else {
        *moves_since_capture += 1;
    }

    let key = position_hash(board, next_player);
    history.push(key);
    // Newest-first with an early exit at the limit: this runs on every applied
    // move (including AI-search nodes), and near-repetitions cluster at the
    // recent end of the history.
    let mut repeats = 0;
    for &hash in history.iter().rev() {
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
    } else if *moves_since_capture >= NO_PROGRESS_LIMIT {
        CrownfallGameState::Draw(DrawReason::NoProgress)
    } else if turns_played >= TOTAL_TURN_LIMIT {
        CrownfallGameState::Draw(DrawReason::TurnLimit)
    } else {
        CrownfallGameState::Playing(CrownfallPlayState::WaitingForInput {
            player: next_player,
        })
    }
}

/// Shorthand for building a starting layout.
fn p(kind: CrownfallPieceKind, player: CrownfallPlayerKind) -> Option<CrownfallPiece> {
    Some(CrownfallPiece { kind, player })
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
    use CrownfallPieceKind::{Crown, Knight, Spy};
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

/// The Mini (5x5) starting layout: 4 Knights and 1 Spy in a single row in
/// front of each Crown, flanked by two more Spies.
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
    use CrownfallPieceKind::{Crown, Knight, Spy};
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
    use CrownfallPieceKind::{Archer, Crown, Knight, Spy};
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
        let history = vec![position_hash(&board, CrownfallPlayerKind::White)];
        CrownfallGame {
            board,
            state: CrownfallGameState::Playing(CrownfallPlayState::WaitingForInput {
                player: CrownfallPlayerKind::White,
            }),
            rules,
            history,
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
        let knights_move_diagonally_enabled = if let CrownfallRuleset::Custom { knights_move_diagonally,..} = rules.ruleset {
            knights_move_diagonally
        } else {
            false
        };
        let variant = self.variant();
        match self.cells()[cell.to_index()] {
            Some(piece) if piece.kind == CrownfallPieceKind::Knight => {
                if knights_move_diagonally_enabled {
                    tables::knight_diagonal_moves(variant, piece.player, cell.to_index())
                } else {
                    tables::knight_moves(variant, piece.player, cell.to_index())
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
        let knights_move_diagonally_enabled = if let CrownfallRuleset::Custom { knights_move_diagonally,..} = rules.ruleset {
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

    fn piece_count(&self, player: CrownfallPlayerKind, kind: CrownfallPieceKind) -> usize {
        self.cells()
            .iter()
            .flatten()
            .filter(|piece| piece.player == player && piece.kind == kind)
            .count()
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
            if matches!(self.cells()[neighbour as usize], Some(piece) if piece.player == attacker && piece.kind != CrownfallPieceKind::Knight)
            {
                attackers[len] = neighbour;
                len += 1;
            }
        }
        for &neighbour in self.knight_capture_shape(attacker.opposite(), target.to_index(), rules) {
            if matches!(self.cells()[neighbour as usize], Some(piece) if piece.player == attacker && piece.kind == CrownfallPieceKind::Knight)
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
        let a = self.cells()[attackers.0.to_index()]?.kind;
        let b = self.cells()[attackers.1.to_index()]?.kind;
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
            if matches!(self.cells()[neighbour as usize], Some(piece) if piece.player == attacker) {
                attackers[len] = neighbour;
                len += 1;
            }
        }

        if self.is_capture_landing_spot_of(target, moved, attacker, rules)
            && matches!(self.cells()[moved.to_index()], Some(piece) if piece.player == attacker && piece.kind == CrownfallPieceKind::Knight)
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
            Some(piece) if piece.player == mover && piece.kind != CrownfallPieceKind::Crown => {
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
            Some(piece) if piece.player == mover && piece.kind == CrownfallPieceKind::Crown => self
                .find_crown_attacking_pair(at, mover.opposite(), at, rules)
                .is_some(),
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
        if matches!(self.cells()[to.to_index()], Some(piece) if piece.kind == CrownfallPieceKind::Knight)
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
            Some(piece) if piece.kind == CrownfallPieceKind::Knight => {
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
            if piece.player == attacker || piece.kind != CrownfallPieceKind::Crown {
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
            if piece.player == attacker || piece.kind == CrownfallPieceKind::Crown {
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
            if piece.player == attacker || piece.kind == CrownfallPieceKind::Crown {
                continue;
            }
            let ally = tables::ortho(variant, index as usize).iter().find(|&&n| {
                matches!(self.cells()[n as usize], Some(p) if p.player == attacker && p.kind != CrownfallPieceKind::Archer)
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

    /// A player is only out of the fight once both their Knights and Spies are
    /// depleted - Spy Capture works independently of Knights, so holding
    /// spies alone is still a real offensive threat (README "Losing the
    /// Game" - Attrition). Archers don't factor into attrition.
    fn is_attrition_defeated(&self, player: CrownfallPlayerKind) -> bool {
        self.piece_count(player, CrownfallPieceKind::Knight) <= 1
            && self.piece_count(player, CrownfallPieceKind::Spy) <= 1
    }

    /// True when a Knight Capture has left one player with a single knight and
    /// the other with none - the exchange that caused it hit both sides at once,
    /// so neither is credited with an attrition win; the game is a draw instead.
    fn is_mutual_knight_exhaustion(&self) -> bool {
        let white = self.piece_count(CrownfallPlayerKind::White, CrownfallPieceKind::Knight);
        let black = self.piece_count(CrownfallPlayerKind::Black, CrownfallPieceKind::Knight);
        matches!((white, black), (0, 1) | (1, 0))
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
        for (index, cell) in self.cells().iter().enumerate() {
            let Some(piece) = cell else { continue };
            if piece.player != player {
                continue;
            }
            let from = CrownfallBoardCell::new_index(index);
            for &dest in self.move_candidates(from, rules) {
                if self.cells()[dest as usize].is_some() {
                    continue;
                }
                let to = CrownfallBoardCell::new_index(dest as usize);
                let mut scratch = *self;
                scratch.cells_mut()[index] = None;
                scratch.cells_mut()[dest as usize] = Some(*piece);
                if scratch.move_captures_something(to, player, piece.kind, rules) {
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
    /// Turns remaining before the `TOTAL_TURN_LIMIT` safety-net draw fires,
    /// regardless of repetition or recent captures.
    pub fn turns_remaining(&self) -> u16 {
        let turns_played = (self.history.len() - 1) as u16;
        TOTAL_TURN_LIMIT.saturating_sub(turns_played)
    }

    /// Turns remaining before the no-progress draw fires if no capture
    /// happens in the meantime (chess's 50-move rule, adapted).
    pub fn turns_remaining_before_no_progress_draw(&self) -> u16 {
        NO_PROGRESS_LIMIT.saturating_sub(self.moves_since_capture)
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
        self.apply_action_with_logging(action, true)
    }

    /// Applies `action` without logging the move/capture, used by the AI's search
    /// (`ai::best_move`/`negamax`) to explore candidate positions - those simulated
    /// moves aren't real turns and would otherwise drown out actual gameplay in the
    /// log (see `game::ai`, which calls this instead of `apply_action`).
    pub(crate) fn apply_action_quiet(
        &mut self,
        action: CrownfallPlayerAction,
    ) -> Result<Option<CrownfallTurnResult>, CrownfallError> {
        self.apply_action_with_logging(action, false)
    }

    fn apply_action_with_logging(
        &mut self,
        action: CrownfallPlayerAction,
        log_moves: bool,
    ) -> Result<Option<CrownfallTurnResult>, CrownfallError> {
        match &self.state {
            CrownfallGameState::Playing(play_state) => {
                if play_state.player() != action.player() {
                    return Err(CrownfallError::NotYourTurn(action.player()));
                }
            }
            CrownfallGameState::Victory(_) => {
                return Err(CrownfallError::GameOver(action.player()));
            }
            CrownfallGameState::Draw(_) => return Err(CrownfallError::GameOver(action.player())),
        }
        match action {
            CrownfallPlayerAction::Move { player, from, to } => {
                self.apply_move(player, from, to, log_moves)
            }
            CrownfallPlayerAction::KnightRemoval { player, at } => {
                self.apply_knight_removal(player, at)
            }
            CrownfallPlayerAction::Surrender { player } => {
                self.state = CrownfallGameState::Victory(player.opposite());
                Ok(None)
            }
        }
    }

    #[cfg_attr(not(feature = "log"), allow(unused_variables))]
    fn apply_move(
        &mut self,
        player: CrownfallPlayerKind,
        from: CrownfallBoardCell,
        to: CrownfallBoardCell,
        log_moves: bool,
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
        if piece.player != player {
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

        let must_capture_rule_enabled = if let CrownfallRuleset::Custom { mandatory_capture,..} = self.rules.ruleset {
            mandatory_capture
        } else {
            false
        };
        if must_capture_rule_enabled {
            let mut scratch = self.board;
            scratch.cells_mut()[from_index] = None;
            scratch.cells_mut()[to_index] = Some(piece);
            let this_move_captures =
                scratch.move_captures_something(to, player, piece.kind, self.rules);
            if !this_move_captures && self.board.has_available_capture(player, self.rules) {
                return Err(CrownfallError::CaptureRequired(player));
            }
        }

        #[cfg(feature = "log")]
        if log_moves {
            log::info!("{player:?} moves {:?} from {from:?} to {to:?}", piece.kind);
        }

        self.board.cells_mut()[from_index] = None;
        self.board.cells_mut()[to_index] = Some(piece);

        // Crown-loss has the highest priority of any capture and is checked first,
        // even ahead of a capture this same move would otherwise complete (README
        // "Crown" section - the crown moving into a trap loses the game outright).
        // This holds regardless of `all_captures_processed`: crown loss always ends
        // the game immediately, so there's nothing left to "also process".
        if self.board.check_own_crown_trap(to, player, self.rules) {
            self.board.cells_mut()[to_index] = None;
            #[cfg(feature = "log")]
            if log_moves {
                log::info!("captured: {player:?} Crown at {to:?}");
            }
            self.state = CrownfallGameState::Victory(player.opposite());
            return Ok(Some(CrownfallTurnResult::Victory {
                player: player.opposite(),
                surrounded_crown: to,
            }));
        }

        let all_captures_processed_enabled = if let CrownfallRuleset::Custom { all_captures_processed,..} = self.rules.ruleset {
            all_captures_processed
        } else {
            false
        };
        if all_captures_processed_enabled {
            self.apply_move_all_captures_processed(player, from, to, piece, log_moves)
        } else {
            self.apply_move_sequential(player, from, to, piece, log_moves)
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
    ) -> Result<Option<CrownfallTurnResult>, CrownfallError> {
        let to_index = to.to_index();

        if let Some(surrounded_crown) = self.board.check_crown_capture(to, player, self.rules) {
            self.board.cells_mut()[surrounded_crown.to_index()] = None;
            #[cfg(feature = "log")]
            if log_moves {
                log::info!(
                    "captured: {:?} Crown at {surrounded_crown:?}",
                    player.opposite()
                );
            }
            self.state = CrownfallGameState::Victory(player);
            return Ok(Some(CrownfallTurnResult::Victory {
                player,
                surrounded_crown,
            }));
        }

        // Spy Capture applies "even if the enemy moved there" - the piece just moved
        // can walk straight into a pre-existing enemy Spy pincer and be captured by it.
        if self.board.check_self_spy_trap(to, player, self.rules) {
            self.board.cells_mut()[to_index] = None;
            #[cfg(feature = "log")]
            if log_moves {
                log::info!("captured: {player:?} {:?} at {to:?}", piece.kind);
            }
            let attackers = self
                .board
                .find_attacking_pair(to, player.opposite(), self.rules)
                .expect("check_self_spy_trap confirmed an attacking pair");
            self.state = self.resolve_after_removal(player, true);
            return Ok(Some(CrownfallTurnResult::Capture {
                player,
                last_move_from: from,
                last_move_to: to,
                removed: to,
                second_attacker: attackers.0.1,
            }));
        }

        let (captures, capture_count) = self.board.check_piece_captures(to, player, self.rules);
        let (archer_captures, archer_count) = if piece.kind == CrownfallPieceKind::Archer {
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
            );
            self.state = self.resolve_after_removal(player, true);
            return Ok(Some(turn_result));
        }

        if archer_count > 0 {
            let turn_result = self.apply_archer_captures(
                &archer_captures[..archer_count],
                player,
                from,
                to,
                log_moves,
            );
            self.state = self.resolve_after_removal(player, true);
            return Ok(Some(turn_result));
        }

        self.state = resolve_continuation(
            &self.board,
            player.opposite(),
            &mut self.history,
            &mut self.moves_since_capture,
            false,
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
    ) -> Result<Option<CrownfallTurnResult>, CrownfallError> {
        let to_index = to.to_index();
        let snapshot = self.board;

        let crown_capture = snapshot.check_crown_capture(to, player, self.rules);
        let self_trapped = snapshot.check_self_spy_trap(to, player, self.rules);
        let (captures, capture_count) = snapshot.check_piece_captures(to, player, self.rules);
        let (archer_captures, archer_count) = if piece.kind == CrownfallPieceKind::Archer {
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
            self.board.cells_mut()[surrounded_crown.to_index()] = None;
            #[cfg(feature = "log")]
            if log_moves {
                log::info!(
                    "captured: {:?} Crown at {surrounded_crown:?}",
                    player.opposite()
                );
            }
            any_capture = true;
            turn_result.get_or_insert(CrownfallTurnResult::Victory {
                player,
                surrounded_crown,
            });
        }

        if self_trapped {
            self.board.cells_mut()[to_index] = None;
            #[cfg(feature = "log")]
            if log_moves {
                log::info!("captured: {player:?} {:?} at {to:?}", piece.kind);
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
            );
            any_capture = true;
            turn_result.get_or_insert(result);
        }

        if crown_capture.is_some() {
            // The Crown was captured outright this move - that always ends
            // the game, regardless of what else also happened.
            self.state = CrownfallGameState::Victory(player);
            return Ok(turn_result);
        }

        if any_capture {
            self.state = self.resolve_after_removal(player, true);
            return Ok(turn_result);
        }

        self.state = resolve_continuation(
            &self.board,
            player.opposite(),
            &mut self.history,
            &mut self.moves_since_capture,
            false,
        );
        Ok(Some(CrownfallTurnResult::PieceMove { player, from, to }))
    }

    /// Removes every target in `captures`, sacrificing the attacking Knight
    /// where the rule requires it (README "Knight Capture"), and returns
    /// the `TurnResult` for the first capture found (matching the existing
    /// single-result-per-move reporting shape).
    #[cfg_attr(not(feature = "log"), allow(unused_variables))]
    fn apply_piece_captures(
        &mut self,
        captures: &[PieceCapture],
        player: CrownfallPlayerKind,
        from: CrownfallBoardCell,
        to: CrownfallBoardCell,
        piece: CrownfallPiece,
        log_moves: bool,
    ) -> CrownfallTurnResult {
        let mut turn_result = None;
        for capture in captures {
            let target_kind =
                self.board.cells()[capture.target.to_index()].map(|target_piece| target_piece.kind);
            self.board.cells_mut()[capture.target.to_index()] = None;
            #[cfg(feature = "log")]
            if log_moves {
                log::info!(
                    "captured: {:?} {:?} at {:?}",
                    player.opposite(),
                    target_kind.expect("target held a piece before removal"),
                    capture.target
                );
            }
            let second_attacker = CrownfallBoardState::other_attacker(capture.attackers, to);
            // The attacking player only loses one of their own knights when the
            // *captured piece itself* was a Knight (README "Knight Capture") - a
            // Knight+Knight/Knight+Crown pincer capturing a Spy carries no penalty.
            if capture.kind == CaptureKind::Knight
                && target_kind == Some(CrownfallPieceKind::Knight)
            {
                let lost_knight = if piece.kind == CrownfallPieceKind::Crown {
                    second_attacker
                } else {
                    to
                };
                self.board.cells_mut()[lost_knight.to_index()] = None;
                #[cfg(feature = "log")]
                if log_moves {
                    log::info!("captured: {player:?} Knight at {lost_knight:?}");
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
    ) -> CrownfallTurnResult {
        let mut turn_result = None;
        for &(target, ally) in captures {
            let target_kind = self.board.cells()[target.to_index()].map(|p| p.kind);
            self.board.cells_mut()[target.to_index()] = None;
            #[cfg(feature = "log")]
            if log_moves {
                log::info!(
                    "captured: {:?} {:?} at {target:?} (archer shot)",
                    player.opposite(),
                    target_kind.expect("target held a piece before removal")
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
    /// removed this move: mutual knight exhaustion and attrition still take
    /// priority over an ordinary continuation, exactly as under the
    /// sequential rules - this variant only changes *which* pieces get
    /// removed, not the priority of the resulting `GameState`.
    fn resolve_after_removal(
        &mut self,
        player: CrownfallPlayerKind,
        captured: bool,
    ) -> CrownfallGameState {
        if self.board.is_mutual_knight_exhaustion() {
            CrownfallGameState::Draw(DrawReason::MutualKnightExhaustion)
        } else if self.board.is_attrition_defeated(player.opposite()) {
            CrownfallGameState::Victory(player)
        } else {
            resolve_continuation(
                &self.board,
                player.opposite(),
                &mut self.history,
                &mut self.moves_since_capture,
                captured,
            )
        }
    }

    fn apply_knight_removal(
        &mut self,
        player: CrownfallPlayerKind,
        at: CrownfallBoardCell,
    ) -> Result<Option<CrownfallTurnResult>, CrownfallError> {
        // Same deserialized-index guard as `apply_move`.
        if at.to_index() >= tables::cell_count(self.rules.board) {
            return Err(CrownfallError::EmptyKnightRemoval(player, at));
        }
        match self.board.cells()[at.to_index()] {
            None => Err(CrownfallError::EmptyKnightRemoval(player, at)),
            Some(cell) => {
                if cell.player == player {
                    self.board.cells_mut()[at.index] = None;
                    self.state = resolve_continuation(
                        &self.board,
                        player.opposite(),
                        &mut self.history,
                        &mut self.moves_since_capture,
                        true,
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
