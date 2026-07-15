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
use core::num::NonZeroU8;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "serde")]
use serde_big_array::BigArray;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CrownfallPieceKind {
    // Explicit discriminants: these double as the low two bits of
    // `CrownfallPiece`'s packed byte, so reordering them would silently
    // change the packing (and the serialized piece bytes).
    Crown = 0,
    Knight = 1,
    Spy = 2,
    /// Grand-variant only: captures at range 2 (orthogonal) instead of by
    /// pincer, provided an allied Crown/Knight/Spy is orthogonally adjacent
    /// to the target. See `CrownfallBoardState::check_archer_capture`.
    Archer = 3,
}

/// The three supported board sizes/piece-set combinations. Each is a
/// distinct fixed-size array (no heap) rather than a single runtime-sized
/// board, so no_std/no-alloc consumers (the GBA build) still get plain ROM
/// tables and stack-sized arrays regardless of which variant is in play.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CrownfallBoardVariant {
    /// 5x5, 3 Knights / 2 Spies / 1 Crown per side.
    Mini,
    /// 7x7, 6 Knights / 3 Spies / 1 Crown per side - the original ruleset.
    Normal,
    /// 9x9, 8 Knights / 3 Spies / 1 Crown / 2 Archers per side.
    Grand,
}

impl CrownfallBoardVariant {
    pub fn length(self) -> usize {
        match self {
            CrownfallBoardVariant::Mini => 5,
            CrownfallBoardVariant::Normal => 7,
            CrownfallBoardVariant::Grand => 9,
        }
    }
}

/// A ruleset: board size (which also implies the starting piece set) plus
/// the independent behavioural toggles. All fields are small and `Copy`,
/// so carrying `rules` on every `CrownfallGame`/passing it into move
/// generation is free compared to the array/table lookups it gates.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CrownfallRules {
    pub board: CrownfallBoardVariant,
    pub ruleset: CrownfallRuleset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CrownfallRuleset {
    Archers,
    Custom {
        /// Variant 4: if any capturing move exists for the player to move,
        /// only capturing moves are legal this turn.
        mandatory_capture: bool,
        /// Variant 5: a move that simultaneously (a) walks the mover into a
        /// pre-existing enemy pincer and (b) completes the mover's own pincer
        /// resolves both captures, instead of (a) pre-empting (b).
        all_captures_processed: bool,
        /// Variant 6: Knights move diagonally forward-only and capture
        /// orthogonally forward/left/right, instead of the standard
        /// orthogonal-forward-only movement with a diagonal-forward capture
        /// arc.
        knights_move_diagonally: bool,
    },
}

