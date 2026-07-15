use eb_crownfall_engine::*;

/// The client only offers Standard games for now - no variant-selection UI
/// exists yet, so every board on screen is always this size.
use pixels_graphics_lib::prelude::{Coord, Graphics, IndexedImage, coord};

pub const CELL_SIZE: usize = 32;

pub struct PieceRenderer {
    crown_white: IndexedImage,
    crown_black: IndexedImage,
    knight_white: IndexedImage,
    knight_black: IndexedImage,
    spy_white: IndexedImage,
    spy_black: IndexedImage,
    arrow_white: IndexedImage,
    arrow_black: IndexedImage,
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
            arrow_white: IndexedImage::from_file_contents(include_bytes!(
                "../resources/arrow_white.ici"
            ))
            .unwrap()
            .0,
            arrow_black: IndexedImage::from_file_contents(include_bytes!(
                "../resources/arrow_black.ici"
            ))
            .unwrap()
            .0,
        }
    }

    pub fn image_for_piece(&self, piece: &CrownfallPiece) -> &IndexedImage {
        match piece.kind() {
            CrownfallPieceKind::Crown => match piece.player() {
                CrownfallPlayerKind::White => &self.crown_white,
                CrownfallPlayerKind::Black => &self.crown_black,
            },
            CrownfallPieceKind::Knight => match piece.player() {
                CrownfallPlayerKind::White => &self.knight_white,
                CrownfallPlayerKind::Black => &self.knight_black,
            },
            CrownfallPieceKind::Spy => match piece.player() {
                CrownfallPlayerKind::White => &self.spy_white,
                CrownfallPlayerKind::Black => &self.spy_black,
            },
            CrownfallPieceKind::Archer => match piece.player() {
                CrownfallPlayerKind::White => &self.arrow_white,
                CrownfallPlayerKind::Black => &self.arrow_black,
            },
        }
    }
}

pub struct BoardRenderer {
    dark: IndexedImage,
    light: IndexedImage,
    pos: Coord,
    flipped: bool,
    size: usize,
}

impl BoardRenderer {
    pub fn new(pos: Coord, size: usize) -> Self {
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
            flipped: false,
            size,
        }
    }

    pub fn set_flipped(&mut self, flipped: bool) {
        self.flipped = flipped;
    }

    fn flip(&self, x: usize, y: usize) -> (usize, usize) {
        if self.flipped {
            (self.size - 1 - x, self.size - 1 - y)
        } else {
            (x, y)
        }
    }

    pub fn render(&self, graphics: &mut Graphics) {
        for y in 0..self.size {
            for x in 0..self.size {
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

    pub fn cell_at(&self, xy: Coord, variant: CrownfallBoardVariant) -> Option<CrownfallBoardCell> {
        let grid = (xy - self.pos) / CELL_SIZE;
        if (0..self.size as isize).contains(&grid.x) && (0..self.size as isize).contains(&grid.y) {
            let (x, y) = self.flip(grid.x as usize, grid.y as usize);
            Some(CrownfallBoardCell::new_coord(x, y, variant))
        } else {
            None
        }
    }

    pub fn pos_for(&self, cell: CrownfallBoardCell, variant: CrownfallBoardVariant) -> Coord {
        let (x, y) = cell.to_coord(variant);
        let (x, y) = self.flip(x, y);
        self.pos + coord!(x, y) * CELL_SIZE
    }
}
