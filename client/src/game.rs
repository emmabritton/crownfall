use std::env::var;
use crate::game_renderer::{BoardRenderer, CELL_SIZE, PieceRenderer};
use crate::net::{poll, send};
use crate::{BACKGROUND, HEIGHT, SceneName, SceneResult, WIDTH, username};
use eb_crownfall_engine::{
    CrownfallBoardCell, CrownfallBoardVariant, CrownfallGameState, CrownfallPiece,
    CrownfallPlayState, CrownfallPlayerAction, CrownfallPlayerKind, CrownfallTurnResult,
};
use networking::models::WebGame;
use networking::packet::{GameId, NetGameState, Packet, PerformActionState};
use pixels_graphics_lib::MouseData;
use pixels_graphics_lib::buffer_graphics_lib::Graphics;
use pixels_graphics_lib::prelude::*;
use pixels_graphics_lib::scenes::SceneUpdateResult::Nothing;

const BOARD_POS: Coord = Coord::new(12, 12);
const MOVE_ANIMATION_DURATION: f64 = 0.25;

#[allow(clippy::large_enum_variant)]
enum GameClientState {
    PreLoad(GameId),
    Loading,
    Playing(WebGame, bool), //true if is_white
    Error(String),
}

struct DragState {
    origin: CrownfallBoardCell,
    valid_destinations: Vec<CrownfallBoardCell>,
    pointer: Coord,
}

/// Animates a piece sliding from `from` to `to` after an update from the other
/// player arrives, rather than snapping straight to the new board state.
struct MoveAnimation {
    piece: CrownfallPiece,
    from: CrownfallBoardCell,
    to: CrownfallBoardCell,
    pending: WebGame,
    is_white: bool,
    elapsed: f64,
}

pub struct GameScene {
    state: GameClientState,
    piece_renderer: PieceRenderer,
    board_renderer: BoardRenderer,
    drag: Option<DragState>,
    animation: Option<MoveAnimation>,
    highlight_image: IndexedImage,
    last_move: Option<CrownfallTurnResult>,
}

impl GameScene {
    pub fn new(id: String, board_length: usize) -> Box<GameScene> {
        Box::new(GameScene {
            state: GameClientState::PreLoad(id),
            last_move: None,
            piece_renderer: PieceRenderer::new(),
            board_renderer: BoardRenderer::new(BOARD_POS, board_length),
            drag: None,
            animation: None,
            highlight_image: IndexedImage::from_file_contents(include_bytes!(
                "../resources/cell_valid.ici"
            ))
            .unwrap()
            .0,
        })
    }

    /// Begin animating `game`'s incoming update if it was the other player moving
    /// a piece; otherwise apply it immediately.
    fn apply_update(
        &mut self,
        game: WebGame,
        is_white: bool,
        turn_result: Option<&CrownfallTurnResult>,
    ) {
        self.board_renderer.set_flipped(!is_white);
        // A new update arrived mid-animation; snap to the previous target first.
        if let Some(anim) = self.animation.take() {
            self.state = GameClientState::Playing(anim.pending, anim.is_white);
        }
        let moved_by_other = turn_result.and_then(move_cells).filter(|(player, _, _)| {
            let mover_is_white = *player == CrownfallPlayerKind::White;
            mover_is_white != is_white
        });
        if let Some((_, from, to)) = moved_by_other
            && let GameClientState::Playing(current, _) = &self.state
            && let Some(piece) = current.game.board.cells()[from.to_index()]
        {
            self.animation = Some(MoveAnimation {
                piece,
                from,
                to,
                pending: game,
                is_white,
                elapsed: 0.0,
            });
        } else {
            self.state = GameClientState::Playing(game, is_white);
        }
    }
}

/// Extracts the (player, from, to) cells of a move from a turn result, if it
/// represents a piece moving on the board.
fn move_cells(
    result: &CrownfallTurnResult,
) -> Option<(CrownfallPlayerKind, CrownfallBoardCell, CrownfallBoardCell)> {
    match result {
        CrownfallTurnResult::PieceMove { player, from, to } => Some((*player, *from, *to)),
        CrownfallTurnResult::Capture {
            player,
            last_move_from,
            last_move_to,
            ..
        } => Some((*player, *last_move_from, *last_move_to)),
        CrownfallTurnResult::Victory { .. } => None,
    }
}

fn is_players_turn(play_state: &CrownfallPlayState, is_white: bool) -> bool {
    (play_state.player() == CrownfallPlayerKind::White && is_white)
        || (play_state.player() == CrownfallPlayerKind::Black && !is_white)
}

