use crate::errors::CrownfallError;
use crate::hash::position_hash;
use crate::tables::{self, CELL_COUNT};
use crate::*;
use alloc::vec;
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CaptureKind {
    Spy,
    Knight,
}

/// One capture detected by `check_piece_captures` — kept `Copy` so a move's
/// worth of captures fits in a stack array (see `PieceCaptures`).
#[derive(Clone, Copy)]
struct PieceCapture {
    target: CrownfallBoardCell,
    kind: CaptureKind,
    attackers: (CrownfallBoardCell, CrownfallBoardCell),
}

/// The capture scan looks at a mover's 4 orthogonal neighbours plus, for a
/// Knight, its 2 forward-diagonal arc cells (the straight-forward one is
/// already orthogonal) — so a single move can never threaten more than 6
/// cells, and each threatened cell yields at most one capture.
const MAX_SCAN_CELLS: usize = 6;

type ScanCells = ([u8; MAX_SCAN_CELLS], usize);
type PieceCaptures = ([PieceCapture; MAX_SCAN_CELLS], usize);

/// Number of times a position (board + player to move) must occur for the
/// game to be declared a draw (matches chess's threefold repetition rule).
const REPETITION_LIMIT: usize = 3;

/// Turns without a capture before the no-progress draw rule fires (chess's
/// 50-move rule, adapted — Crownfall has no pawn-equivalent, so "no capture"
/// is the sole progress signal).
const NO_PROGRESS_LIMIT: u16 = 40;

/// Absolute turn-count safety net: the game is drawn if it's still going
/// after this many turns, regardless of repetition or progress.
const TOTAL_TURN_LIMIT: u16 = 200;

/// Records the position that's about to be played from and returns the
/// resulting `GameState` — `Draw` if this exact position has now recurred
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

/// Shorthand for building `BoardState::default`'s initial layout.
fn p(kind: CrownfallPieceKind, player: CrownfallPlayerKind) -> Option<CrownfallPiece> {
    Some(CrownfallPiece { kind, player })
}

