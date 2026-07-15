//! Position hashing for repetition detection: Zobrist hashing over a
//! compile-time table of per-(cell, piece) random keys. Only occupied cells
//! contribute (a plain XOR each), so a typical midgame board costs ~20 loads
//! and XORs instead of a serial XOR-multiply chain over every cell — there's
//! no data dependency between cells and no multiply at all, which matters on
//! the ARM7TDMI. The table is const-evaluated (splitmix64), so it lives in
//! ROM on the GBA build. The hash runs once per applied move, including
//! every node of the AI search.
use crate::tables::GRAND_CELL_COUNT;
use crate::{CrownfallBoardState, CrownfallPiece, CrownfallPlayerKind};

/// 4 piece kinds x 2 players.
const PIECE_CODES: usize = 8;

/// splitmix64's output mix over a counter — the standard way to generate a
/// fixed table of statistically-independent 64-bit keys in a const context.
const fn zobrist_key(index: u64) -> u64 {
    let mut z = index.wrapping_add(1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

const fn build_keys() -> [[u64; PIECE_CODES]; GRAND_CELL_COUNT] {
    let mut out = [[0u64; PIECE_CODES]; GRAND_CELL_COUNT];
    let mut cell = 0;
    while cell < GRAND_CELL_COUNT {
        let mut code = 0;
        while code < PIECE_CODES {
            out[cell][code] = zobrist_key((cell * PIECE_CODES + code) as u64);
            code += 1;
        }
        cell += 1;
    }
    out
}

/// One key per (cell, piece kind, piece owner). Sized for the largest board;
/// smaller boards use the leading entries. Hashes are only ever compared
/// within a single game (one board size), so sharing keys across sizes is
/// fine.
static KEYS: [[u64; PIECE_CODES]; GRAND_CELL_COUNT] = build_keys();

static BLACK_TO_MOVE: u64 = zobrist_key((GRAND_CELL_COUNT * PIECE_CODES) as u64);

/// The Zobrist key of one (cell, piece) pairing. XORing this into a position
/// hash adds/removes that piece, which is what lets `resolve_continuation`
/// derive each new hash incrementally from the previous one (a handful of
/// XORs) instead of rescanning the whole board every applied move.
#[inline]
pub(crate) fn piece_key(index: usize, piece: CrownfallPiece) -> u64 {
    KEYS[index][piece.code()]
}

/// The side-to-move key. The player to move flips on every applied action,
/// so an incremental hash update always XORs this exactly once.
#[inline]
pub(crate) fn side_to_move_toggle() -> u64 {
    BLACK_TO_MOVE
}

/// Hash of a position: board contents + player to move. Two positions
/// compare equal for the threefold-repetition rule iff both match.
pub(crate) fn position_hash(board: &CrownfallBoardState, next_player: CrownfallPlayerKind) -> u64 {
    let mut hash = 0u64;
    for (index, cell) in board.cells().iter().enumerate() {
        if let Some(piece) = cell {
            hash ^= piece_key(index, *piece);
        }
    }
    if matches!(next_player, CrownfallPlayerKind::Black) {
        hash ^= BLACK_TO_MOVE;
    }
    hash
}
