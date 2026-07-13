//! Compile-time lookup tables for the 7x7 board, stored in ROM. The board is
//! small enough that every neighbour relationship can be precomputed, so the
//! hot paths (AI search, capture checks) do plain table lookups instead of
//! coordinate math — no division/modulo, no bounds branches, and no heap
//! allocation for neighbour lists on the ARM7TDMI.
use crate::BOARD_LENGTH;

pub(crate) const CELL_COUNT: usize = BOARD_LENGTH * BOARD_LENGTH;

/// A fixed neighbour list (a cell has at most 4 orthogonal neighbours).
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

/// Orthogonal neighbours of every cell, in up/down/left/right order (the
/// same order `orthogonal_neighbours` produced before it became a table).
pub(crate) static ORTHO: [CellList; CELL_COUNT] = build_ortho(None);

/// Legal Knight move destinations per cell: orthogonal neighbours minus the
/// backward one. Indexed by `player as usize` (White = 0, Black = 1); White
/// advances toward y=0 so its backward neighbour is down (+y), Black's is up.
pub(crate) static KNIGHT_MOVES: [[CellList; CELL_COUNT]; 2] = [
    build_ortho(Some(Skip::Down)),
    build_ortho(Some(Skip::Up)),
];

/// A Knight's forward capture arc per cell: the (up to) 3 cells in the row
/// ahead of it — forward-left, straight forward, forward-right. Indexed by
/// `player as usize` (White = 0, Black = 1).
pub(crate) static KNIGHT_ARCS: [[CellList; CELL_COUNT]; 2] =
    [build_arcs(false), build_arcs(true)];

/// Manhattan distance between every pair of cells, indexed `[a][b]`.
pub(crate) static DIST: [[u8; CELL_COUNT]; CELL_COUNT] = build_dist();

/// `(x, y)` of every cell index, so `Cell::to_coord` is a lookup instead of
/// `%`/`/` by 7 — the ARM7TDMI has no divide instruction, and Thumb-1 can't
/// even use the usual multiply-by-magic-constant lowering.
pub(crate) static COORD: [(u8, u8); CELL_COUNT] = build_coord();

enum Skip {
    Up,
    Down,
}

const fn build_ortho(skip: Option<Skip>) -> [CellList; CELL_COUNT] {
    let mut out = [CellList::EMPTY; CELL_COUNT];
    let mut index = 0;
    while index < CELL_COUNT {
        let x = index % BOARD_LENGTH;
        let y = index / BOARD_LENGTH;
        if y > 0 && !matches!(skip, Some(Skip::Up)) {
            out[index].push(index - BOARD_LENGTH);
        }
        if y + 1 < BOARD_LENGTH && !matches!(skip, Some(Skip::Down)) {
            out[index].push(index + BOARD_LENGTH);
        }
        if x > 0 {
            out[index].push(index - 1);
        }
        if x + 1 < BOARD_LENGTH {
            out[index].push(index + 1);
        }
        index += 1;
    }
    out
}

/// `forward_down` is true for Black (advances toward y=6), false for White.
const fn build_arcs(forward_down: bool) -> [CellList; CELL_COUNT] {
    let mut out = [CellList::EMPTY; CELL_COUNT];
    let mut index = 0;
    while index < CELL_COUNT {
        let x = index % BOARD_LENGTH;
        let y = index / BOARD_LENGTH;
        let in_bounds = if forward_down {
            y + 1 < BOARD_LENGTH
        } else {
            y > 0
        };
        if in_bounds {
            let forward = if forward_down {
                index + BOARD_LENGTH
            } else {
                index - BOARD_LENGTH
            };
            if x > 0 {
                out[index].push(forward - 1);
            }
            out[index].push(forward);
            if x + 1 < BOARD_LENGTH {
                out[index].push(forward + 1);
            }
        }
        index += 1;
    }
    out
}

const fn build_coord() -> [(u8, u8); CELL_COUNT] {
    let mut out = [(0u8, 0u8); CELL_COUNT];
    let mut index = 0;
    while index < CELL_COUNT {
        out[index] = ((index % BOARD_LENGTH) as u8, (index / BOARD_LENGTH) as u8);
        index += 1;
    }
    out
}

const fn build_dist() -> [[u8; CELL_COUNT]; CELL_COUNT] {
    let mut out = [[0u8; CELL_COUNT]; CELL_COUNT];
    let mut a = 0;
    while a < CELL_COUNT {
        let (ax, ay) = (a % BOARD_LENGTH, a / BOARD_LENGTH);
        let mut b = 0;
        while b < CELL_COUNT {
            let (bx, by) = (b % BOARD_LENGTH, b / BOARD_LENGTH);
            out[a][b] = (ax.abs_diff(bx) + ay.abs_diff(by)) as u8;
            b += 1;
        }
        a += 1;
    }
    out
}
