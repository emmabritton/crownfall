use std::fmt::format;
use crate::game_renderer::{BoardRenderer, CELL_SIZE, PieceRenderer};
use crate::{DOMAIN, HEIGHT, SceneName, SceneResult, WIDTH};
use common::{GamePollResponse, WebGame, URL_PLAY, PerformActionRequest, PerformActionResponse};
use game::{Cell, GameState, PlayState, PlayerAction, PlayerKind, TurnResult};
use pixels_graphics_lib::MouseData;
use pixels_graphics_lib::buffer_graphics_lib::Graphics;
use pixels_graphics_lib::prelude::{coord, Coord, FxHashSet, KeyCode, MouseButton, PixelFont, Positioning, Scene, SceneUpdateResult, TextPos, Timer, Timing, Window, WrappingStrategy, BLACK, RED, WHITE, IndexedImage};
use pixels_graphics_lib::scenes::SceneUpdateResult::Nothing;
use reqwest::blocking::Client;

const BOARD_POS: Coord = Coord::new(16, 16);

enum GameClientState {
    Playing(WebGame),
    Error(String),
}

pub struct GameScene {
    client: Client,
    state: GameClientState,
    is_white: bool,
    piece_renderer: PieceRenderer,
    board_renderer: BoardRenderer,
    timer: Timer,
    highlight: Vec<Cell>,
    highlight_origin: Cell,
    highlight_image: IndexedImage,
    last_move: Option<TurnResult>,
}

impl GameScene {
    pub fn new(id: String, is_white: bool) -> Box<GameScene> {
        let client = Client::new();
        let state = get_game(&client, &id);

        Box::new(GameScene {
            client,
            state,
            is_white,
            last_move: None,
            piece_renderer: PieceRenderer::new(),
            board_renderer: BoardRenderer::new(BOARD_POS),
            timer: Timer::new(1.0),
            highlight: Vec::new(),
            highlight_origin: Cell::new_index(0),
            highlight_image: IndexedImage::from_file_contents(include_bytes!("../resources/cell_valid.ici"))
            .unwrap()
            .0,
        })
    }
}