impl CrownfallRules {
    pub const fn standard() -> CrownfallRules {
        CrownfallRules {
            board: CrownfallBoardVariant::Normal,
            ruleset: CrownfallRuleset::Custom {
                mandatory_capture: false,
                all_captures_processed: false,
                knights_move_diagonally: false,
            },
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

    pub const fn standard_archers() -> CrownfallRules {
        CrownfallRules {
            board: CrownfallBoardVariant::Normal,
            ruleset: CrownfallRuleset::Archers,
        }
    }

    pub const fn mini_archers() -> CrownfallRules {
        CrownfallRules {
            board: CrownfallBoardVariant::Mini,
            ruleset: CrownfallRuleset::Archers,
        }
    }

    pub const fn grand_archers() -> CrownfallRules {
        CrownfallRules {
            board: CrownfallBoardVariant::Grand,
            ruleset: CrownfallRuleset::Archers,
        }
    }

    pub const fn standard_mandatory_capture() -> CrownfallRules {
        CrownfallRules {
            ruleset: CrownfallRuleset::Custom {
                mandatory_capture: true,
                all_captures_processed: false,
                knights_move_diagonally: false,
            },
            ..CrownfallRules::standard()
        }
    }

    pub const fn standard_all_captures_processed() -> CrownfallRules {
        CrownfallRules {
            ruleset: CrownfallRuleset::Custom {
                mandatory_capture: false,
                all_captures_processed: true,
                knights_move_diagonally: false,
            },
            ..CrownfallRules::standard()
        }
    }

    pub const fn standard_diagonal_knights() -> CrownfallRules {
        CrownfallRules {
            ruleset: CrownfallRuleset::Custom {
                mandatory_capture: false,
                all_captures_processed: false,
                knights_move_diagonally: true,
            },
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
    Victory(CrownfallPlayerKind, WinReason),
    Draw(DrawReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum WinReason {
    /// The loser's Crown was surrounded by two enemy Spies, two enemy
    /// Knights, or an enemy Knight and Crown - whether by the winner's move
    /// completing the pincer, or the loser's own Crown walking into one
    /// that already existed.
    CrownCaptured,
    /// The loser was left with one or fewer Knights and one or fewer Spies.
    Attrition,
    /// The loser surrendered.
    Surrender,
}

impl WinReason {
    pub const fn description(&self) -> &'static str {
        match self {
            WinReason::CrownCaptured => "crown captured",
            WinReason::Attrition => "opponent out of knights and spies",
            WinReason::Surrender => "opponent surrendered",
        }
    }
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

/// A piece on the board, packed into one byte: bits 0-1 the kind, bit 2 the
/// player, bit 3 always set (the occupancy marker that keeps the value
/// non-zero). Backing it with `NonZeroU8` gives `Option<CrownfallPiece>` the
/// niche optimization - one byte per board cell instead of two - which
/// halves every board copy and scan in the AI search's hot path (and the
/// GBA's memory traffic with it). Unpacking `kind()`/`player()` is a single
/// AND/shift, and piece equality is a plain byte compare.
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct CrownfallPiece(NonZeroU8);

impl CrownfallPiece {
    /// Bit 3: set on every valid piece, so the packed byte is never zero
    /// (the `NonZeroU8` niche) and never collides with an empty cell.
    const OCCUPIED: u8 = 0b1000;

    pub const fn new(kind: CrownfallPieceKind, player: CrownfallPlayerKind) -> CrownfallPiece {
        let byte = Self::OCCUPIED | ((player as u8) << 2) | kind as u8;
        match NonZeroU8::new(byte) {
            Some(value) => CrownfallPiece(value),
            // OCCUPIED is always set, so the byte is never zero.
            None => unreachable!(),
        }
    }

    pub const fn kind(self) -> CrownfallPieceKind {
        match self.0.get() & 0b11 {
            0 => CrownfallPieceKind::Crown,
            1 => CrownfallPieceKind::Knight,
            2 => CrownfallPieceKind::Spy,
            _ => CrownfallPieceKind::Archer,
        }
    }

    pub const fn player(self) -> CrownfallPlayerKind {
        if self.0.get() & 0b100 == 0 {
            CrownfallPlayerKind::White
        } else {
            CrownfallPlayerKind::Black
        }
    }

    /// `kind + 4 * player` - the per-piece index into the Zobrist key table
    /// (see `hash::piece_key`), which the packing makes a single mask.
    pub(crate) const fn code(self) -> usize {
        (self.0.get() & 0b111) as usize
    }

    /// The packed byte, for the serde impls below.
    #[cfg(feature = "serde")]
    const fn to_byte(self) -> u8 {
        self.0.get()
    }

    /// Inverse of `to_byte`; `None` for any byte that isn't a valid packed
    /// piece (used to validate deserialized input).
    #[cfg(feature = "serde")]
    const fn from_byte(byte: u8) -> Option<CrownfallPiece> {
        if byte & !0b111 != Self::OCCUPIED {
            return None;
        }
        match NonZeroU8::new(byte) {
            Some(value) => Some(CrownfallPiece(value)),
            None => None,
        }
    }
}

impl core::fmt::Debug for CrownfallPiece {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CrownfallPiece")
            .field("kind", &self.kind())
            .field("player", &self.player())
            .finish()
    }
}

/// Serialized as the packed byte (8-15), not a `{kind, player}` map - both
/// halves of the protocol live in this workspace, so the wire format only
/// has to agree with itself, and a bare integer keeps board packets small.
/// Deserialization rejects any byte that isn't a valid packed piece.
#[cfg(feature = "serde")]
impl Serialize for CrownfallPiece {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(self.to_byte())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for CrownfallPiece {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let byte = u8::deserialize(deserializer)?;
        CrownfallPiece::from_byte(byte).ok_or_else(|| {
            serde::de::Error::invalid_value(
                serde::de::Unexpected::Unsigned(byte as u64),
                &"a packed Crownfall piece byte (8-15)",
            )
        })
    }
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

/// A board position as a cell index. Stored as `u8` (the largest board has
/// 81 cells), which shrinks every type that carries positions - actions,
/// turn results, play state, the AI's undo records - and means a
/// deserialized index above 255 is rejected by serde outright; indices
/// 82-255 are still caught by `apply_move`'s cell-count guard. The
/// `new_index`/`to_index` API stays `usize`-based so callers can keep
/// indexing slices without casts.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CrownfallBoardCell {
    pub index: u8,
}

impl CrownfallBoardCell {
    pub fn new_index(index: usize) -> CrownfallBoardCell {
        debug_assert!(
            index <= u8::MAX as usize,
            "cell index {index} exceeds the largest supported board"
        );
        CrownfallBoardCell { index: index as u8 }
    }

    pub fn new_coord(x: usize, y: usize, board: CrownfallBoardVariant) -> CrownfallBoardCell {
        CrownfallBoardCell {
            index: (x + y * tables::board_length(board)) as u8,
        }
    }

    pub fn to_index(self) -> usize {
        self.index as usize
    }

    pub fn to_coord(self, board: CrownfallBoardVariant) -> (usize, usize) {
        let (x, y) = tables::coord(board, self.to_index());
        (x as usize, y as usize)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CrownfallPlayerKind {
    // Explicit discriminants: bit 2 of `CrownfallPiece`'s packed byte, and
    // the index into the per-player lookup tables/count arrays.
    White = 0,
    Black = 1,
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
