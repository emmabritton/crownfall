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
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "serde")]
use serde_big_array::BigArray;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CrownfallPieceKind {
    Crown,
    Knight,
    Spy,
}

pub const BOARD_LENGTH: usize = 7;

pub const BOARD_SIZE: (usize, usize) = (BOARD_LENGTH, BOARD_LENGTH);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CrownfallPlayState {
    WaitingForInput {
        player: CrownfallPlayerKind,
    },
    MustRemoveKnight {
        player: CrownfallPlayerKind,
        options: (CrownfallBoardCell, CrownfallBoardCell),
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CrownfallGameState {
    Playing(CrownfallPlayState),
    Victory(CrownfallPlayerKind),
    Draw(DrawReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CrownfallPiece {
    pub kind: CrownfallPieceKind,
    pub player: CrownfallPlayerKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CrownfallBoardState {
    #[cfg_attr(feature = "serde", serde(with = "BigArray"))]
    pub cells: [Option<CrownfallPiece>; BOARD_SIZE.0 * BOARD_SIZE.1],
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CrownfallGame {
    pub board: CrownfallBoardState,
    pub state: CrownfallGameState,
    /// Hashes of past positions (board + player to move), used to detect
    /// threefold repetition. Grows for the lifetime of the game.
    #[cfg_attr(feature = "serde", serde(default))]
    pub history: Vec<u64>,
    /// Turns played since the last capture, used for the no-progress draw
    /// rule. Reset to 0 on every capture.
    #[cfg_attr(feature = "serde", serde(default))]
    pub moves_since_capture: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CrownfallBoardCell {
    pub index: usize,
}

impl CrownfallBoardCell {
    pub fn new_index(index: usize) -> CrownfallBoardCell {
        CrownfallBoardCell { index }
    }

    pub fn new_coord(x: usize, y: usize) -> CrownfallBoardCell {
        CrownfallBoardCell {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CrownfallPlayerKind {
    White,
    Black,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CrownfallPlayerAction {
    Move {
        player: CrownfallPlayerKind,
        from: CrownfallBoardCell,
        to: CrownfallBoardCell,
    },
    KnightRemoval {
        player: CrownfallPlayerKind,
        at: CrownfallBoardCell,
    },
    Surrender {
        player: CrownfallPlayerKind,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CrownfallTurnResult {
    PieceMove {
        player: CrownfallPlayerKind,
        from: CrownfallBoardCell,
        to: CrownfallBoardCell,
    },
    Capture {
        player: CrownfallPlayerKind,
        last_move_from: CrownfallBoardCell,
        last_move_to: CrownfallBoardCell,
        removed: CrownfallBoardCell,
        second_attacker: CrownfallBoardCell,
    },
    Victory {
        player: CrownfallPlayerKind,
        surrounded_crown: CrownfallBoardCell,
    },
}
