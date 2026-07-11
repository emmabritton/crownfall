use crate::errors::GameError;
use crate::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CaptureKind {
    Spy,
    Knight,
}

impl Default for BoardState {
    fn default() -> BoardState {
        BoardState {
            cells: [
                None,
                None,
                Some(Piece {
                    kind: PieceKind::Knight,
                    player: PlayerKind::Black,
                }),
                Some(Piece {
                    kind: PieceKind::Crown,
                    player: PlayerKind::Black,
                }),
                Some(Piece {
                    kind: PieceKind::Knight,
                    player: PlayerKind::Black,
                }),
                None,
                None,
                Some(Piece {
                    kind: PieceKind::Spy,
                    player: PlayerKind::Black,
                }),
                Some(Piece {
                    kind: PieceKind::Spy,
                    player: PlayerKind::Black,
                }),
                Some(Piece {
                    kind: PieceKind::Knight,
                    player: PlayerKind::Black,
                }),
                Some(Piece {
                    kind: PieceKind::Knight,
                    player: PlayerKind::Black,
                }),
                Some(Piece {
                    kind: PieceKind::Knight,
                    player: PlayerKind::Black,
                }),
                Some(Piece {
                    kind: PieceKind::Spy,
                    player: PlayerKind::Black,
                }),
                Some(Piece {
                    kind: PieceKind::Spy,
                    player: PlayerKind::Black,
                }),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(Piece {
                    kind: PieceKind::Spy,
                    player: PlayerKind::White,
                }),
                Some(Piece {
                    kind: PieceKind::Spy,
                    player: PlayerKind::White,
                }),
                Some(Piece {
                    kind: PieceKind::Knight,
                    player: PlayerKind::White,
                }),
                Some(Piece {
                    kind: PieceKind::Knight,
                    player: PlayerKind::White,
                }),
                Some(Piece {
                    kind: PieceKind::Knight,
                    player: PlayerKind::White,
                }),
                Some(Piece {
                    kind: PieceKind::Spy,
                    player: PlayerKind::White,
                }),
                Some(Piece {
                    kind: PieceKind::Spy,
                    player: PlayerKind::White,
                }),
                None,
                None,
                Some(Piece {
                    kind: PieceKind::Knight,
                    player: PlayerKind::White,
                }),
                Some(Piece {
                    kind: PieceKind::Crown,
                    player: PlayerKind::White,
                }),
                Some(Piece {
                    kind: PieceKind::Knight,
                    player: PlayerKind::White,
                }),
                None,
                None,
            ],
        }
    }
}

