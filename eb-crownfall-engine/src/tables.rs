//! Compile-time lookup tables for each supported board size, stored in ROM.
//! Every neighbour relationship is precomputed per size, so the hot paths
//! (AI search, capture checks) do plain table lookups instead of coordinate
//! math - no division/modulo, no bounds branches, and no heap allocation
//! for neighbour lists on the ARM7TDMI.
//!
//! Const generics are used only inside this module, purely to avoid
//! writing each builder three times (once per board size) - every public
//! item here is a plain runtime function taking a `CrownfallBoardVariant`,
//! so nothing outside this module needs to know about generics.
use crate::{CrownfallBoardVariant, CrownfallPlayerKind};

pub(crate) const MINI_LENGTH: usize = 5;
pub(crate) const NORMAL_LENGTH: usize = 7;
pub(crate) const GRAND_LENGTH: usize = 9;

pub(crate) const MINI_CELL_COUNT: usize = MINI_LENGTH * MINI_LENGTH;
pub(crate) const NORMAL_CELL_COUNT: usize = NORMAL_LENGTH * NORMAL_LENGTH;
pub(crate) const GRAND_CELL_COUNT: usize = GRAND_LENGTH * GRAND_LENGTH;

/// A fixed neighbour list. 4 orthogonal neighbours is the largest shape any
/// table needs (Archer range is also at most 4: up/down/left/right two
/// tiles out).
#[derive(Clone, Copy)]
pub(crate) struct CellList {
    cells: [u8; 4],
    len: u8,
}

impl CellList {
    const EMPTY: CellList = CellList {
        cells: [0; 4],
        len: 0,
    };

    const fn push(&mut self, index: usize) {
        self.cells[self.len as usize] = index as u8;
        self.len += 1;
    }

    pub(crate) fn as_slice(&self) -> &[u8] {
        &self.cells[..self.len as usize]
    }
}

enum Skip {
    Up,
    Down,
}

/// Orthogonal neighbours, optionally omitting one direction - `None` is the
/// plain `ORTHO` table; `Some(Skip::Down)`/`Some(Skip::Up)` produce the
/// ortho-minus-backward Knight movement table for White/Black respectively.
const fn build_ortho<const LEN: usize, const N: usize>(skip: Option<Skip>) -> [CellList; N] {
    assert!(N == LEN * LEN, "N must be LEN*LEN");
    let mut out = [CellList::EMPTY; N];
    let mut index = 0;
    while index < N {
        let x = index % LEN;
        let y = index / LEN;
        if y > 0 && !matches!(skip, Some(Skip::Up)) {
            out[index].push(index - LEN);
        }
        if y + 1 < LEN && !matches!(skip, Some(Skip::Down)) {
            out[index].push(index + LEN);
        }
        if x > 0 {
            out[index].push(index - 1);
        }
        if x + 1 < LEN {
            out[index].push(index + 1);
        }
        index += 1;
    }
    out
}

/// The 3-cell forward arc (forward-left, straight-forward, forward-right)
/// of a piece standing at each cell. `forward_down` is true for Black
/// (advances toward the high-y edge), false for White.
const fn build_arcs<const LEN: usize, const N: usize>(forward_down: bool) -> [CellList; N] {
    assert!(N == LEN * LEN, "N must be LEN*LEN");
    let mut out = [CellList::EMPTY; N];
    let mut index = 0;
    while index < N {
        let x = index % LEN;
        let y = index / LEN;
        let in_bounds = if forward_down { y + 1 < LEN } else { y > 0 };
        if in_bounds {
            let forward = if forward_down {
                index + LEN
            } else {
                index - LEN
            };
            if x > 0 {
                out[index].push(forward - 1);
            }
            out[index].push(forward);
            if x + 1 < LEN {
                out[index].push(forward + 1);
            }
        }
        index += 1;
    }
    out
}

/// Just the two diagonal cells of the forward arc (forward-left,
/// forward-right - straight-forward excluded). Used as the *movement*
/// shape for the diagonal-Knight variant (rules.knights_move_diagonally).
const fn build_diagonal_moves<const LEN: usize, const N: usize>(
    forward_down: bool,
) -> [CellList; N] {
    assert!(N == LEN * LEN, "N must be LEN*LEN");
    let mut out = [CellList::EMPTY; N];
    let mut index = 0;
    while index < N {
        let x = index % LEN;
        let y = index / LEN;
        let in_bounds = if forward_down { y + 1 < LEN } else { y > 0 };
        if in_bounds {
            let forward = if forward_down {
                index + LEN
            } else {
                index - LEN
            };
            if x > 0 {
                out[index].push(forward - 1);
            }
            if x + 1 < LEN {
                out[index].push(forward + 1);
            }
        }
        index += 1;
    }
    out
}