impl Scene<SceneResult, SceneName> for GameScene {
    fn render(&self, graphics: &mut Graphics, _: &MouseData, _: &FxHashSet<KeyCode>) {
        graphics.clear(BACKGROUND);
        match &self.state {
            GameClientState::Playing(web_game, is_white) => {
                self.board_renderer.render(graphics);
                for (i, cell) in web_game.game.board.cells().iter().enumerate() {
                    if self.drag.as_ref().is_some_and(|d| d.origin.to_index() == i) {
                        continue;
                    }
                    if self.animation.as_ref().is_some_and(|a| a.from.to_index() == i) {
                        continue;
                    }
                    if let Some(cell) = cell {
                        let xy = self
                            .board_renderer
                            .pos_for(CrownfallBoardCell::new_index(i), web_game.game.board.variant());
                        let image = self.piece_renderer.image_for_piece(cell);
                        graphics.draw_indexed_image(xy, image);
                    }
                }
                if let Some(anim) = &self.animation {
                    let t = (anim.elapsed / MOVE_ANIMATION_DURATION).clamp(0.0, 1.0);
                    let from = self.board_renderer.pos_for(anim.from, web_game.game.board.variant());
                    let to = self.board_renderer.pos_for(anim.to, web_game.game.board.variant());
                    let xy = from + (to - from) * t;
                    let image = self.piece_renderer.image_for_piece(&anim.piece);
                    graphics.draw_indexed_image(xy, image);
                }
                draw_status(web_game, *is_white, graphics, &self.last_move);
                if let Some(drag) = &self.drag {
                    for destination in &drag.valid_destinations {
                        let pos = self.board_renderer.pos_for(*destination, web_game.game.board.variant());
                        graphics.draw_indexed_image(pos, &self.highlight_image);
                    }
                    if let Some(piece) = web_game.game.board.cells()[drag.origin.to_index()] {
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
        let CrownfallGameState::Playing(play_state) = &web_game.game.state else {
            return;
        };
        if !is_players_turn(play_state, *is_white) {
            return;
        }
        let Some(cell) = self.board_renderer.cell_at(mouse.xy, web_game.game.board.variant()) else {
            return;
        };
        if let Some(piece) = web_game.game.board.cells()[cell.to_index()]
            && piece.player() == play_state.player()
        {
            self.drag = Some(DragState {
                origin: cell,
                valid_destinations: web_game
                    .game
                    .board
                    .get_valid_destinations_for(cell, web_game.game.rules),
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
        let GameClientState::Playing(web_game, _) = &mut self.state else {
            return;
        };
        let Some(target) = self.board_renderer.cell_at(mouse.xy, web_game.game.board.variant()) else {
            return;
        };
        let CrownfallGameState::Playing(play_state) = &web_game.game.state else {
            return;
        };
        let player = play_state.player();
        if target != drag.origin && drag.valid_destinations.contains(&target) {
            match send(Packet::PerformActionRequest(
                web_game.id.clone(),
                CrownfallPlayerAction::Move {
                    player,
                    from: drag.origin,
                    to: target,
                },
            )) {
                // Optimistically apply the move locally so the piece doesn't snap
                // back to its origin while waiting for the server's confirmation.
                Ok(()) => {
                    let moved = web_game.game.board.cells_mut()[drag.origin.to_index()].take();
                    web_game.game.board.cells_mut()[target.to_index()] = moved;
                }
                Err(e) => self.state = GameClientState::Error(e.to_string()),
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
                self.state = GameClientState::Playing(anim.pending, anim.is_white);
            }
        }
        match &self.state {
            GameClientState::Playing(_, _) => match poll() {
                Ok(packets) => {
                    for packet in packets {
                        match packet {
                            Packet::GameUpdateCommand(game, turn_result) => {
                                let is_white = game.white_player_name
                                    == username().expect("must have username");
                                self.apply_update(game, is_white, turn_result.as_ref());
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
    _last_move: &Option<CrownfallTurnResult>,
) {
    let x = match web_game.game.board.variant() {
        CrownfallBoardVariant::Mini => 172,
        CrownfallBoardVariant::Normal => 236,
        CrownfallBoardVariant::Grand => 300,
    };
    let pos = coord!(x, 6);
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
        &format!("{} turns remaining", web_game.game.turns_remaining()),
        TextPos::px(pos + (0, 40)),
        (WHITE, PixelFont::Standard6x7),
    );

    graphics.draw_text(
        &state_to_text(
            web_game.game.board.variant(),
            &web_game.game.state,
            &web_game.white_player_name,
            &web_game.black_player_name,
        ),
        TextPos::px(pos + (0, 60)),
        (WHITE, PixelFont::Standard6x7, WrappingStrategy::AtCol(18)),
    );
    // if let Some(result) = last_move {
    //     graphics.draw_text(
    //         &last_move_text(result),
    //         TextPos::px(pos + (0, 52)),
    //         (WHITE, PixelFont::Standard6x7),
    //     );
    // }
}

fn state_to_text(variant: CrownfallBoardVariant,state: &CrownfallGameState, white_name: &str, black_name: &str) -> String {
    match state {
        CrownfallGameState::Playing(state) => match state {
            CrownfallPlayState::WaitingForInput { player } => {
                let name = if player == &CrownfallPlayerKind::White {
                    white_name
                } else {
                    black_name
                };
                format!("Waiting for {name}")
            }
            CrownfallPlayState::MustRemoveKnight { player, options } => {
                let name = if player == &CrownfallPlayerKind::White {
                    white_name
                } else {
                    black_name
                };
                format!(
                    "{name} must remove knight at either {:?} or {:?}",
                    options.0.to_coord(variant),
                    options.1.to_coord(variant)
                )
            }
        },
        CrownfallGameState::Victory(player) => {
            let name = if player == &CrownfallPlayerKind::White {
                white_name
            } else {
                black_name
            };
            format!("Victory: {name}")
        }
        CrownfallGameState::Draw(reason) => format!("Draw ({})", reason.description()),
    }
}
