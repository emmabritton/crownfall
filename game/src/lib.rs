#![no_std]

extern crate alloc;

pub mod ai;
pub mod errors;
mod hash;
pub mod impls;
mod tables;

pub mod prelude {
    pub use crate::errors::*;
    pub use crate::*;
}

use alloc::vec::Vec;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PlayState {
    WaitingForInput {
        player: PlayerKind,
    },
    MustRemoveKnight {
        player: PlayerKind,
        options: (Cell, Cell),
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GameState {
    Playing(PlayState),
    Victory(PlayerKind),
    Draw(DrawReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DrawReason {
    /// The same position (board + player to move) occurred three times.
    Repetition,
    /// No capture occurred for a set number of consecutive turns.
    NoProgress,
    /// The game reached the absolute turn-count safety net.
    TurnLimit,
    /// A Knight Capture left one player with a single Knight and the other
    /// with none, both as a result of the same move.
    MutualKnightExhaustion,
}

impl DrawReason {
    pub const fn description(&self) -> &'static str {
        match self {
            DrawReason::Repetition => "threefold repetition",
            DrawReason::NoProgress => "no captures for too long",
            DrawReason::TurnLimit => "turn limit reached",
            DrawReason::MutualKnightExhaustion => "both sides out of knights",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Piece {
    pub kind: PieceKind,
    pub player: PlayerKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct BoardState {
    #[serde(with = "BigArray")]
    pub cells: [Option<Piece>; BOARD_SIZE.0 * BOARD_SIZE.1],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Game {
    pub board: BoardState,
    pub state: GameState,
    /// Hashes of past positions (board + player to move), used to detect
    /// threefold repetition. Grows for the lifetime of the game.
    #[serde(default)]
    pub history: Vec<u64>,
    /// Turns played since the last capture, used for the no-progress draw
    /// rule. Reset to 0 on every capture.
    #[serde(default)]
    pub moves_since_capture: u16,
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
        let (x, y) = tables::COORD[self.index];
        (x as usize, y as usize)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum PlayerKind {
    White,
    Black,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum TurnResult {
    PieceMove {
        player: PlayerKind,
        from: Cell,
        to: Cell,
    },
    Capture {
        player: PlayerKind,
        last_move_from: Cell,
        last_move_to: Cell,
        removed: Cell,
        second_attacker: Cell,
    },
    Victory {
        player: PlayerKind,
        surrounded_crown: Cell,
    },
}