/// The (up to 4) cells exactly two orthogonal tiles away - an Archer's
/// ranged-capture reach.
const fn build_archer_range<const LEN: usize, const N: usize>() -> [CellList; N] {
    assert!(N == LEN * LEN, "N must be LEN*LEN");
    let mut out = [CellList::EMPTY; N];
    let mut index = 0;
    while index < N {
        let x = index % LEN;
        let y = index / LEN;
        if y >= 2 {
            out[index].push(index - 2 * LEN);
        }
        if y + 2 < LEN {
            out[index].push(index + 2 * LEN);
        }
        if x >= 2 {
            out[index].push(index - 2);
        }
        if x + 2 < LEN {
            out[index].push(index + 2);
        }
        index += 1;
    }
    out
}

const fn build_coord<const LEN: usize, const N: usize>() -> [(u8, u8); N] {
    assert!(N == LEN * LEN, "N must be LEN*LEN");
    let mut out = [(0u8, 0u8); N];
    let mut index = 0;
    while index < N {
        out[index] = ((index % LEN) as u8, (index / LEN) as u8);
        index += 1;
    }
    out
}

const fn build_dist<const LEN: usize, const N: usize>() -> [[u8; N]; N] {
    assert!(N == LEN * LEN, "N must be LEN*LEN");
    let mut out = [[0u8; N]; N];
    let mut a = 0;
    while a < N {
        let (ax, ay) = (a % LEN, a / LEN);
        let mut b = 0;
        while b < N {
            let (bx, by) = (b % LEN, b / LEN);
            out[a][b] = (ax.abs_diff(bx) + ay.abs_diff(by)) as u8;
            b += 1;
        }
        a += 1;
    }
    out
}

// Three explicit copies, one per board size, rather than a macro over
// identifiers (which would need an extra proc-macro dependency this
// no_std crate otherwise has no use for) - kept as plain repeated `static`
// items so the const-eval'd tables stay simple statics.
mod mini {
    use super::*;
    pub(crate) static ORTHO: [CellList; MINI_CELL_COUNT] =
        build_ortho::<MINI_LENGTH, MINI_CELL_COUNT>(None);
    pub(crate) static KNIGHT_MOVES: [[CellList; MINI_CELL_COUNT]; 2] = [
        build_ortho::<MINI_LENGTH, MINI_CELL_COUNT>(Some(Skip::Down)),
        build_ortho::<MINI_LENGTH, MINI_CELL_COUNT>(Some(Skip::Up)),
    ];
    pub(crate) static KNIGHT_ARCS: [[CellList; MINI_CELL_COUNT]; 2] = [
        build_arcs::<MINI_LENGTH, MINI_CELL_COUNT>(false),
        build_arcs::<MINI_LENGTH, MINI_CELL_COUNT>(true),
    ];
    pub(crate) static KNIGHT_DIAGONAL_MOVES: [[CellList; MINI_CELL_COUNT]; 2] = [
        build_diagonal_moves::<MINI_LENGTH, MINI_CELL_COUNT>(false),
        build_diagonal_moves::<MINI_LENGTH, MINI_CELL_COUNT>(true),
    ];
    pub(crate) static ARCHER_RANGE: [CellList; MINI_CELL_COUNT] =
        build_archer_range::<MINI_LENGTH, MINI_CELL_COUNT>();
    pub(crate) static COORD: [(u8, u8); MINI_CELL_COUNT] =
        build_coord::<MINI_LENGTH, MINI_CELL_COUNT>();
    pub(crate) static DIST: [[u8; MINI_CELL_COUNT]; MINI_CELL_COUNT] =
        build_dist::<MINI_LENGTH, MINI_CELL_COUNT>();
}

mod normal {
    use super::*;
    pub(crate) static ORTHO: [CellList; NORMAL_CELL_COUNT] =
        build_ortho::<NORMAL_LENGTH, NORMAL_CELL_COUNT>(None);
    pub(crate) static KNIGHT_MOVES: [[CellList; NORMAL_CELL_COUNT]; 2] = [
        build_ortho::<NORMAL_LENGTH, NORMAL_CELL_COUNT>(Some(Skip::Down)),
        build_ortho::<NORMAL_LENGTH, NORMAL_CELL_COUNT>(Some(Skip::Up)),
    ];
    pub(crate) static KNIGHT_ARCS: [[CellList; NORMAL_CELL_COUNT]; 2] = [
        build_arcs::<NORMAL_LENGTH, NORMAL_CELL_COUNT>(false),
        build_arcs::<NORMAL_LENGTH, NORMAL_CELL_COUNT>(true),
    ];
    pub(crate) static KNIGHT_DIAGONAL_MOVES: [[CellList; NORMAL_CELL_COUNT]; 2] = [
        build_diagonal_moves::<NORMAL_LENGTH, NORMAL_CELL_COUNT>(false),
        build_diagonal_moves::<NORMAL_LENGTH, NORMAL_CELL_COUNT>(true),
    ];
    pub(crate) static ARCHER_RANGE: [CellList; NORMAL_CELL_COUNT] =
        build_archer_range::<NORMAL_LENGTH, NORMAL_CELL_COUNT>();
    pub(crate) static COORD: [(u8, u8); NORMAL_CELL_COUNT] =
        build_coord::<NORMAL_LENGTH, NORMAL_CELL_COUNT>();
    pub(crate) static DIST: [[u8; NORMAL_CELL_COUNT]; NORMAL_CELL_COUNT] =
        build_dist::<NORMAL_LENGTH, NORMAL_CELL_COUNT>();
}

