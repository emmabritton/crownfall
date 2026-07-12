use game::{BOARD_LENGTH, Cell, Piece, PieceKind, PlayerKind};
use pixels_graphics_lib::prelude::{Coord, Graphics, IndexedImage, coord};

pub const CELL_SIZE: usize = 32;

pub struct PieceRenderer {
    crown_white: IndexedImage,
    crown_black: IndexedImage,
    knight_white: IndexedImage,
    knight_black: IndexedImage,
    spy_white: IndexedImage,
    spy_black: IndexedImage,
}

impl PieceRenderer {
    pub fn new() -> Self {
        Self {
            crown_white: IndexedImage::from_file_contents(include_bytes!(
                "../resources/crown_white.ici"
            ))
            .unwrap()
            .0,
            crown_black: IndexedImage::from_file_contents(include_bytes!(
                "../resources/crown_black.ici"
            ))
            .unwrap()
            .0,
            knight_white: IndexedImage::from_file_contents(include_bytes!(
                "../resources/knight_white.ici"
            ))
            .unwrap()
            .0,
            knight_black: IndexedImage::from_file_contents(include_bytes!(
                "../resources/knight_black.ici"
            ))
            .unwrap()
            .0,
            spy_white: IndexedImage::from_file_contents(include_bytes!(
                "../resources/spy_white.ici"
            ))
            .unwrap()
            .0,
            spy_black: IndexedImage::from_file_contents(include_bytes!(
                "../resources/spy_black.ici"
            ))
            .unwrap()
            .0,
        }
    }

    pub fn image_for_piece(&self, piece: &Piece) -> &IndexedImage {
        match piece.kind {
            PieceKind::Crown => match piece.player {
                PlayerKind::White => &self.crown_white,
                PlayerKind::Black => &self.crown_black,
            },
            PieceKind::Knight => match piece.player {
                PlayerKind::White => &self.knight_white,
                PlayerKind::Black => &self.knight_black,
            },
            PieceKind::Spy => match piece.player {
                PlayerKind::White => &self.spy_white,
                PlayerKind::Black => &self.spy_black,
            },
        }
    }
}

pub struct BoardRenderer {
    dark: IndexedImage,
    light: IndexedImage,
    pos: Coord,
}

impl BoardRenderer {
    pub fn new(pos: Coord) -> Self {
        let sqr_dark =
            IndexedImage::from_file_contents(include_bytes!("../resources/sqr_dark.ici"))
                .unwrap()
                .0;
        let sqr_light =
            IndexedImage::from_file_contents(include_bytes!("../resources/sqr_light.ici"))
                .unwrap()
                .0;
        Self {
            dark: sqr_dark,
            light: sqr_light,
            pos,
        }
    }

    pub fn render(&self, graphics: &mut Graphics) {
        for y in 0..BOARD_LENGTH {
            for x in 0..BOARD_LENGTH {
                let cell_pos =
                    self.pos + Coord::new((x * CELL_SIZE) as isize, (y * CELL_SIZE) as isize);
                let image = if (x + y) % 2 == 0 {
                    &self.light
                } else {
                    &self.dark
                };
                graphics.draw_indexed_image(cell_pos, image);
            }
        }
    }

    pub fn cell_at(&self, xy: Coord) -> Option<Cell> {
        let grid = (xy - self.pos) / CELL_SIZE;
        if (0..BOARD_LENGTH as isize).contains(&grid.x)
            && (0..BOARD_LENGTH as isize).contains(&grid.y)
        {
            Some(Cell::new_coord(grid.x as usize, grid.y as usize))
        } else {
            None
        }
    }

    pub fn pos_for(&self, cell: Cell) -> Coord {
        self.pos + coord!(cell.to_coord()) * CELL_SIZE
    }
}
