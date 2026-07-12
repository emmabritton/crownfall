use crate::game_renderer::{BoardRenderer, CELL_SIZE, PieceRenderer};
use crate::net::{poll, send};
use crate::{BACKGROUND, HEIGHT, SceneName, SceneResult, WIDTH, username};
use game::{Cell, GameState, PlayState, PlayerAction, PlayerKind, TurnResult};
use networking::models::WebGame;
use networking::packet::{GameId, NetGameState, Packet, PerformActionState};
use pixels_graphics_lib::MouseData;
use pixels_graphics_lib::buffer_graphics_lib::Graphics;
use pixels_graphics_lib::prelude::*;
use pixels_graphics_lib::scenes::SceneUpdateResult::Nothing;

const BOARD_POS: Coord = Coord::new(16, 16);

enum GameClientState {
    PreLoad(GameId),
    Loading,
    Playing(WebGame, bool), //true if is_white
    Error(String),
}

struct DragState {
    origin: Cell,
    valid_destinations: Vec<Cell>,
    pointer: Coord,
}

pub struct GameScene {
    state: GameClientState,
    piece_renderer: PieceRenderer,
    board_renderer: BoardRenderer,
    drag: Option<DragState>,
    highlight_image: IndexedImage,
    last_move: Option<TurnResult>,
}

impl GameScene {
    pub fn new(id: String) -> Box<GameScene> {
        Box::new(GameScene {
            state: GameClientState::PreLoad(id),
            last_move: None,
            piece_renderer: PieceRenderer::new(),
            board_renderer: BoardRenderer::new(BOARD_POS),
            drag: None,
            highlight_image: IndexedImage::from_file_contents(include_bytes!(
                "../resources/cell_valid.ici"
            ))
            .unwrap()
            .0,
        })
    }
}

fn is_players_turn(play_state: &PlayState, is_white: bool) -> bool {
    (play_state.player() == PlayerKind::White && is_white)
        || (play_state.player() == PlayerKind::Black && !is_white)
}