mod grand {
    use super::*;
    pub(crate) static ORTHO: [CellList; GRAND_CELL_COUNT] =
        build_ortho::<GRAND_LENGTH, GRAND_CELL_COUNT>(None);
    pub(crate) static KNIGHT_MOVES: [[CellList; GRAND_CELL_COUNT]; 2] = [
        build_ortho::<GRAND_LENGTH, GRAND_CELL_COUNT>(Some(Skip::Down)),
        build_ortho::<GRAND_LENGTH, GRAND_CELL_COUNT>(Some(Skip::Up)),
    ];
    pub(crate) static KNIGHT_ARCS: [[CellList; GRAND_CELL_COUNT]; 2] = [
        build_arcs::<GRAND_LENGTH, GRAND_CELL_COUNT>(false),
        build_arcs::<GRAND_LENGTH, GRAND_CELL_COUNT>(true),
    ];
    pub(crate) static KNIGHT_DIAGONAL_MOVES: [[CellList; GRAND_CELL_COUNT]; 2] = [
        build_diagonal_moves::<GRAND_LENGTH, GRAND_CELL_COUNT>(false),
        build_diagonal_moves::<GRAND_LENGTH, GRAND_CELL_COUNT>(true),
    ];
    pub(crate) static ARCHER_RANGE: [CellList; GRAND_CELL_COUNT] =
        build_archer_range::<GRAND_LENGTH, GRAND_CELL_COUNT>();
    pub(crate) static COORD: [(u8, u8); GRAND_CELL_COUNT] =
        build_coord::<GRAND_LENGTH, GRAND_CELL_COUNT>();
    pub(crate) static DIST: [[u8; GRAND_CELL_COUNT]; GRAND_CELL_COUNT] =
        build_dist::<GRAND_LENGTH, GRAND_CELL_COUNT>();
}

pub(crate) fn cell_count(variant: CrownfallBoardVariant) -> usize {
    match variant {
        CrownfallBoardVariant::Mini => MINI_CELL_COUNT,
        CrownfallBoardVariant::Normal => NORMAL_CELL_COUNT,
        CrownfallBoardVariant::Grand => GRAND_CELL_COUNT,
    }
}

pub(crate) fn board_length(variant: CrownfallBoardVariant) -> usize {
    match variant {
        CrownfallBoardVariant::Mini => MINI_LENGTH,
        CrownfallBoardVariant::Normal => NORMAL_LENGTH,
        CrownfallBoardVariant::Grand => GRAND_LENGTH,
    }
}

pub(crate) fn ortho(variant: CrownfallBoardVariant, index: usize) -> &'static [u8] {
    match variant {
        CrownfallBoardVariant::Mini => mini::ORTHO[index].as_slice(),
        CrownfallBoardVariant::Normal => normal::ORTHO[index].as_slice(),
        CrownfallBoardVariant::Grand => grand::ORTHO[index].as_slice(),
    }
}

pub(crate) fn knight_moves(
    variant: CrownfallBoardVariant,
    player: CrownfallPlayerKind,
    index: usize,
) -> &'static [u8] {
    match variant {
        CrownfallBoardVariant::Mini => mini::KNIGHT_MOVES[player as usize][index].as_slice(),
        CrownfallBoardVariant::Normal => normal::KNIGHT_MOVES[player as usize][index].as_slice(),
        CrownfallBoardVariant::Grand => grand::KNIGHT_MOVES[player as usize][index].as_slice(),
    }
}

pub(crate) fn knight_arcs(
    variant: CrownfallBoardVariant,
    player: CrownfallPlayerKind,
    index: usize,
) -> &'static [u8] {
    match variant {
        CrownfallBoardVariant::Mini => mini::KNIGHT_ARCS[player as usize][index].as_slice(),
        CrownfallBoardVariant::Normal => normal::KNIGHT_ARCS[player as usize][index].as_slice(),
        CrownfallBoardVariant::Grand => grand::KNIGHT_ARCS[player as usize][index].as_slice(),
    }
}

