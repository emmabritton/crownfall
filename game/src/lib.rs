pub mod errors;
pub mod impls;

pub mod prelude {
    pub use crate::errors::*;
    pub use crate::impls::*;
    pub use crate::*;
}

use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum PieceKind {
    Crown,
    Knight,
    Spy,
}

pub const BOARD_LENGTH: usize = 7;

pub const BOARD_SIZE: (usize, usize) = (BOARD_LENGTH, BOARD_LENGTH);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum PlayerKind {
    White,
    Black,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PlayerAction {
    Move {
        player: PlayerKind,
        from: Cell,
        to: Cell,
    },
    KnightRemoval {
        player: PlayerKind,
        at: Cell,
    },
    Surrender {
        player: PlayerKind,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum TurnResult {
    PieceMove {
        from: Cell,
        to: Cell,
    },
    Capture {
        last_move_from: Cell,
        last_move_to: Cell,
        removed: Cell,
        second_attacker: Cell,
    },
    Victory {
        surrounded_crown: Cell,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PlayState {
    WaitingForInput {
        player: PlayerKind,
    },
    MustRemoveKnight {
        player: PlayerKind,
        options: (Cell, Cell),
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GameState {
    Playing(PlayState),
    Victory(PlayerKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Piece {
    pub kind: PieceKind,
    pub player: PlayerKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BoardState {
    #[serde(with = "BigArray")]
    pub cells: [Option<Piece>; BOARD_SIZE.0 * BOARD_SIZE.1],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Game {
    pub board: BoardState,
    pub state: GameState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Cell {
    pub index: usize,
}

impl Cell {
    pub fn new_index(index: usize) -> Cell {
        Cell { index }
    }

    pub fn new_coord(x: usize, y: usize) -> Cell {
        Cell {
            index: x + y * BOARD_LENGTH,
        }
    }

    pub fn to_index(self) -> usize {
        self.index
    }

    pub fn to_coord(self) -> (usize, usize) {
        (self.index % BOARD_LENGTH, self.index / BOARD_LENGTH)
    }
}