impl Default for CrownfallBoardState {
    /// Each side's 6 Knights are staggered across two rows (4 on the row
    /// nearest their own Crown, 2 one row further forward, offset into the
    /// gaps) rather than one solid contiguous line. A solid 6-wide line
    /// meeting an identical enemy line head-on puts every Knight in a
    /// mutual forward-arc pincer simultaneously, cascading into a
    /// same-turn massacre that trades away most of both sides' Knights
    /// before the game has really started. Staggering means only the 2
    /// advanced Knights per side can meet that early, keeping the rest in
    /// play for later.
    fn default() -> CrownfallBoardState {
        use CrownfallPieceKind::{Crown, Knight, Spy};
        use CrownfallPlayerKind::{Black, White};
        CrownfallBoardState {
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
}

impl Default for CrownfallGame {
    fn default() -> Self {
        let board = CrownfallBoardState::default();
        let history = vec![position_hash(&board, CrownfallPlayerKind::White)];
        Self {
            board,
            state: CrownfallGameState::Playing(CrownfallPlayState::WaitingForInput {
                player: CrownfallPlayerKind::White,
            }),
            history,
            moves_since_capture: 0,
        }
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
    /// Move-candidate cells for the piece at `cell`, ignoring occupancy:
    /// Knights get their orthogonal-minus-backward table, everything else the
    /// plain orthogonal one (see `tables` — Knights move orthogonally like
    /// every other piece but may never move backward; their old
    /// diagonal-forward reach is now the shape of their *capture* threat
    /// instead, `tables::KNIGHT_ARCS`). A straight ROM lookup: no coordinate
    /// math, no bounds branches, no allocation.
    pub(crate) fn move_candidates(&self, cell: CrownfallBoardCell) -> &'static [u8] {
        match self.cells[cell.to_index()] {
            Some(piece) if piece.kind == CrownfallPieceKind::Knight => {
                tables::KNIGHT_MOVES[piece.player as usize][cell.to_index()].as_slice()
            }
            Some(_) => tables::ORTHO[cell.to_index()].as_slice(),
            None => &[],
        }
    }

    /// Legal move destinations for the piece at `cell`. Allocates the result
    /// for UI callers; the AI and move validation use `move_candidates`
    /// directly and never build this `Vec`.
    pub fn get_valid_destinations_for(&self, cell: CrownfallBoardCell) -> Vec<CrownfallBoardCell> {
        self.move_candidates(cell)
            .iter()
            .filter(|&&index| self.cells[index as usize].is_none())
            .map(|&index| CrownfallBoardCell::new_index(index as usize))
            .collect()
    }

    /// True if `attacker_cell` is diagonally (not straight) ahead of `target`
    /// from `attacker`'s forward direction — i.e. one of the two diagonal
    /// cells of `tables::KNIGHT_ARCS[attacker.opposite()][target]` (the arc
    /// as seen from the target's square), excluding the straight-ahead one.
    /// A Knight that just moved must land here (not merely directly ahead)
    /// to be the piece completing a Knight Capture pincer — see
    /// `check_piece_captures`/`check_crown_capture`.
    fn is_diagonally_forward_of(target: CrownfallBoardCell, attacker_cell: CrownfallBoardCell, attacker: CrownfallPlayerKind) -> bool {
        tables::COORD[attacker_cell.to_index()].0 != tables::COORD[target.to_index()].0
            && tables::KNIGHT_ARCS[attacker.opposite() as usize][target.to_index()]
                .as_slice()
                .contains(&(attacker_cell.to_index() as u8))
    }

    fn piece_count(&self, player: CrownfallPlayerKind, kind: CrownfallPieceKind) -> usize {
        self.cells
            .iter()
            .flatten()
            .filter(|piece| piece.player == player && piece.kind == kind)
            .count()
    }

    /// First pair among `attackers` whose piece kinds form a valid capture,
    /// in the order the attackers were gathered.
    fn first_capturing_pair(&self, attackers: &[u8]) -> Option<((CrownfallBoardCell, CrownfallBoardCell), CaptureKind)> {
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
    /// own forward arc — a Knight standing beside or behind `target` cannot form a
    /// Knight Capture pincer, only one standing ahead of or diagonally ahead of it can
    /// (see `tables::KNIGHT_ARCS`). Whether the just-moved piece specifically
    /// must land diagonally (not just anywhere in the arc) is enforced by callers via
    /// `is_diagonally_forward_of`, not here — this only finds *some* valid pair.
    /// Extra attacker-owned pieces also adjacent to
    /// `target` (of any kind) must not block a genuine pincer formed by two others.
    fn find_attacking_pair(
        &self,
        target: CrownfallBoardCell,
        attacker: CrownfallPlayerKind,
    ) -> Option<((CrownfallBoardCell, CrownfallBoardCell), CaptureKind)> {
        // At most 4 orthogonal non-Knight attackers + 3 arc Knights.
        let mut attackers = [0u8; 7];
        let mut len = 0;
        for &neighbour in tables::ORTHO[target.to_index()].as_slice() {
            if matches!(self.cells[neighbour as usize], Some(piece) if piece.player == attacker && piece.kind != CrownfallPieceKind::Knight)
            {
                attackers[len] = neighbour;
                len += 1;
            }
        }
        for &neighbour in
            tables::KNIGHT_ARCS[attacker.opposite() as usize][target.to_index()].as_slice()
        {
            if matches!(self.cells[neighbour as usize], Some(piece) if piece.player == attacker && piece.kind == CrownfallPieceKind::Knight)
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
    /// so a Crown+Spy pair does not form a valid capture.
    fn capture_kind(&self, attackers: (CrownfallBoardCell, CrownfallBoardCell)) -> Option<CaptureKind> {
        let a = self.cells[attackers.0.to_index()]?.kind;
        let b = self.cells[attackers.1.to_index()]?.kind;
        match (a, b) {
            (CrownfallPieceKind::Spy, CrownfallPieceKind::Spy) => Some(CaptureKind::Spy),
            (CrownfallPieceKind::Knight, CrownfallPieceKind::Knight) => Some(CaptureKind::Knight),
            (CrownfallPieceKind::Crown, CrownfallPieceKind::Knight) | (CrownfallPieceKind::Knight, CrownfallPieceKind::Crown) => {
                Some(CaptureKind::Knight)
            }
            _ => None,
        }
    }

    /// Finds a valid capturing pincer against the Crown at `target`, occupied by
    /// `attacker`-owned pieces. Any of the Crown's orthogonally adjacent tiles counts
    /// unconditionally (any side, whether that piece just moved or was already in
    /// place) — Crown captures are not bound by the Knight forward-arc restriction the
    /// way ordinary Knight Captures are. However, a Knight can *also* attack from one
    /// of its two forward-diagonal cells (outside plain orthogonal adjacency) if —
    /// and only if — `moved` is that Knight: the diagonal reach only activates for the
    /// Knight that's actively moving into it this turn, never for one that was
    /// already sitting there (see README "Captures" — "invalid" example).
    fn find_crown_attacking_pair(
        &self,
        target: CrownfallBoardCell,
        attacker: CrownfallPlayerKind,
        moved: CrownfallBoardCell,
    ) -> Option<((CrownfallBoardCell, CrownfallBoardCell), CaptureKind)> {
        // At most 4 orthogonal attackers + the just-moved diagonal Knight.
        let mut attackers = [0u8; 5];
        let mut len = 0;
        for &neighbour in tables::ORTHO[target.to_index()].as_slice() {
            if matches!(self.cells[neighbour as usize], Some(piece) if piece.player == attacker) {
                attackers[len] = neighbour;
                len += 1;
            }
        }

        if Self::is_diagonally_forward_of(target, moved, attacker)
            && matches!(self.cells[moved.to_index()], Some(piece) if piece.player == attacker && piece.kind == CrownfallPieceKind::Knight)
        {
            attackers[len] = moved.to_index() as u8;
            len += 1;
        }

        self.first_capturing_pair(&attackers[..len])
    }

    /// The attacker's own piece other than `moved` in an attacking pair.
    fn other_attacker(attackers: (CrownfallBoardCell, CrownfallBoardCell), moved: CrownfallBoardCell) -> CrownfallBoardCell {
        if attackers.0 == moved {
            attackers.1
        } else {
            attackers.0
        }
    }

    /// True if the piece just moved to `at` (owned by `mover`) is captured by a
    /// pre-existing enemy Spy pair — the Spy Capture rule applies "even if the enemy
    /// moved there" (README "Spy Capture"). The Crown is exempt: its own capture is
    /// governed exclusively by the higher-priority crown-loss check.
    fn check_self_spy_trap(&self, at: CrownfallBoardCell, mover: CrownfallPlayerKind) -> bool {
        match self.cells[at.to_index()] {
            Some(piece) if piece.player == mover && piece.kind != CrownfallPieceKind::Crown => {
                self.find_attacking_pair(at, mover.opposite())
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
    fn check_own_crown_trap(&self, at: CrownfallBoardCell, mover: CrownfallPlayerKind) -> bool {
        match self.cells[at.to_index()] {
            Some(piece) if piece.player == mover && piece.kind == CrownfallPieceKind::Crown => self
                .find_crown_attacking_pair(at, mover.opposite(), at)
                .is_some(),
            _ => false,
        }
    }

    /// Cells a just-moved piece at `to` might now be threatening as an attacker: its
    /// plain orthogonal neighbours, plus — if it's a Knight — its forward arc, since a
    /// Knight's capture reach extends diagonally ahead of it (see
    /// `tables::KNIGHT_ARCS`).
    fn capture_scan_cells(&self, to: CrownfallBoardCell, mover: CrownfallPlayerKind) -> ScanCells {
        let mut cells = [0u8; MAX_SCAN_CELLS];
        let mut len = 0;
        for &neighbour in tables::ORTHO[to.to_index()].as_slice() {
            cells[len] = neighbour;
            len += 1;
        }
        if matches!(self.cells[to.to_index()], Some(piece) if piece.kind == CrownfallPieceKind::Knight) {
            for &cell in tables::KNIGHT_ARCS[mover as usize][to.to_index()].as_slice() {
                if !cells[..len].contains(&cell) {
                    cells[len] = cell;
                    len += 1;
                }
            }
        }
        (cells, len)
    }

    /// True unless `mover_piece` is a Knight that just moved to `to` and would be
    /// completing the pincer against `target` by landing directly (not diagonally)
    /// ahead of it. A Knight can only be the piece that *springs* a Knight Capture
    /// pincer if it lands diagonally ahead of the target — a partner Knight already
    /// in place may sit directly ahead, but the just-moved piece may not (see
    /// `is_diagonally_forward_of`). Non-Knight movers (Crown, Spy) are unrestricted.
    fn moved_knight_completes_pincer(&self, to: CrownfallBoardCell, target: CrownfallBoardCell, attacker: CrownfallPlayerKind) -> bool {
        match self.cells[to.to_index()] {
            Some(piece) if piece.kind == CrownfallPieceKind::Knight => {
                Self::is_diagonally_forward_of(target, to, attacker)
            }
            _ => true,
        }
    }

    fn check_crown_capture(&self, to: CrownfallBoardCell, attacker: CrownfallPlayerKind) -> Option<CrownfallBoardCell> {
        let (cells, len) = self.capture_scan_cells(to, attacker);
        for &index in &cells[..len] {
            let Some(piece) = self.cells[index as usize] else {
                continue;
            };
            if piece.player == attacker || piece.kind != CrownfallPieceKind::Crown {
                continue;
            }
            let neighbour = CrownfallBoardCell::new_index(index as usize);
            if self
                .find_crown_attacking_pair(neighbour, attacker, to)
                .is_some()
            {
                return Some(neighbour);
            }
        }
        None
    }

    fn check_piece_captures(&self, to: CrownfallBoardCell, attacker: CrownfallPlayerKind) -> PieceCaptures {
        let placeholder = PieceCapture {
            target: CrownfallBoardCell { index: 0 },
            kind: CaptureKind::Spy,
            attackers: (CrownfallBoardCell { index: 0 }, CrownfallBoardCell { index: 0 }),
        };
        let mut captures = [placeholder; MAX_SCAN_CELLS];
        let mut count = 0;
        let (cells, len) = self.capture_scan_cells(to, attacker);
        for &index in &cells[..len] {
            let Some(piece) = self.cells[index as usize] else {
                continue;
            };
            if piece.player == attacker || piece.kind == CrownfallPieceKind::Crown {
                continue;
            }
            let target = CrownfallBoardCell::new_index(index as usize);
            // Cheap arc check first — the pair search is the expensive part.
            if !self.moved_knight_completes_pincer(to, target, attacker) {
                continue;
            }
            let Some((attackers, kind)) = self.find_attacking_pair(target, attacker) else {
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

    /// A player is only out of the fight once both their Knights and Spies are
    /// depleted — Spy Capture works independently of Knights, so holding
    /// spies alone is still a real offensive threat (README "Losing the
    /// Game" — Attrition).
    fn is_attrition_defeated(&self, player: CrownfallPlayerKind) -> bool {
        self.piece_count(player, CrownfallPieceKind::Knight) <= 1
            && self.piece_count(player, CrownfallPieceKind::Spy) <= 1
    }

    /// True when a Knight Capture has left one player with a single knight and
    /// the other with none — the exchange that caused it hit both sides at once,
    /// so neither is credited with an attrition win; the game is a draw instead.
    fn is_mutual_knight_exhaustion(&self) -> bool {
        let white = self.piece_count(CrownfallPlayerKind::White, CrownfallPieceKind::Knight);
        let black = self.piece_count(CrownfallPlayerKind::Black, CrownfallPieceKind::Knight);
        matches!((white, black), (0, 1) | (1, 0))
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
    /// position `history`. On `Err` the game is guaranteed untouched —
    /// every validation runs before the first mutation.
    pub fn apply_action(&mut self, action: CrownfallPlayerAction) -> Result<Option<CrownfallTurnResult>, CrownfallError> {
        self.apply_action_with_logging(action, true)
    }

    /// Applies `action` without logging the move/capture, used by the AI's search
    /// (`ai::best_move`/`negamax`) to explore candidate positions — those simulated
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
            CrownfallGameState::Victory(_) => return Err(CrownfallError::GameOver(action.player())),
            CrownfallGameState::Draw(_) => return Err(CrownfallError::GameOver(action.player())),
        }
        match action {
            CrownfallPlayerAction::Move { player, from, to } => self.apply_move(player, from, to, log_moves),
            CrownfallPlayerAction::KnightRemoval { player, at } => self.apply_knight_removal(player, at),
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
        // `Cell` is just a deserialized index — reject out-of-range ones here
        // rather than panicking on a board access (and `to` must be checked
        // before it's truncated to `u8` for the candidate-table comparison).
        if from_index >= CELL_COUNT {
            return Err(CrownfallError::EmptyMove(player, from));
        }
        if to_index >= CELL_COUNT {
            return Err(CrownfallError::InvalidDestination(player, from, to));
        }
        let piece = self.board.cells[from_index].ok_or(CrownfallError::EmptyMove(player, from))?;
        if piece.player != player {
            return Err(CrownfallError::EnemyMove(player, from));
        }
        if self.board.cells[to_index].is_some()
            || !self.board.move_candidates(from).contains(&(to_index as u8))
        {
            return Err(CrownfallError::InvalidDestination(player, from, to));
        }

        #[cfg(feature = "log")]
        if log_moves {
            log::info!("{player:?} moves {:?} from {from:?} to {to:?}", piece.kind);
        }

        self.board.cells[from_index] = None;
        self.board.cells[to_index] = Some(piece);

        // Crown-loss has the highest priority of any capture and is checked first,
        // even ahead of a capture this same move would otherwise complete (README
        // "Crown" section — the crown moving into a trap loses the game outright).
        if self.board.check_own_crown_trap(to, player) {
            self.board.cells[to_index] = None;
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

        if let Some(surrounded_crown) = self.board.check_crown_capture(to, player) {
            self.board.cells[surrounded_crown.to_index()] = None;
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

        // Spy Capture applies "even if the enemy moved there" — the piece just moved
        // can walk straight into a pre-existing enemy Spy pincer and be captured by it.
        if self.board.check_self_spy_trap(to, player) {
            self.board.cells[to_index] = None;
            #[cfg(feature = "log")]
            if log_moves {
                log::info!("captured: {player:?} {:?} at {to:?}", piece.kind);
            }
            let attackers = self
                .board
                .find_attacking_pair(to, player.opposite())
                .expect("check_self_spy_trap confirmed an attacking pair");
            self.state = if self.board.is_attrition_defeated(player) {
                CrownfallGameState::Victory(player.opposite())
            } else {
                resolve_continuation(
                    &self.board,
                    player.opposite(),
                    &mut self.history,
                    &mut self.moves_since_capture,
                    true,
                )
            };
            return Ok(Some(CrownfallTurnResult::Capture {
                player,
                last_move_from: from,
                last_move_to: to,
                removed: to,
                second_attacker: attackers.0.1,
            }));
        }

        let (captures, capture_count) = self.board.check_piece_captures(to, player);
        if capture_count > 0 {
            let mut turn_result = None;
            for capture in &captures[..capture_count] {
                let target_kind = self.board.cells[capture.target.to_index()]
                    .map(|target_piece| target_piece.kind);
                self.board.cells[capture.target.to_index()] = None;
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
                // *captured piece itself* was a Knight (README "Knight Capture") — a
                // Knight+Knight/Knight+Crown pincer capturing a Spy carries no penalty.
                if capture.kind == CaptureKind::Knight && target_kind == Some(CrownfallPieceKind::Knight) {
                    let lost_knight = if piece.kind == CrownfallPieceKind::Crown {
                        second_attacker
                    } else {
                        to
                    };
                    self.board.cells[lost_knight.to_index()] = None;
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
            let turn_result = turn_result.expect("captures is non-empty");

            self.state = if self.board.is_mutual_knight_exhaustion() {
                CrownfallGameState::Draw(DrawReason::MutualKnightExhaustion)
            } else if self.board.is_attrition_defeated(player.opposite()) {
                CrownfallGameState::Victory(player)
            } else {
                resolve_continuation(
                    &self.board,
                    player.opposite(),
                    &mut self.history,
                    &mut self.moves_since_capture,
                    true,
                )
            };

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

    fn apply_knight_removal(
        &mut self,
        player: CrownfallPlayerKind,
        at: CrownfallBoardCell,
    ) -> Result<Option<CrownfallTurnResult>, CrownfallError> {
        // Same deserialized-index guard as `apply_move`.
        if at.to_index() >= CELL_COUNT {
            return Err(CrownfallError::EmptyKnightRemoval(player, at));
        }
        match self.board.cells[at.to_index()] {
            None => Err(CrownfallError::EmptyKnightRemoval(player, at)),
            Some(cell) => {
                if cell.player == player {
                    self.board.cells[at.index] = None;
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
        }
    }

    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            CrownfallPieceKind::Crown => "Crown",
            CrownfallPieceKind::Knight => "Knight",
            CrownfallPieceKind::Spy => "Spy",
        }
    }
}
