//! Position hashing for repetition detection: FNV-1a over a packed one-byte-
//! per-cell board encoding plus the player to move. Written directly (no
//! `Hasher` trait, no derived `Hash` walking `Option` discriminants) — this
//! crate is `no_std` + `alloc` so `std`'s `DefaultHasher` isn't available,
//! and the hash runs once per applied move, including every node of the AI
//! search, so it needs to stay one XOR + one multiply per cell.
use crate::{CrownfallBoardState, CrownfallPlayerKind};

const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

/// Hash of a position: board contents + player to move. Two positions
/// compare equal for the threefold-repetition rule iff both match.
pub(crate) fn position_hash(board: &CrownfallBoardState, next_player: CrownfallPlayerKind) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for cell in &board.cells {
        let byte = match cell {
            None => 0u8,
            Some(piece) => 1 + piece.kind as u8 + 3 * piece.player as u8,
        };
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash ^= next_player as u8 as u64;
    hash.wrapping_mul(FNV_PRIME)
}