impl Scene<SceneResult, SceneName> for GameScene {
    fn render(&self, graphics: &mut Graphics, _: &MouseData, _: &FxHashSet<KeyCode>) {
        graphics.clear(BACKGROUND);
        match &self.state {
            GameClientState::Playing(web_game, is_white) => {
                self.board_renderer.render(graphics);
                for (i, cell) in web_game.game.board.cells.iter().enumerate() {
                    if self.drag.as_ref().is_some_and(|d| d.origin.index == i) {
                        continue;
                    }
                    if let Some(cell) = cell {
                        let xy = self.board_renderer.pos_for(Cell::new_index(i));
                        let image = self.piece_renderer.image_for_piece(cell);
                        graphics.draw_indexed_image(xy, image);
                    }
                }
                draw_status(web_game, *is_white, graphics, &self.last_move);
                if let Some(drag) = &self.drag {
                    for destination in &drag.valid_destinations {
                        let pos = self.board_renderer.pos_for(*destination);
                        graphics.draw_indexed_image(pos, &self.highlight_image);
                    }
                    if let Some(piece) = web_game.game.board.cells[drag.origin.index] {
                        let image = self.piece_renderer.image_for_piece(&piece);
                        let half_cell = (CELL_SIZE / 2) as isize;
                        let xy = drag.pointer - Coord::new(half_cell, half_cell);
                        graphics.draw_indexed_image(xy, image);
                    }
                }
            }
            GameClientState::Error(err) => {
                graphics.clear(BLACK);
                graphics.draw_text(
                    &format!("Error: {err}\nPlease restart client"),
                    TextPos::px(coord!(WIDTH / 2, HEIGHT / 2)),
                    (
                        RED,
                        PixelFont::Standard6x7,
                        WrappingStrategy::AtCol(60),
                        Positioning::Center,
                    ),
                );
            }
            GameClientState::PreLoad(_) | GameClientState::Loading => {
                graphics.draw_text(
                    "Loading...",
                    TextPos::px(coord!(WIDTH / 2, HEIGHT / 2)),
                    (
                        WHITE,
                        PixelFont::Standard6x7,
                        WrappingStrategy::AtCol(60),
                        Positioning::Center,
                    ),
                );
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
        let GameClientState::Playing(web_game, is_white) = &self.state else {
            return;
        };
        let GameState::Playing(play_state) = &web_game.game.state else {
            return;
        };
        if !is_players_turn(play_state, *is_white) {
            return;
        }
        let Some(cell) = self.board_renderer.cell_at(mouse.xy) else {
            return;
        };
        if let Some(piece) = web_game.game.board.cells[cell.index]
            && piece.player == play_state.player()
        {
            self.drag = Some(DragState {
                origin: cell,
                valid_destinations: web_game.game.board.get_valid_destinations_for(cell),
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
        let GameClientState::Playing(web_game, _) = &mut self.state else {
            return;
        };
        let GameState::Playing(play_state) = &web_game.game.state else {
            return;
        };
        let player = play_state.player();
        if target != drag.origin && drag.valid_destinations.contains(&target) {
            match send(Packet::PerformActionRequest(
                web_game.id.clone(),
                PlayerAction::Move {
                    player,
                    from: drag.origin,
                    to: target,
                },
            )) {
                // Optimistically apply the move locally so the piece doesn't snap
                // back to its origin while waiting for the server's confirmation.
                Ok(()) => {
                    web_game.game.board.cells[target.index] =
                        web_game.game.board.cells[drag.origin.index].take();
                }
                Err(e) => self.state = GameClientState::Error(e.to_string()),
            }
        }
    }

    fn update(
        &mut self,
        _: &Timing,
        _: &MouseData,
        _: &FxHashSet<KeyCode>,
        _: &Window,
    ) -> SceneUpdateResult<SceneResult, SceneName> {
        match &self.state {
            GameClientState::Playing(_, _) => match poll() {
                Ok(packets) => {
                    for packet in packets {
                        match packet {
                            Packet::GameUpdateCommand(game, _) => {
                                let is_white = game.white_player_name
                                    == username().expect("must have username");
                                self.board_renderer.set_flipped(!is_white);
                                self.state = GameClientState::Playing(game, is_white);
                            }
                            Packet::PerformActionResponse(state) => match state {
                                PerformActionState::Done => {}
                                PerformActionState::NotYourTurn => {}
                                PerformActionState::InvalidGame => {}
                            },
                            _ => {}
                        }
                    }
                }
                Err(e) => self.state = GameClientState::Error(format!("{e}")),
            },
            GameClientState::Error(_) => {}
            GameClientState::Loading => match poll() {
                Ok(packets) => {
                    for packet in packets {
                        if let Packet::PollGameResponse(state) = packet {
                            match state {
                                NetGameState::Active(game) => {
                                    let is_white = game.white_player_name
                                        == username().expect("must have username");
                                    self.board_renderer.set_flipped(!is_white);
                                    self.state = GameClientState::Playing(game, is_white);
                                }
                                NetGameState::InvalidGame => {
                                    self.state = GameClientState::Error("Invalid game".to_string())
                                }
                            }
                        }
                    }
                }
                Err(e) => self.state = GameClientState::Error(format!("{e}")),
            },
            GameClientState::PreLoad(id) => {
                if let Err(e) = send(Packet::PollGameRequest(id.clone())) {
                    self.state = GameClientState::Error(format!("{e}"));
                } else {
                    self.state = GameClientState::Loading;
                }
            }
        }
        Nothing
    }
}

fn draw_status(
    web_game: &WebGame,
    is_white: bool,
    graphics: &mut Graphics,
    _last_move: &Option<TurnResult>,
) {
    let pos = coord!(260, 16);
    graphics.draw_text(
        &format!(
            "White: {} {}",
            web_game.white_player_name,
            if is_white { "(you)" } else { "" }
        ),
        TextPos::px(pos),
        (WHITE, PixelFont::Standard6x7),
    );
    graphics.draw_text(
        &format!(
            "Black: {} {}",
            web_game.black_player_name,
            if !is_white { "(you)" } else { "" }
        ),
        TextPos::px(pos + (0, 10)),
        (WHITE, PixelFont::Standard6x7),
    );
    graphics.draw_text(
        &state_to_text(
            &web_game.game.state,
            &web_game.white_player_name,
            &web_game.black_player_name,
        ),
        TextPos::px(pos + (0, 40)),
        (WHITE, PixelFont::Standard6x7),
    );
    // if let Some(result) = last_move {
    //     graphics.draw_text(
    //         &last_move_text(result),
    //         TextPos::px(pos + (0, 52)),
    //         (WHITE, PixelFont::Standard6x7),
    //     );
    // }
}

fn state_to_text(state: &GameState, white_name: &str, black_name: &str) -> String {
    match state {
        GameState::Playing(state) => match state {
            PlayState::WaitingForInput { player } => {
                let name = if player == &PlayerKind::White {
                    white_name
                } else {
                    black_name
                };
                format!("Waiting for {name}")
            }
            PlayState::MustRemoveKnight { player, options } => {
                let name = if player == &PlayerKind::White {
                    white_name
                } else {
                    black_name
                };
                format!(
                    "{name} must remove knight at either {:?} or {:?}",
                    options.0.to_coord(),
                    options.1.to_coord()
                )
            }
        },
        GameState::Victory(player) => {
            let name = if player == &PlayerKind::White {
                white_name
            } else {
                black_name
            };
            format!("Victory: {name}")
        }
    }
}
