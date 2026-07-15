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
    /// Grand-variant only: captures at range 2 (orthogonal) instead of by
    /// pincer, provided an allied Crown/Knight/Spy is orthogonally adjacent
    /// to the target. See `CrownfallBoardState::check_archer_capture`.
    Archer,
}

/// Kept for backwards compatibility with code written against the single
/// fixed board size this crate used to support - equal to `Normal`'s
/// length/area. Prefer `CrownfallBoardVariant`/`tables::board_length` for
/// new code, since board size is now selectable per game.
pub const BOARD_LENGTH: usize = tables::NORMAL_LENGTH;
pub const BOARD_SIZE: (usize, usize) = (BOARD_LENGTH, BOARD_LENGTH);

/// The three supported board sizes/piece-set combinations. Each is a
/// distinct fixed-size array (no heap) rather than a single runtime-sized
/// board, so no_std/no-alloc consumers (the GBA build) still get plain ROM
/// tables and stack-sized arrays regardless of which variant is in play.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CrownfallBoardVariant {
    /// 5x5, 4 Knights / 3 Spies / 1 Crown per side.
    Mini,
    /// 7x7, 6 Knights / 3 Spies / 1 Crown per side - the original ruleset.
    Normal,
    /// 9x9, 8 Knights / 3 Spies / 1 Crown / 2 Archers per side.
    Grand,
}

/// A ruleset: board size (which also implies the starting piece set) plus
/// the independent behavioural toggles. All fields are small and `Copy`,
/// so carrying `rules` on every `CrownfallGame`/passing it into move
/// generation is free compared to the array/table lookups it gates.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CrownfallRules {
    pub board: CrownfallBoardVariant,
    /// Variant 4: if any capturing move exists for the player to move,
    /// only capturing moves are legal this turn.
    pub mandatory_capture: bool,
    /// Variant 5: a move that simultaneously (a) walks the mover into a
    /// pre-existing enemy pincer and (b) completes the mover's own pincer
    /// resolves both captures, instead of (a) pre-empting (b).
    pub all_captures_processed: bool,
    /// Variant 6: Knights move diagonally forward-only and capture
    /// orthogonally forward/left/right, instead of the standard
    /// orthogonal-forward-only movement with a diagonal-forward capture
    /// arc.
    pub knights_move_diagonally: bool,
}

impl CrownfallRules {
    pub const fn standard() -> CrownfallRules {
        CrownfallRules {
            board: CrownfallBoardVariant::Normal,
            mandatory_capture: false,
            all_captures_processed: false,
            knights_move_diagonally: false,
        }
    }

    pub const fn mini() -> CrownfallRules {
        CrownfallRules {
            board: CrownfallBoardVariant::Mini,
            ..CrownfallRules::standard()
        }
    }

    pub const fn grand() -> CrownfallRules {
        CrownfallRules {
            board: CrownfallBoardVariant::Grand,
            ..CrownfallRules::standard()
        }
    }

    pub const fn standard_mandatory_capture() -> CrownfallRules {
        CrownfallRules {
            mandatory_capture: true,
            ..CrownfallRules::standard()
        }
    }

    pub const fn standard_all_captures_processed() -> CrownfallRules {
        CrownfallRules {
            all_captures_processed: true,
            ..CrownfallRules::standard()
        }
    }

    pub const fn standard_diagonal_knights() -> CrownfallRules {
        CrownfallRules {
            knights_move_diagonally: true,
            ..CrownfallRules::standard()
        }
    }

    /// Archers are exclusive to the Grand board - no separate flag needed.
    pub const fn has_archers(&self) -> bool {
        matches!(self.board, CrownfallBoardVariant::Grand)
    }
}

impl Default for CrownfallRules {
    fn default() -> CrownfallRules {
        CrownfallRules::standard()
    }
}

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

/// The board itself: a fixed-size cell array per supported size. Most
/// logic (capture scanning, piece counting, ...) is written once against
/// `cells()`/`cells_mut()` slices rather than duplicated per arm - only
/// index<->coordinate math and the neighbour tables (`tables.rs`) need to
/// know which size is in play.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CrownfallBoardState {
    Mini {
        #[cfg_attr(feature = "serde", serde(with = "BigArray"))]
        cells: [Option<CrownfallPiece>; tables::MINI_CELL_COUNT],
    },
    Normal {
        #[cfg_attr(feature = "serde", serde(with = "BigArray"))]
        cells: [Option<CrownfallPiece>; tables::NORMAL_CELL_COUNT],
    },
    Grand {
        #[cfg_attr(feature = "serde", serde(with = "BigArray"))]
        cells: [Option<CrownfallPiece>; tables::GRAND_CELL_COUNT],
    },
}

impl CrownfallBoardState {
    pub fn cells(&self) -> &[Option<CrownfallPiece>] {
        match self {
            CrownfallBoardState::Mini { cells } => cells.as_slice(),
            CrownfallBoardState::Normal { cells } => cells.as_slice(),
            CrownfallBoardState::Grand { cells } => cells.as_slice(),
        }
    }

    pub fn cells_mut(&mut self) -> &mut [Option<CrownfallPiece>] {
        match self {
            CrownfallBoardState::Mini { cells } => cells.as_mut_slice(),
            CrownfallBoardState::Normal { cells } => cells.as_mut_slice(),
            CrownfallBoardState::Grand { cells } => cells.as_mut_slice(),
        }
    }

    pub const fn variant(&self) -> CrownfallBoardVariant {
        match self {
            CrownfallBoardState::Mini { .. } => CrownfallBoardVariant::Mini,
            CrownfallBoardState::Normal { .. } => CrownfallBoardVariant::Normal,
            CrownfallBoardState::Grand { .. } => CrownfallBoardVariant::Grand,
        }
    }

    pub fn board_length(&self) -> usize {
        tables::board_length(self.variant())
    }

    /// An empty board of the given size - used by variant layout builders
    /// and by tests that want full control over piece placement.
    pub fn empty(variant: CrownfallBoardVariant) -> CrownfallBoardState {
        match variant {
            CrownfallBoardVariant::Mini => CrownfallBoardState::Mini {
                cells: [None; tables::MINI_CELL_COUNT],
            },
            CrownfallBoardVariant::Normal => CrownfallBoardState::Normal {
                cells: [None; tables::NORMAL_CELL_COUNT],
            },
            CrownfallBoardVariant::Grand => CrownfallBoardState::Grand {
                cells: [None; tables::GRAND_CELL_COUNT],
            },
        }
    }
}

impl Default for CrownfallBoardState {
    /// The Standard (Normal-size) starting layout, for callers that don't
    /// care about variants. See `impls::standard_layout` for the layout
    /// itself and `CrownfallGame::new` for the other variants.
    fn default() -> CrownfallBoardState {
        impls::standard_layout()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CrownfallGame {
    pub board: CrownfallBoardState,
    pub state: CrownfallGameState,
    pub rules: CrownfallRules,
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

    pub fn new_coord(x: usize, y: usize, board: CrownfallBoardVariant) -> CrownfallBoardCell {
        CrownfallBoardCell {
            index: x + y * tables::board_length(board),
        }
    }

    pub fn to_index(self) -> usize {
        self.index
    }

    pub fn to_coord(self, board: CrownfallBoardVariant) -> (usize, usize) {
        let (x, y) = tables::coord(board, self.index);
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