impl Scene<SceneResult, SceneName> for GameScene {
    fn render(&self, graphics: &mut Graphics, mouse: &MouseData, _: &FxHashSet<KeyCode>) {
        graphics.clear(BLACK);
        match &self.state {
            GameClientState::Playing(web_game) => {
                self.board_renderer.render(graphics);
                for (i, cell) in web_game.game.board.cells.iter().enumerate() {
                    if let Some(cell) = cell {
                        let xy = BOARD_POS + (coord!(Cell::new_index(i).to_coord()) * CELL_SIZE);
                        let image = self.piece_renderer.image_for_piece(cell);
                        graphics.draw_indexed_image(xy, image);
                    }
                }
                draw_status(web_game, self.is_white, graphics, &self.last_move);
                for highlight in &self.highlight {
                    let pos = (coord!(highlight.to_coord()) * CELL_SIZE) + BOARD_POS;
                    graphics.draw_indexed_image(pos, &self.highlight_image);
                }
            }
            GameClientState::Error(err) => {
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
        }
    }

    fn on_mouse_click(
        &mut self,
        down_at: Coord,
        mouse: &MouseData,
        mouse_button: MouseButton,
        _: &FxHashSet<KeyCode>,
    ) {
        if mouse_button == MouseButton::Left {
            match &self.state {
                GameClientState::Playing(web_game) => {
                    match &web_game.game.state {
                        GameState::Playing(play_state) => {
                            let is_players_turn = (play_state.player() == PlayerKind::White && self.is_white) || (play_state.player() == PlayerKind::Black && !self.is_white);
                            if is_players_turn {
                                let offset = mouse.xy - BOARD_POS;
                                let grid_coord = offset/CELL_SIZE;
                                if (0..=6).contains(&grid_coord.x) && (0..=6).contains(&grid_coord.y) {
                                    let cell = Cell::new_coord(grid_coord.x as usize, grid_coord.y as usize);
                                    if let Some(piece) = web_game.game.board.cells[cell.index] {
                                        if piece.player == play_state.player() {
                                            self.highlight_origin = cell;
                                            self.highlight = web_game.game.board.get_valid_destinations_for(cell);
                                        }
                                    } else  if self.highlight.contains(&cell) {
                                        self.highlight.clear();
                                        self.state = match self.client.post(format!("{DOMAIN}{URL_PLAY}"))
                                            .json(&PerformActionRequest {
                                                id: web_game.id.clone(),
                                                action: PlayerAction::Move {
                                                    player: play_state.player(),
                                                    from: self.highlight_origin,
                                                    to: cell,
                                                },
                                            }).send() {
                                            Ok(resp) => match resp.json::<PerformActionResponse>() {
                                                Ok(response) => match response {
                                                    PerformActionResponse::Success { game, result } => {
                                                        self.last_move = result;
                                                        get_game(&self.client, &game.id)
                                                    }
                                                    PerformActionResponse::Error(err) => GameClientState::Error(format!("{err:?}")),
                                                },
                                                Err(err) => GameClientState::Error(err.to_string()),
                                            },
                                            Err(err) => GameClientState::Error(err.to_string()),
                                        }
                                    };
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    fn update(
        &mut self,
        timing: &Timing,
        mouse: &MouseData,
        _: &FxHashSet<KeyCode>,
        _: &Window,
    ) -> SceneUpdateResult<SceneResult, SceneName> {
        match &self.state {
            GameClientState::Playing(web_game) => {
                match &web_game.game.state {
                    GameState::Playing(play_state) => {
                        let is_players_turn = play_state.player() == PlayerKind::White && self.is_white;
                        if !is_players_turn {
                            if self.timer.update(timing) {
                                self.state = get_game(&self.client, &web_game.id);
                            }
                        }
                    }
                    GameState::Victory(_) => {}
                }
            }
            GameClientState::Error(_) => {}
        }
        Nothing
    }
}

fn draw_status(web_game: &WebGame, is_white: bool, graphics: &mut Graphics, last_move: &Option<TurnResult>) {
    let pos = coord!(300, 16);
    graphics.draw_text(
        "Crownfall",
        TextPos::px(pos),
        (WHITE, PixelFont::Standard8x10),
    );
    graphics.draw_text(
        &format!(
            "White: {} {}",
            web_game.white_player_name,
            if is_white { "(you)" } else { "" }
        ),
        TextPos::px(pos + (0, 16)),
        (WHITE, PixelFont::Standard6x7),
    );
    graphics.draw_text(
        &format!(
            "Black: {} {}",
            web_game.black_player_name,
            if !is_white { "(you)" } else { "" }
        ),
        TextPos::px(pos + (0, 26)),
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
    if let Some(result) = last_move {
        graphics.draw_text(
            &last_move_text(result),
            TextPos::px(pos + (0, 52)),
            (WHITE, PixelFont::Standard6x7),
        );
    }
}

fn last_move_text(turn_result: &TurnResult) -> String{
    match turn_result {
        TurnResult::PieceMove { from, to } => format!("Moved from {:?} to {:?}", from.to_coord(), to.to_coord()),
        TurnResult::Capture { last_move_from, last_move_to, removed, second_attacker } => format!("Captured piece at {:?}", removed.to_coord()),
        TurnResult::Victory { surrounded_crown } => format!("Captured crown at {:?}", surrounded_crown.to_coord())
    }
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

fn get_game(client: &Client, id: &str) -> GameClientState {
    match client.get(format!("{DOMAIN}/poll/{id}")).send() {
        Ok(response) => match response.json::<GamePollResponse>() {
            Ok(result) => match result {
                GamePollResponse::Active(game) => GameClientState::Playing(game),
                GamePollResponse::Error(err) => GameClientState::Error(format!("{err:?}")),
            },
            Err(err) => GameClientState::Error(err.to_string()),
        },
        Err(err) => GameClientState::Error(err.to_string()),
    }
}
