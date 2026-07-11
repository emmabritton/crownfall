pub enum PieceKind {
    Crown,
    Knight,
    Spy
}

pub const BOARD_SIZE: (usize, usize) = (7, 7);

pub enum PlayerKind {
    White,
    Black
}

pub struct Piece {
    kind: PieceKind,
    player: PlayerKind
}

pub struct BoardState {
    cells: [Piece; BOARD_SIZE.0 * BOARD_SIZE.1],
}