pub(crate) fn knight_diagonal_moves(
    variant: CrownfallBoardVariant,
    player: CrownfallPlayerKind,
    index: usize,
) -> &'static [u8] {
    match variant {
        CrownfallBoardVariant::Mini => {
            mini::KNIGHT_DIAGONAL_MOVES[player as usize][index].as_slice()
        }
        CrownfallBoardVariant::Normal => {
            normal::KNIGHT_DIAGONAL_MOVES[player as usize][index].as_slice()
        }
        CrownfallBoardVariant::Grand => {
            grand::KNIGHT_DIAGONAL_MOVES[player as usize][index].as_slice()
        }
    }
}

pub(crate) fn archer_range(variant: CrownfallBoardVariant, index: usize) -> &'static [u8] {
    match variant {
        CrownfallBoardVariant::Mini => mini::ARCHER_RANGE[index].as_slice(),
        CrownfallBoardVariant::Normal => normal::ARCHER_RANGE[index].as_slice(),
        CrownfallBoardVariant::Grand => grand::ARCHER_RANGE[index].as_slice(),
    }
}

pub(crate) fn coord(variant: CrownfallBoardVariant, index: usize) -> (u8, u8) {
    match variant {
        CrownfallBoardVariant::Mini => mini::COORD[index],
        CrownfallBoardVariant::Normal => normal::COORD[index],
        CrownfallBoardVariant::Grand => grand::COORD[index],
    }
}

// Whole-table accessors for the AI's hot loops (`ai::evaluate`,
// `ai::collect_moves`/`order_moves`, which together dominate search time):
// resolving the board-variant match once per call and indexing the returned
// slice directly beats re-matching it inside every per-piece/per-move lookup
// above. The per-index functions stay for the one-shot call sites.

pub(crate) fn ortho_table(variant: CrownfallBoardVariant) -> &'static [CellList] {
    match variant {
        CrownfallBoardVariant::Mini => &mini::ORTHO,
        CrownfallBoardVariant::Normal => &normal::ORTHO,
        CrownfallBoardVariant::Grand => &grand::ORTHO,
    }
}

pub(crate) fn knight_moves_table(
    variant: CrownfallBoardVariant,
    player: CrownfallPlayerKind,
) -> &'static [CellList] {
    match variant {
        CrownfallBoardVariant::Mini => &mini::KNIGHT_MOVES[player as usize],
        CrownfallBoardVariant::Normal => &normal::KNIGHT_MOVES[player as usize],
        CrownfallBoardVariant::Grand => &grand::KNIGHT_MOVES[player as usize],
    }
}

pub(crate) fn knight_arcs_table(
    variant: CrownfallBoardVariant,
    player: CrownfallPlayerKind,
) -> &'static [CellList] {
    match variant {
        CrownfallBoardVariant::Mini => &mini::KNIGHT_ARCS[player as usize],
        CrownfallBoardVariant::Normal => &normal::KNIGHT_ARCS[player as usize],
        CrownfallBoardVariant::Grand => &grand::KNIGHT_ARCS[player as usize],
    }
}

pub(crate) fn knight_diagonal_moves_table(
    variant: CrownfallBoardVariant,
    player: CrownfallPlayerKind,
) -> &'static [CellList] {
    match variant {
        CrownfallBoardVariant::Mini => &mini::KNIGHT_DIAGONAL_MOVES[player as usize],
        CrownfallBoardVariant::Normal => &normal::KNIGHT_DIAGONAL_MOVES[player as usize],
        CrownfallBoardVariant::Grand => &grand::KNIGHT_DIAGONAL_MOVES[player as usize],
    }
}

pub(crate) fn archer_range_table(variant: CrownfallBoardVariant) -> &'static [CellList] {
    match variant {
        CrownfallBoardVariant::Mini => &mini::ARCHER_RANGE,
        CrownfallBoardVariant::Normal => &normal::ARCHER_RANGE,
        CrownfallBoardVariant::Grand => &grand::ARCHER_RANGE,
    }
}

/// All Manhattan distances from `a` as one row - callers measuring many
/// cells against a fixed anchor (the enemy Crown, in the AI's proximity
/// term and move ordering) fetch the row once and index bytes.
pub(crate) fn dist_row(variant: CrownfallBoardVariant, a: usize) -> &'static [u8] {
    match variant {
        CrownfallBoardVariant::Mini => &mini::DIST[a],
        CrownfallBoardVariant::Normal => &normal::DIST[a],
        CrownfallBoardVariant::Grand => &grand::DIST[a],
    }
}
