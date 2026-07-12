use crate::game_renderer::{BoardRenderer, CELL_SIZE, PieceRenderer};
use crate::{BACKGROUND, SceneName, SceneResult};
use game::ai::{DEFAULT_DEPTH, best_move};
use game::{Cell, Game, GameState, PlayState, PlayerAction, PlayerKind};
use pixels_graphics_lib::MouseData;
use pixels_graphics_lib::buffer_graphics_lib::Graphics;
use pixels_graphics_lib::prelude::*;
use pixels_graphics_lib::scenes::SceneUpdateResult::Nothing;

const BOARD_POS: Coord = Coord::new(16, 16);
const HUMAN: PlayerKind = PlayerKind::White;
const AI: PlayerKind = PlayerKind::Black;
/// Small pause before the AI "moves" so its turn doesn't feel instantaneous.
const AI_THINK_DELAY: f64 = 0.4;

struct DragState {
    origin: Cell,
    valid_destinations: Vec<Cell>,
    pointer: Coord,
}

pub struct AiGameScene {
    game: Game,
    drag: Option<DragState>,
    ai_timer: Option<Timer>,
    piece_renderer: PieceRenderer,
    board_renderer: BoardRenderer,
    highlight_image: IndexedImage,
}

impl AiGameScene {
    pub fn new() -> Box<AiGameScene> {
        Box::new(AiGameScene {
            game: Game::default(),
            drag: None,
            ai_timer: None,
            piece_renderer: PieceRenderer::new(),
            board_renderer: BoardRenderer::new(BOARD_POS),
            highlight_image: IndexedImage::from_file_contents(include_bytes!(
                "../resources/cell_valid.ici"
            ))
            .unwrap()
            .0,
        })
    }
}

fn is_humans_turn(game: &Game) -> bool {
    matches!(&game.state, GameState::Playing(state) if state.player() == HUMAN)
}

fn is_ais_turn(game: &Game) -> bool {
    matches!(&game.state, GameState::Playing(state) if state.player() == AI)
}

impl Scene<SceneResult, SceneName> for AiGameScene {
    fn render(&self, graphics: &mut Graphics, _: &MouseData, _: &FxHashSet<KeyCode>) {
        graphics.clear(BACKGROUND);
        self.board_renderer.render(graphics);
        for (i, cell) in self.game.board.cells.iter().enumerate() {
            if self.drag.as_ref().is_some_and(|d| d.origin.index == i) {
                continue;
            }
            if let Some(cell) = cell {
                let xy = self.board_renderer.pos_for(Cell::new_index(i));
                let image = self.piece_renderer.image_for_piece(cell);
                graphics.draw_indexed_image(xy, image);
            }
        }
        draw_status(&self.game, graphics);
        if let Some(drag) = &self.drag {
            for destination in &drag.valid_destinations {
                let pos = self.board_renderer.pos_for(*destination);
                graphics.draw_indexed_image(pos, &self.highlight_image);
            }
            if let Some(piece) = self.game.board.cells[drag.origin.index] {
                let image = self.piece_renderer.image_for_piece(&piece);
                let half_cell = (CELL_SIZE / 2) as isize;
                let xy = drag.pointer - Coord::new(half_cell, half_cell);
                graphics.draw_indexed_image(xy, image);
            }
        }
    }

    fn on_mouse_down(
        &mut self,
        mouse: &MouseData,
        mouse_button: MouseButton,
        _: &FxHashSet<KeyCode>,
    ) {
        if mouse_button != MouseButton::Left {
            return;
        }
        if !is_humans_turn(&self.game) {
            return;
        }
        let Some(cell) = self.board_renderer.cell_at(mouse.xy) else {
            return;
        };
        if let Some(piece) = self.game.board.cells[cell.index]
            && piece.player == HUMAN
        {
            self.drag = Some(DragState {
                origin: cell,
                valid_destinations: self.game.board.get_valid_destinations_for(cell),
                pointer: mouse.xy,
            });
        }
    }

    fn on_mouse_drag(&mut self, mouse: &MouseData, _: &FxHashSet<KeyCode>) {
        if let Some(drag) = &mut self.drag {
            drag.pointer = mouse.xy;
        }
    }

    fn on_mouse_up(
        &mut self,
        mouse: &MouseData,
        mouse_button: MouseButton,
        _: &FxHashSet<KeyCode>,
    ) {
        if mouse_button != MouseButton::Left {
            return;
        }
        let Some(drag) = self.drag.take() else {
            return;
        };
        let Some(target) = self.board_renderer.cell_at(mouse.xy) else {
            return;
        };
        if target == drag.origin || !drag.valid_destinations.contains(&target) {
            return;
        }
        let action = PlayerAction::Move {
            player: HUMAN,
            from: drag.origin,
            to: target,
        };
        if let Ok((next, _)) = self.game.clone().handle_player_action(action) {
            self.game = next;
            if is_ais_turn(&self.game) {
                self.ai_timer = Some(Timer::new_once(AI_THINK_DELAY));
            }
        }
    }

    fn update(
        &mut self,
        timing: &Timing,
        _: &MouseData,
        _: &FxHashSet<KeyCode>,
        _: &Window,
    ) -> SceneUpdateResult<SceneResult, SceneName> {
        if let Some(timer) = &mut self.ai_timer
            && timer.update(timing)
        {
            self.ai_timer = None;
            if is_ais_turn(&self.game)
                && let Some(action) = best_move(&self.game, AI, DEFAULT_DEPTH)
                && let Ok((next, _)) = self.game.clone().handle_player_action(action)
            {
                self.game = next;
            }
        }
        Nothing
    }

    fn resuming(&mut self, _: Option<SceneResult>) {
        self.game = Game::default();
        self.drag = None;
        self.ai_timer = None;
    }
}

fn draw_status(game: &Game, graphics: &mut Graphics) {
    let pos = coord!(260, 16);
    graphics.draw_text(
        "White: You",
        TextPos::px(pos),
        (WHITE, PixelFont::Standard6x7),
    );
    graphics.draw_text(
        "Black: Computer",
        TextPos::px(pos + (0, 10)),
        (WHITE, PixelFont::Standard6x7),
    );
    graphics.draw_text(
        &state_to_text(&game.state),
        TextPos::px(pos + (0, 40)),
        (WHITE, PixelFont::Standard6x7),
    );
}

fn state_to_text(state: &GameState) -> String {
    match state {
        GameState::Playing(state) => match state {
            PlayState::WaitingForInput { player } => {
                let name = if player == &HUMAN {
                    "you"
                } else {
                    "the computer"
                };
                format!("Waiting for {name}")
            }
            PlayState::MustRemoveKnight { player, options } => {
                let name = if player == &HUMAN {
                    "You"
                } else {
                    "The computer"
                };
                format!(
                    "{name} must remove knight at either {:?} or {:?}",
                    options.0.to_coord(),
                    options.1.to_coord()
                )
            }
        },
        GameState::Victory(player) => {
            let name = if player == &HUMAN {
                "You win!"
            } else {
                "Computer wins"
            };
            name.to_string()
        }
    }
}
