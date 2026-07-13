use crate::game_renderer::{BoardRenderer, CELL_SIZE, PieceRenderer};
use crate::{BACKGROUND, SceneName, SceneResult};
use game::ai::{Difficulty, Personality, best_move};
use game::{Cell, Game, GameState, Piece, PlayState, PlayerAction, PlayerKind, TurnResult};
use pixels_graphics_lib::MouseData;
use pixels_graphics_lib::buffer_graphics_lib::Graphics;
use pixels_graphics_lib::prelude::*;
use pixels_graphics_lib::scenes::SceneUpdateResult::Nothing;

const BOARD_POS: Coord = Coord::new(16, 16);
const HUMAN: PlayerKind = PlayerKind::White;
const AI: PlayerKind = PlayerKind::Black;
/// Small pause before the AI "moves" so its turn doesn't feel instantaneous.
const AI_THINK_DELAY: f64 = 0.4;
const MOVE_ANIMATION_DURATION: f64 = 0.25;

struct DragState {
    origin: Cell,
    valid_destinations: Vec<Cell>,
    pointer: Coord,
}

/// Animates the AI's piece sliding from `from` to `to` instead of it snapping
/// straight into place.
struct MoveAnimation {
    piece: Piece,
    from: Cell,
    to: Cell,
    pending: Game,
    elapsed: f64,
}

pub struct AiGameScene {
    game: Game,
    difficulty: Difficulty,
    personality: Personality,
    drag: Option<DragState>,
    ai_timer: Option<Timer>,
    animation: Option<MoveAnimation>,
    piece_renderer: PieceRenderer,
    board_renderer: BoardRenderer,
    highlight_image: IndexedImage,
}

impl AiGameScene {
    pub fn new(difficulty: Difficulty, personality: Personality) -> Box<AiGameScene> {
        Box::new(AiGameScene {
            game: Game::default(),
            difficulty,
            personality,
            drag: None,
            ai_timer: None,
            animation: None,
            piece_renderer: PieceRenderer::new(),
            board_renderer: BoardRenderer::new(BOARD_POS),
            highlight_image: IndexedImage::from_file_contents(include_bytes!(
                "../resources/cell_valid.ici"
            ))
            .unwrap()
            .0,
        })
    }

    /// Begin animating the AI's move rather than applying `next` immediately.
    fn animate_or_apply(&mut self, next: Game, turn_result: Option<TurnResult>) {
        let move_cells = turn_result.as_ref().and_then(move_cells);
        if let Some((from, to)) = move_cells
            && let Some(piece) = self.game.board.cells[from.index]
        {
            self.animation = Some(MoveAnimation {
                piece,
                from,
                to,
                pending: next,
                elapsed: 0.0,
            });
        } else {
            self.game = next;
        }
    }
}

/// Extracts the (from, to) cells of a move from a turn result, if it
/// represents a piece moving on the board.
fn move_cells(result: &TurnResult) -> Option<(Cell, Cell)> {
    match result {
        TurnResult::PieceMove { from, to, .. } => Some((*from, *to)),
        TurnResult::Capture {
            last_move_from,
            last_move_to,
            ..
        } => Some((*last_move_from, *last_move_to)),
        TurnResult::Victory { .. } => None,
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
            if self.animation.as_ref().is_some_and(|a| a.from.index == i) {
                continue;
            }
            if let Some(cell) = cell {
                let xy = self.board_renderer.pos_for(Cell::new_index(i));
                let image = self.piece_renderer.image_for_piece(cell);
                graphics.draw_indexed_image(xy, image);
            }
        }
        if let Some(anim) = &self.animation {
            let t = (anim.elapsed / MOVE_ANIMATION_DURATION).clamp(0.0, 1.0);
            let from = self.board_renderer.pos_for(anim.from);
            let to = self.board_renderer.pos_for(anim.to);
            let xy = from + (to - from) * t;
            let image = self.piece_renderer.image_for_piece(&anim.piece);
            graphics.draw_indexed_image(xy, image);
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
        if let Some(anim) = &mut self.animation {
            anim.elapsed += timing.fixed_time_step;
            if anim.elapsed >= MOVE_ANIMATION_DURATION {
                let anim = self.animation.take().unwrap();
                self.game = anim.pending;
            }
        }
        if let Some(timer) = &mut self.ai_timer
            && timer.update(timing)
        {
            self.ai_timer = None;
            if is_ais_turn(&self.game)
                && let Some(action) =
                    best_move(&self.game, AI, self.difficulty.depth(), self.personality)
                && let Ok((next, turn_result)) = self.game.clone().handle_player_action(action)
            {
                self.animate_or_apply(next, turn_result);
            }
        }
        Nothing
    }

    fn resuming(&mut self, _: Option<SceneResult>) {
        self.game = Game::default();
        self.drag = None;
        self.ai_timer = None;
        self.animation = None;
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
        &format!("{} turns remaining", game.turns_remaining()),
        TextPos::px(pos + (0, 40)),
        (WHITE, PixelFont::Standard6x7),
    );

    graphics.draw_text(
        &state_to_text(&game.state),
        TextPos::px(pos + (0, 60)),
        (WHITE, PixelFont::Standard6x7, WrappingStrategy::AtCol(18)),
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
        GameState::Draw(reason) => format!("Draw ({})", reason.description()),
    }
}