impl Default for Game {
    fn default() -> Self {
        Self {
            board: Default::default(),
            state: GameState::Playing(PlayState::WaitingForInput {
                player: PlayerKind::White,
            }),
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
    pub fn get_valid_destinations_for(&self, cell: Cell) -> Vec<Cell> {
        if self.cells[cell.to_index()].is_none() {
            return Vec::new();
        }

        Self::orthogonal_neighbours(cell)
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

    fn piece_count(&self, player: PlayerKind, kind: PieceKind) -> usize {
        self.cells
            .iter()
            .flatten()
            .filter(|piece| piece.player == player && piece.kind == kind)
            .count()
    }

    /// Returns the two attacking cells surrounding `target` if exactly two of its
    /// orthogonal neighbours are occupied by `attacker`'s pieces.
    fn find_attacking_pair(&self, target: Cell, attacker: PlayerKind) -> Option<(Cell, Cell)> {
        let mut attackers = Self::orthogonal_neighbours(target).into_iter().filter(
            |neighbour| matches!(self.cells[neighbour.to_index()], Some(piece) if piece.player == attacker),
        );

        let first = attackers.next()?;
        let second = attackers.next()?;
        if attackers.next().is_some() {
            return None;
        }
        Some((first, second))
    }

    /// Determines which capture rule (if any) the attacking pair satisfies.
    fn capture_kind(&self, attackers: (Cell, Cell)) -> Option<CaptureKind> {
        let a = self.cells[attackers.0.to_index()]?.kind;
        let b = self.cells[attackers.1.to_index()]?.kind;
        match (a, b) {
            (PieceKind::Spy, PieceKind::Spy) => Some(CaptureKind::Spy),
            (PieceKind::Knight, PieceKind::Knight) => Some(CaptureKind::Knight),
            (PieceKind::Crown, PieceKind::Spy) | (PieceKind::Spy, PieceKind::Crown) => {
                Some(CaptureKind::Spy)
            }
            (PieceKind::Crown, PieceKind::Knight) | (PieceKind::Knight, PieceKind::Crown) => {
                Some(CaptureKind::Knight)
            }
            _ => None,
        }
    }

    fn check_crown_capture(&self, to: Cell, attacker: PlayerKind) -> Option<Cell> {
        Self::orthogonal_neighbours(to)
            .into_iter()
            .find_map(|neighbour| {
                let piece = self.cells[neighbour.to_index()]?;
                if piece.player == attacker || piece.kind != PieceKind::Crown {
                    return None;
                }
                let attackers = self.find_attacking_pair(neighbour, attacker)?;
                self.capture_kind(attackers)?;
                Some(neighbour)
            })
    }

    fn check_piece_captures(
        &self,
        to: Cell,
        attacker: PlayerKind,
    ) -> Vec<(Cell, CaptureKind, (Cell, Cell))> {
        Self::orthogonal_neighbours(to)
            .into_iter()
            .filter_map(|neighbour| {
                let piece = self.cells[neighbour.to_index()]?;
                if piece.player == attacker || piece.kind == PieceKind::Crown {
                    return None;
                }
                let attackers = self.find_attacking_pair(neighbour, attacker)?;
                let kind = self.capture_kind(attackers)?;
                Some((neighbour, kind, attackers))
            })
            .collect()
    }

    fn is_attrition_defeated(&self, player: PlayerKind) -> bool {
        self.piece_count(player, PieceKind::Knight) <= 1
            && self.piece_count(player, PieceKind::Spy) <= 1
    }
}

impl Game {
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
        }
        match action {
            PlayerAction::Move { player, from, to } => self.handle_move(player, from, to),
            PlayerAction::KnightRemoval { player, at } => self.handle_knight_removal(player, at),
            PlayerAction::Surrender { player } => Ok((
                Game {
                    board: self.board,
                    state: GameState::Victory(player.opposite()),
                },
                None,
            )),
        }
    }

    fn handle_move(
        self,
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

        if let Some(surrounded_crown) = board.check_crown_capture(to, player) {
            board.cells[surrounded_crown.to_index()] = None;
            return Ok((
                Game {
                    board,
                    state: GameState::Victory(player),
                },
                Some(TurnResult::Victory { surrounded_crown }),
            ));
        }

        let captures = board.check_piece_captures(to, player);
        if !captures.is_empty() {
            let mut turn_result = None;
            for (target, kind, attackers) in captures {
                board.cells[target.to_index()] = None;
                let captured_this = match kind {
                    CaptureKind::Spy => TurnResult::Capture {
                        last_move_from: from,
                        last_move_to: to,
                        removed: target,
                        second_attacker: attackers.1,
                    },
                    CaptureKind::Knight => {
                        let lost_knight = if piece.kind == PieceKind::Crown {
                            if attackers.0 == to {
                                attackers.1
                            } else {
                                attackers.0
                            }
                        } else {
                            to
                        };
                        board.cells[lost_knight.to_index()] = None;
                        TurnResult::Capture {
                            last_move_from: from,
                            last_move_to: to,
                            removed: target,
                            second_attacker: attackers.1,
                        }
                    }
                };
                turn_result.get_or_insert(captured_this);
            }
            let turn_result = turn_result.expect("captures is non-empty");

            let state = if board.is_attrition_defeated(player.opposite()) {
                GameState::Victory(player)
            } else {
                GameState::Playing(PlayState::WaitingForInput {
                    player: player.opposite(),
                })
            };

            return Ok((Game { board, state }, Some(turn_result)));
        }

        Ok((
            Game {
                board,
                state: GameState::Playing(PlayState::WaitingForInput {
                    player: player.opposite(),
                }),
            },
            Some(TurnResult::PieceMove { from, to }),
        ))
    }

    fn handle_knight_removal(
        self,
        player: PlayerKind,
        at: Cell,
    ) -> Result<(Game, Option<TurnResult>), GameError> {
        match self.board.cells[at.to_index()] {
            None => Err(GameError::EmptyKnightRemoval(player, at)),
            Some(cell) => {
                if cell.player == player {
                    let mut new_board = self.board.clone();
                    new_board.cells[at.index] = None;
                    Ok((
                        Game {
                            board: new_board,
                            state: GameState::Playing(PlayState::WaitingForInput {
                                player: player.opposite(),
                            }),
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
