use crate::errors::GameError;
use crate::hash::Fnv1aHasher;
use crate::*;
use alloc::vec;
use alloc::vec::Vec;
use core::hash::{Hash, Hasher};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CaptureKind {
    Spy,
    Knight,
}

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

fn position_hash(board: &BoardState, next_player: PlayerKind) -> u64 {
    let mut hasher = Fnv1aHasher::default();
    board.hash(&mut hasher);
    next_player.hash(&mut hasher);
    hasher.finish()
}

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
    let repeats = history.iter().filter(|&&h| h == key).count();
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
    /// Legal move destinations for the piece at `cell`. Knights move
    /// orthogonally like every other piece, except they may never move
    /// backward (away from the opponent's starting rows) — forward, left,
    /// and right only. Their old diagonal-forward reach is now the shape of
    /// their *capture* threat instead (see `knight_forward_neighbours`).
    pub fn get_valid_destinations_for(&self, cell: Cell) -> Vec<Cell> {
        let Some(piece) = self.cells[cell.to_index()] else {
            return Vec::new();
        };

        let candidates = if piece.kind == PieceKind::Knight {
            Self::knight_move_neighbours(cell, piece.player)
        } else {
            Self::orthogonal_neighbours(cell)
        };

        candidates
            .into_iter()
            .filter(|neighbour| self.cells[neighbour.to_index()].is_none())
            .collect()
    }

    fn orthogonal_neighbours(cell: Cell) -> Vec<Cell> {
        let (x, y) = cell.to_coord();
        let mut neighbours = Vec::with_capacity(4);
        if y > 0 {
            neighbours.push(Cell::new_coord(x, y - 1));
        }
        if y + 1 < BOARD_LENGTH {
            neighbours.push(Cell::new_coord(x, y + 1));
        }
        if x > 0 {
            neighbours.push(Cell::new_coord(x - 1, y));
        }
        if x + 1 < BOARD_LENGTH {
            neighbours.push(Cell::new_coord(x + 1, y));
        }
        neighbours
    }

    /// The y-axis direction a player's Knights advance in: Black starts near
    /// row A (y=0) and advances toward row G (+y); White starts near row G
    /// and advances toward row A (-y).
    fn knight_forward_dir(player: PlayerKind) -> isize {
        match player {
            PlayerKind::Black => 1,
            PlayerKind::White => -1,
        }
    }

    /// The orthogonal neighbours of a Knight's cell, excluding the backward
    /// one (away from the opponent's starting rows) — forward, left, and
    /// right are legal Knight moves, matching every other piece except for
    /// that one missing direction.
    fn knight_move_neighbours(cell: Cell, player: PlayerKind) -> Vec<Cell> {
        let (x, y) = cell.to_coord();
        let backward_y = y as isize - Self::knight_forward_dir(player);
        Self::orthogonal_neighbours(cell)
            .into_iter()
            .filter(|neighbour| {
                let (nx, ny) = neighbour.to_coord();
                !(nx == x && ny as isize == backward_y)
            })
            .collect()
    }

    /// The (up to) 3 cells in the row ahead of a Knight: straight forward,
    /// forward-left, and forward-right. This is no longer a movement shape —
    /// it's the Knight's *capture* reach: a Knight can only take part in a
    /// Knight Capture pincer against a target that falls within this arc
    /// from its own cell (a Knight directly beside or behind a target does
    /// not count, only ahead or diagonally ahead of it). A Knight that just
    /// moved must additionally land in one of the two *diagonal* cells to be
    /// the piece completing the pincer — see `is_diagonally_forward_of`.
    fn knight_forward_neighbours(cell: Cell, player: PlayerKind) -> Vec<Cell> {
        let (x, y) = cell.to_coord();
        let forward_y = y as isize + Self::knight_forward_dir(player);
        if forward_y < 0 || forward_y as usize >= BOARD_LENGTH {
            return Vec::new();
        }
        let forward_y = forward_y as usize;
        [-1isize, 0, 1]
            .into_iter()
            .filter_map(|dx| {
                let nx = x as isize + dx;
                (nx >= 0 && (nx as usize) < BOARD_LENGTH)
                    .then(|| Cell::new_coord(nx as usize, forward_y))
            })
            .collect()
    }

    /// True if `attacker_cell` is diagonally (not straight) ahead of `target`
    /// from `attacker`'s forward direction — i.e. one of the two diagonal
    /// cells in `knight_forward_neighbours(target, attacker.opposite())`,
    /// excluding the straight-ahead one. A Knight that just moved must land
    /// here (not merely directly ahead) to be the piece completing a Knight
    /// Capture pincer — see `check_piece_captures`/`check_crown_capture`.
    fn is_diagonally_forward_of(target: Cell, attacker_cell: Cell, attacker: PlayerKind) -> bool {
        let (tx, _) = target.to_coord();
        let (ax, _) = attacker_cell.to_coord();
        ax != tx
            && Self::knight_forward_neighbours(target, attacker.opposite()).contains(&attacker_cell)
    }

    fn piece_count(&self, player: PlayerKind, kind: PieceKind) -> usize {
        self.cells
            .iter()
            .flatten()
            .filter(|piece| piece.player == player && piece.kind == kind)
            .count()
    }

    /// Finds a valid capturing pincer against `target` occupied by `attacker`-owned
    /// pieces. Crown and Spy attackers only need plain orthogonal adjacency (any of
    /// the 4 sides); Knight attackers additionally need `target` to fall within their
    /// own forward arc — a Knight standing beside or behind `target` cannot form a
    /// Knight Capture pincer, only one standing ahead of or diagonally ahead of it can
    /// (see `knight_forward_neighbours`). Whether the just-moved piece specifically
    /// must land diagonally (not just anywhere in the arc) is enforced by callers via
    /// `is_diagonally_forward_of`, not here — this only finds *some* valid pair.
    /// Extra attacker-owned pieces also adjacent to
    /// `target` (of any kind) must not block a genuine pincer formed by two others.
    fn find_attacking_pair(
        &self,
        target: Cell,
        attacker: PlayerKind,
    ) -> Option<((Cell, Cell), CaptureKind)> {
        let mut attackers: Vec<Cell> = Self::orthogonal_neighbours(target)
            .into_iter()
            .filter(|neighbour| {
                matches!(self.cells[neighbour.to_index()], Some(piece) if piece.player == attacker && piece.kind != PieceKind::Knight)
            })
            .collect();
        attackers.extend(Self::knight_forward_neighbours(target, attacker.opposite())
            .into_iter()
            .filter(|neighbour| {
                matches!(self.cells[neighbour.to_index()], Some(piece) if piece.player == attacker && piece.kind == PieceKind::Knight)
            }));

        for i in 0..attackers.len() {
            for j in (i + 1)..attackers.len() {
                let pair = (attackers[i], attackers[j]);
                if let Some(kind) = self.capture_kind(pair) {
                    return Some((pair, kind));
                }
            }
        }
        None
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
        let mut attackers: Vec<Cell> = Self::orthogonal_neighbours(target)
            .into_iter()
            .filter(|neighbour| {
                matches!(self.cells[neighbour.to_index()], Some(piece) if piece.player == attacker)
            })
            .collect();

        if Self::is_diagonally_forward_of(target, moved, attacker)
            && matches!(self.cells[moved.to_index()], Some(piece) if piece.player == attacker && piece.kind == PieceKind::Knight)
        {
            attackers.push(moved);
        }

        for i in 0..attackers.len() {
            for j in (i + 1)..attackers.len() {
                let pair = (attackers[i], attackers[j]);
                if let Some(kind) = self.capture_kind(pair) {
                    return Some((pair, kind));
                }
            }
        }
        None
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
    /// `knight_forward_neighbours`).
    fn capture_scan_cells(&self, to: Cell, mover: PlayerKind) -> Vec<Cell> {
        let mut cells = Self::orthogonal_neighbours(to);
        if matches!(self.cells[to.to_index()], Some(piece) if piece.kind == PieceKind::Knight) {
            for cell in Self::knight_forward_neighbours(to, mover) {
                if !cells.contains(&cell) {
                    cells.push(cell);
                }
            }
        }
        cells
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
        self.capture_scan_cells(to, attacker)
            .into_iter()
            .find_map(|neighbour| {
                let piece = self.cells[neighbour.to_index()]?;
                if piece.player == attacker || piece.kind != PieceKind::Crown {
                    return None;
                }
                self.find_crown_attacking_pair(neighbour, attacker, to)?;
                Some(neighbour)
            })
    }

    fn check_piece_captures(
        &self,
        to: Cell,
        attacker: PlayerKind,
    ) -> Vec<(Cell, CaptureKind, (Cell, Cell))> {
        self.capture_scan_cells(to, attacker)
            .into_iter()
            .filter_map(|neighbour| {
                let piece = self.cells[neighbour.to_index()]?;
                if piece.player == attacker || piece.kind == PieceKind::Crown {
                    return None;
                }
                let (attackers, kind) = self.find_attacking_pair(neighbour, attacker)?;
                if !self.moved_knight_completes_pincer(to, neighbour, attacker) {
                    return None;
                }
                Some((neighbour, kind, attackers))
            })
            .collect()
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
        self,
        action: PlayerAction,
    ) -> Result<(Game, Option<TurnResult>), GameError> {
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
            PlayerAction::Move { player, from, to } => self.handle_move(player, from, to),
            PlayerAction::KnightRemoval { player, at } => self.handle_knight_removal(player, at),
            PlayerAction::Surrender { player } => Ok((
                Game {
                    board: self.board,
                    state: GameState::Victory(player.opposite()),
                    history: self.history,
                    moves_since_capture: self.moves_since_capture,
                },
                None,
            )),
        }
    }

    fn handle_move(
        mut self,
        player: PlayerKind,
        from: Cell,
        to: Cell,
    ) -> Result<(Game, Option<TurnResult>), GameError> {
        let piece = self.board.cells[from.to_index()].ok_or(GameError::EmptyMove(player, from))?;
        if piece.player != player {
            return Err(GameError::EnemyMove(player, from));
        }
        if !self.board.get_valid_destinations_for(from).contains(&to) {
            return Err(GameError::InvalidDestination(player, from, to));
        }

        let mut board = self.board.clone();
        board.cells[from.to_index()] = None;
        board.cells[to.to_index()] = Some(piece);

        // Crown-loss has the highest priority of any capture and is checked first,
        // even ahead of a capture this same move would otherwise complete (README
        // "Crown" section — the crown moving into a trap loses the game outright).
        if board.check_own_crown_trap(to, player) {
            board.cells[to.to_index()] = None;
            return Ok((
                Game {
                    board,
                    state: GameState::Victory(player.opposite()),
                    history: self.history,
                    moves_since_capture: self.moves_since_capture,
                },
                Some(TurnResult::Victory {
                    player: player.opposite(),
                    surrounded_crown: to,
                }),
            ));
        }

        if let Some(surrounded_crown) = board.check_crown_capture(to, player) {
            board.cells[surrounded_crown.to_index()] = None;
            return Ok((
                Game {
                    board,
                    state: GameState::Victory(player),
                    history: self.history,
                    moves_since_capture: self.moves_since_capture,
                },
                Some(TurnResult::Victory {
                    player,
                    surrounded_crown,
                }),
            ));
        }

        // Spy Capture applies "even if the enemy moved there" — the piece just moved
        // can walk straight into a pre-existing enemy Spy pincer and be captured by it.
        if board.check_self_spy_trap(to, player) {
            board.cells[to.to_index()] = None;
            let attackers = board
                .find_attacking_pair(to, player.opposite())
                .expect("check_self_spy_trap confirmed an attacking pair");
            let state = if board.is_attrition_defeated(player) {
                GameState::Victory(player.opposite())
            } else {
                resolve_continuation(
                    &board,
                    player.opposite(),
                    &mut self.history,
                    &mut self.moves_since_capture,
                    true,
                )
            };
            return Ok((
                Game {
                    board,
                    state,
                    history: self.history,
                    moves_since_capture: self.moves_since_capture,
                },
                Some(TurnResult::Capture {
                    player,
                    last_move_from: from,
                    last_move_to: to,
                    removed: to,
                    second_attacker: attackers.0.1,
                }),
            ));
        }

        let captures = board.check_piece_captures(to, player);
        if !captures.is_empty() {
            let mut turn_result = None;
            for (target, kind, attackers) in captures {
                let target_kind =
                    board.cells[target.to_index()].map(|target_piece| target_piece.kind);
                board.cells[target.to_index()] = None;
                let second_attacker = BoardState::other_attacker(attackers, to);
                // The attacking player only loses one of their own knights when the
                // *captured piece itself* was a Knight (README "Knight Capture") — a
                // Knight+Knight/Knight+Crown pincer capturing a Spy carries no penalty.
                if kind == CaptureKind::Knight && target_kind == Some(PieceKind::Knight) {
                    let lost_knight = if piece.kind == PieceKind::Crown {
                        second_attacker
                    } else {
                        to
                    };
                    board.cells[lost_knight.to_index()] = None;
                }
                let captured_this = TurnResult::Capture {
                    player,
                    last_move_from: from,
                    last_move_to: to,
                    removed: target,
                    second_attacker,
                };
                turn_result.get_or_insert(captured_this);
            }
            let turn_result = turn_result.expect("captures is non-empty");

            let state = if board.is_mutual_knight_exhaustion() {
                GameState::Draw(DrawReason::MutualKnightExhaustion)
            } else if board.is_attrition_defeated(player.opposite()) {
                GameState::Victory(player)
            } else {
                resolve_continuation(
                    &board,
                    player.opposite(),
                    &mut self.history,
                    &mut self.moves_since_capture,
                    true,
                )
            };

            return Ok((
                Game {
                    board,
                    state,
                    history: self.history,
                    moves_since_capture: self.moves_since_capture,
                },
                Some(turn_result),
            ));
        }

        let state = resolve_continuation(
            &board,
            player.opposite(),
            &mut self.history,
            &mut self.moves_since_capture,
            false,
        );
        Ok((
            Game {
                board,
                state,
                history: self.history,
                moves_since_capture: self.moves_since_capture,
            },
            Some(TurnResult::PieceMove { player, from, to }),
        ))
    }

    fn handle_knight_removal(
        mut self,
        player: PlayerKind,
        at: Cell,
    ) -> Result<(Game, Option<TurnResult>), GameError> {
        match self.board.cells[at.to_index()] {
            None => Err(GameError::EmptyKnightRemoval(player, at)),
            Some(cell) => {
                if cell.player == player {
                    let mut new_board = self.board.clone();
                    new_board.cells[at.index] = None;
                    let state = resolve_continuation(
                        &new_board,
                        player.opposite(),
                        &mut self.history,
                        &mut self.moves_since_capture,
                        true,
                    );
                    Ok((
                        Game {
                            board: new_board,
                            state,
                            history: self.history,
                            moves_since_capture: self.moves_since_capture,
                        },
                        None,
                    ))
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
