use crate::errors::GameError;
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
    target: Cell,
    kind: CaptureKind,
    attackers: (Cell, Cell),
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
    board: &BoardState,
    next_player: PlayerKind,
    history: &mut Vec<u64>,
    moves_since_capture: &mut u16,
    captured: bool,
) -> GameState {
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
        GameState::Draw(DrawReason::Repetition)
    } else if *moves_since_capture >= NO_PROGRESS_LIMIT {
        GameState::Draw(DrawReason::NoProgress)
    } else if turns_played >= TOTAL_TURN_LIMIT {
        GameState::Draw(DrawReason::TurnLimit)
    } else {
        GameState::Playing(PlayState::WaitingForInput {
            player: next_player,
        })
    }
}

/// Shorthand for building `BoardState::default`'s initial layout.
fn p(kind: PieceKind, player: PlayerKind) -> Option<Piece> {
    Some(Piece { kind, player })
}

impl Default for BoardState {
    /// Each side's 6 Knights are staggered across two rows (4 on the row
    /// nearest their own Crown, 2 one row further forward, offset into the
    /// gaps) rather than one solid contiguous line. A solid 6-wide line
    /// meeting an identical enemy line head-on puts every Knight in a
    /// mutual forward-arc pincer simultaneously, cascading into a
    /// same-turn massacre that trades away most of both sides' Knights
    /// before the game has really started. Staggering means only the 2
    /// advanced Knights per side can meet that early, keeping the rest in
    /// play for later.
    fn default() -> BoardState {
        use PieceKind::{Crown, Knight, Spy};
        use PlayerKind::{Black, White};
        BoardState {
            cells: [
                // Row A (y=0)
                None, None, p(Spy, Black), p(Crown, Black), p(Spy, Black), None, None,
                // Row B (y=1)
                p(Knight, Black), None, p(Knight, Black), p(Spy, Black), p(Knight, Black), None, p(Knight, Black),
                // Row C (y=2)
                None, p(Knight, Black), None, None, None, p(Knight, Black), None,
                // Row D (y=3)
                None, None, None, None, None, None, None,
                // Row E (y=4)
                None, p(Knight, White), None, None, None, p(Knight, White), None,
                // Row F (y=5)
                p(Knight, White), None, p(Knight, White), p(Spy, White), p(Knight, White), None, p(Knight, White),
                // Row G (y=6)
                None, None, p(Spy, White), p(Crown, White), p(Spy, White), None, None,
            ],
        }
    }
}

impl Default for Game {
    fn default() -> Self {
        let board = BoardState::default();
        let history = vec![position_hash(&board, PlayerKind::White)];
        Self {
            board,
            state: GameState::Playing(PlayState::WaitingForInput {
                player: PlayerKind::White,
            }),
            history,
            moves_since_capture: 0,
        }
    }
}

impl PlayerAction {
    pub const fn player(&self) -> PlayerKind {
        match self {
            PlayerAction::Move { player, .. } => *player,
            PlayerAction::KnightRemoval { player, .. } => *player,
            PlayerAction::Surrender { player } => *player,
        }
    }
}

impl PlayState {
    pub const fn player(&self) -> PlayerKind {
        match self {
            PlayState::WaitingForInput { player } => *player,
            PlayState::MustRemoveKnight { player, .. } => *player,
        }
    }
}

impl BoardState {
    /// Move-candidate cells for the piece at `cell`, ignoring occupancy:
    /// Knights get their orthogonal-minus-backward table, everything else the
    /// plain orthogonal one (see `tables` — Knights move orthogonally like
    /// every other piece but may never move backward; their old
    /// diagonal-forward reach is now the shape of their *capture* threat
    /// instead, `tables::KNIGHT_ARCS`). A straight ROM lookup: no coordinate
    /// math, no bounds branches, no allocation.
    pub(crate) fn move_candidates(&self, cell: Cell) -> &'static [u8] {
        match self.cells[cell.to_index()] {
            Some(piece) if piece.kind == PieceKind::Knight => {
                tables::KNIGHT_MOVES[piece.player as usize][cell.to_index()].as_slice()
            }
            Some(_) => tables::ORTHO[cell.to_index()].as_slice(),
            None => &[],
        }
    }

    /// Legal move destinations for the piece at `cell`. Allocates the result
    /// for UI callers; the AI and move validation use `move_candidates`
    /// directly and never build this `Vec`.
    pub fn get_valid_destinations_for(&self, cell: Cell) -> Vec<Cell> {
        self.move_candidates(cell)
            .iter()
            .filter(|&&index| self.cells[index as usize].is_none())
            .map(|&index| Cell::new_index(index as usize))
            .collect()
    }

    /// True if `attacker_cell` is diagonally (not straight) ahead of `target`
    /// from `attacker`'s forward direction — i.e. one of the two diagonal
    /// cells of `tables::KNIGHT_ARCS[attacker.opposite()][target]` (the arc
    /// as seen from the target's square), excluding the straight-ahead one.
    /// A Knight that just moved must land here (not merely directly ahead)
    /// to be the piece completing a Knight Capture pincer — see
    /// `check_piece_captures`/`check_crown_capture`.
    fn is_diagonally_forward_of(target: Cell, attacker_cell: Cell, attacker: PlayerKind) -> bool {
        tables::COORD[attacker_cell.to_index()].0 != tables::COORD[target.to_index()].0
            && tables::KNIGHT_ARCS[attacker.opposite() as usize][target.to_index()]
                .as_slice()
                .contains(&(attacker_cell.to_index() as u8))
    }

    fn piece_count(&self, player: PlayerKind, kind: PieceKind) -> usize {
        self.cells
            .iter()
            .flatten()
            .filter(|piece| piece.player == player && piece.kind == kind)
            .count()
    }

    /// First pair among `attackers` whose piece kinds form a valid capture,
    /// in the order the attackers were gathered.
    fn first_capturing_pair(&self, attackers: &[u8]) -> Option<((Cell, Cell), CaptureKind)> {
        for i in 0..attackers.len() {
            for j in (i + 1)..attackers.len() {
                let pair = (
                    Cell::new_index(attackers[i] as usize),
                    Cell::new_index(attackers[j] as usize),
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
        target: Cell,
        attacker: PlayerKind,
    ) -> Option<((Cell, Cell), CaptureKind)> {
        // At most 4 orthogonal non-Knight attackers + 3 arc Knights.
        let mut attackers = [0u8; 7];
        let mut len = 0;
        for &neighbour in tables::ORTHO[target.to_index()].as_slice() {
            if matches!(self.cells[neighbour as usize], Some(piece) if piece.player == attacker && piece.kind != PieceKind::Knight)
            {
                attackers[len] = neighbour;
                len += 1;
            }
        }
        for &neighbour in tables::KNIGHT_ARCS[attacker.opposite() as usize][target.to_index()]
            .as_slice()
        {
            if matches!(self.cells[neighbour as usize], Some(piece) if piece.player == attacker && piece.kind == PieceKind::Knight)
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
    fn capture_kind(&self, attackers: (Cell, Cell)) -> Option<CaptureKind> {
        let a = self.cells[attackers.0.to_index()]?.kind;
        let b = self.cells[attackers.1.to_index()]?.kind;
        match (a, b) {
            (PieceKind::Spy, PieceKind::Spy) => Some(CaptureKind::Spy),
            (PieceKind::Knight, PieceKind::Knight) => Some(CaptureKind::Knight),
            (PieceKind::Crown, PieceKind::Knight) | (PieceKind::Knight, PieceKind::Crown) => {
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
        target: Cell,
        attacker: PlayerKind,
        moved: Cell,
    ) -> Option<((Cell, Cell), CaptureKind)> {
        // At most 4 orthogonal attackers + the just-moved diagonal Knight.
        let mut attackers = [0u8; 5];
        let mut len = 0;
        for &neighbour in tables::ORTHO[target.to_index()].as_slice() {
            if matches!(self.cells[neighbour as usize], Some(piece) if piece.player == attacker)
            {
                attackers[len] = neighbour;
                len += 1;
            }
        }

        if Self::is_diagonally_forward_of(target, moved, attacker)
            && matches!(self.cells[moved.to_index()], Some(piece) if piece.player == attacker && piece.kind == PieceKind::Knight)
        {
            attackers[len] = moved.to_index() as u8;
            len += 1;
        }

        self.first_capturing_pair(&attackers[..len])
    }

    /// The attacker's own piece other than `moved` in an attacking pair.
    fn other_attacker(attackers: (Cell, Cell), moved: Cell) -> Cell {
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
    fn check_self_spy_trap(&self, at: Cell, mover: PlayerKind) -> bool {
        match self.cells[at.to_index()] {
            Some(piece) if piece.player == mover && piece.kind != PieceKind::Crown => {
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
    fn check_own_crown_trap(&self, at: Cell, mover: PlayerKind) -> bool {
        match self.cells[at.to_index()] {
            Some(piece) if piece.player == mover && piece.kind == PieceKind::Crown => {
                self.find_crown_attacking_pair(at, mover.opposite(), at)
                    .is_some()
            }
            _ => false,
        }
    }

    /// Cells a just-moved piece at `to` might now be threatening as an attacker: its
    /// plain orthogonal neighbours, plus — if it's a Knight — its forward arc, since a
    /// Knight's capture reach extends diagonally ahead of it (see
    /// `tables::KNIGHT_ARCS`).
    fn capture_scan_cells(&self, to: Cell, mover: PlayerKind) -> ScanCells {
        let mut cells = [0u8; MAX_SCAN_CELLS];
        let mut len = 0;
        for &neighbour in tables::ORTHO[to.to_index()].as_slice() {
            cells[len] = neighbour;
            len += 1;
        }
        if matches!(self.cells[to.to_index()], Some(piece) if piece.kind == PieceKind::Knight) {
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
    fn moved_knight_completes_pincer(&self, to: Cell, target: Cell, attacker: PlayerKind) -> bool {
        match self.cells[to.to_index()] {
            Some(piece) if piece.kind == PieceKind::Knight => {
                Self::is_diagonally_forward_of(target, to, attacker)
            }
            _ => true,
        }
    }

    fn check_crown_capture(&self, to: Cell, attacker: PlayerKind) -> Option<Cell> {
        let (cells, len) = self.capture_scan_cells(to, attacker);
        for &index in &cells[..len] {
            let Some(piece) = self.cells[index as usize] else {
                continue;
            };
            if piece.player == attacker || piece.kind != PieceKind::Crown {
                continue;
            }
            let neighbour = Cell::new_index(index as usize);
            if self
                .find_crown_attacking_pair(neighbour, attacker, to)
                .is_some()
            {
                return Some(neighbour);
            }
        }
        None
    }

    fn check_piece_captures(&self, to: Cell, attacker: PlayerKind) -> PieceCaptures {
        let placeholder = PieceCapture {
            target: Cell { index: 0 },
            kind: CaptureKind::Spy,
            attackers: (Cell { index: 0 }, Cell { index: 0 }),
        };
        let mut captures = [placeholder; MAX_SCAN_CELLS];
        let mut count = 0;
        let (cells, len) = self.capture_scan_cells(to, attacker);
        for &index in &cells[..len] {
            let Some(piece) = self.cells[index as usize] else {
                continue;
            };
            if piece.player == attacker || piece.kind == PieceKind::Crown {
                continue;
            }
            let target = Cell::new_index(index as usize);
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
    fn is_attrition_defeated(&self, player: PlayerKind) -> bool {
        self.piece_count(player, PieceKind::Knight) <= 1
            && self.piece_count(player, PieceKind::Spy) <= 1
    }

    /// True when a Knight Capture has left one player with a single knight and
    /// the other with none — the exchange that caused it hit both sides at once,
    /// so neither is credited with an attrition win; the game is a draw instead.
    fn is_mutual_knight_exhaustion(&self) -> bool {
        let white = self.piece_count(PlayerKind::White, PieceKind::Knight);
        let black = self.piece_count(PlayerKind::Black, PieceKind::Knight);
        matches!((white, black), (0, 1) | (1, 0))
    }
}

impl Game {
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
        action: PlayerAction,
    ) -> Result<(Game, Option<TurnResult>), GameError> {
        let result = self.apply_action(action)?;
        Ok((self, result))
    }

    /// In-place equivalent of `handle_player_action`: applies `action`
    /// directly to this game instead of consuming and returning it, so
    /// callers that would otherwise `clone()` first (every node of the AI
    /// search, most importantly) don't have to copy the ever-growing
    /// position `history`. On `Err` the game is guaranteed untouched —
    /// every validation runs before the first mutation.
    pub fn apply_action(&mut self, action: PlayerAction) -> Result<Option<TurnResult>, GameError> {
        self.apply_action_with_logging(action, true)
    }

    /// Applies `action` without logging the move/capture, used by the AI's search
    /// (`ai::best_move`/`negamax`) to explore candidate positions — those simulated
    /// moves aren't real turns and would otherwise drown out actual gameplay in the
    /// log (see `game::ai`, which calls this instead of `apply_action`).
    pub(crate) fn apply_action_quiet(
        &mut self,
        action: PlayerAction,
    ) -> Result<Option<TurnResult>, GameError> {
        self.apply_action_with_logging(action, false)
    }

    fn apply_action_with_logging(
        &mut self,
        action: PlayerAction,
        log_moves: bool,
    ) -> Result<Option<TurnResult>, GameError> {
        match &self.state {
            GameState::Playing(play_state) => {
                if play_state.player() != action.player() {
                    return Err(GameError::NotYourTurn(action.player()));
                }
            }
            GameState::Victory(_) => return Err(GameError::GameOver(action.player())),
            GameState::Draw(_) => return Err(GameError::GameOver(action.player())),
        }
        match action {
            PlayerAction::Move { player, from, to } => {
                self.apply_move(player, from, to, log_moves)
            }
            PlayerAction::KnightRemoval { player, at } => self.apply_knight_removal(player, at),
            PlayerAction::Surrender { player } => {
                self.state = GameState::Victory(player.opposite());
                Ok(None)
            }
        }
    }

    #[cfg_attr(not(feature = "log"), allow(unused_variables))]
    fn apply_move(
        &mut self,
        player: PlayerKind,
        from: Cell,
        to: Cell,
        log_moves: bool,
    ) -> Result<Option<TurnResult>, GameError> {
        let from_index = from.to_index();
        let to_index = to.to_index();
        // `Cell` is just a deserialized index — reject out-of-range ones here
        // rather than panicking on a board access (and `to` must be checked
        // before it's truncated to `u8` for the candidate-table comparison).
        if from_index >= CELL_COUNT {
            return Err(GameError::EmptyMove(player, from));
        }
        if to_index >= CELL_COUNT {
            return Err(GameError::InvalidDestination(player, from, to));
        }
        let piece = self.board.cells[from_index].ok_or(GameError::EmptyMove(player, from))?;
        if piece.player != player {
            return Err(GameError::EnemyMove(player, from));
        }
        if self.board.cells[to_index].is_some()
            || !self.board.move_candidates(from).contains(&(to_index as u8))
        {
            return Err(GameError::InvalidDestination(player, from, to));
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
            self.state = GameState::Victory(player.opposite());
            return Ok(Some(TurnResult::Victory {
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
            self.state = GameState::Victory(player);
            return Ok(Some(TurnResult::Victory {
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
                GameState::Victory(player.opposite())
            } else {
                resolve_continuation(
                    &self.board,
                    player.opposite(),
                    &mut self.history,
                    &mut self.moves_since_capture,
                    true,
                )
            };
            return Ok(Some(TurnResult::Capture {
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
                let second_attacker = BoardState::other_attacker(capture.attackers, to);
                // The attacking player only loses one of their own knights when the
                // *captured piece itself* was a Knight (README "Knight Capture") — a
                // Knight+Knight/Knight+Crown pincer capturing a Spy carries no penalty.
                if capture.kind == CaptureKind::Knight && target_kind == Some(PieceKind::Knight) {
                    let lost_knight = if piece.kind == PieceKind::Crown {
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
                let captured_this = TurnResult::Capture {
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
                GameState::Draw(DrawReason::MutualKnightExhaustion)
            } else if self.board.is_attrition_defeated(player.opposite()) {
                GameState::Victory(player)
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
        Ok(Some(TurnResult::PieceMove { player, from, to }))
    }

    fn apply_knight_removal(
        &mut self,
        player: PlayerKind,
        at: Cell,
    ) -> Result<Option<TurnResult>, GameError> {
        // Same deserialized-index guard as `apply_move`.
        if at.to_index() >= CELL_COUNT {
            return Err(GameError::EmptyKnightRemoval(player, at));
        }
        match self.board.cells[at.to_index()] {
            None => Err(GameError::EmptyKnightRemoval(player, at)),
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
                    Err(GameError::EnemyKnightRemoval(player, at))
                }
            }
        }
    }
}

impl PlayerKind {
    #[inline]
    pub const fn opposite(self) -> Self {
        match self {
            PlayerKind::White => PlayerKind::Black,
            PlayerKind::Black => PlayerKind::White,
        }
    }

    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            PlayerKind::White => "White",
            PlayerKind::Black => "Black",
        }
    }

    #[inline]
    pub const fn symbol(self) -> char {
        match self {
            PlayerKind::White => 'W',
            PlayerKind::Black => 'B',
        }
    }
}

impl PieceKind {
    #[inline]
    pub const fn symbol(self) -> char {
        match self {
            PieceKind::Crown => 'C',
            PieceKind::Knight => 'K',
            PieceKind::Spy => 'S',
        }
    }

    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            PieceKind::Crown => "Crown",
            PieceKind::Knight => "Knight",
            PieceKind::Spy => "Spy",
        }
    }
}
