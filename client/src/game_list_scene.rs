use crate::BACKGROUND;
use crate::net::{poll, send};
use crate::{HEIGHT, SceneName, SceneResult, WIDTH};
use chrono::Local;
use networking::models::PendingGame;
use networking::packet::{Packet, PendingGameState};
use pixels_graphics_lib::prelude::SceneUpdateResult::Nothing;
use pixels_graphics_lib::prelude::*;
use pixels_graphics_lib::ui::button::Button;
use pixels_graphics_lib::ui::styles::UiStyle;
use pixels_graphics_lib::ui::{PixelView, ViewState};

enum GameListState {
    PreLoad,
    Loading,
    Joining(usize),
    Creating,
    WaitingForOtherPlayer(PendingGame),
    List(Vec<PendingGame>),
    Error(String),
}

pub struct GameListScene {
    state: GameListState,
    reload_timer: Timer,
    join_timer: Timer,
    list_buttons: Vec<Button>,
    create_button: Button,
    ai_button: Button,
    result: SceneUpdateResult<SceneResult, SceneName>,
}

impl GameListScene {
    pub fn new(style: &UiStyle) -> Box<GameListScene> {
        let mut list_buttons = Vec::new();
        for i in 0..10 {
            list_buttons.push(Button::new(
                coord!(16, 16) + (0, i * 24),
                "Join",
                Some(40),
                &style.button,
            ));
            list_buttons
                .last_mut()
                .unwrap()
                .set_state(ViewState::Disabled);
        }
        Box::new(Self {
            state: GameListState::PreLoad,
            reload_timer: Timer::new_with_delay(20.0, 10.0),
            join_timer: Timer::new_with_delay(10.0, 1.0),
            list_buttons,
            create_button: Button::new(coord!(16, 270), "Create game", Some(100), &style.button),
            ai_button: Button::new(coord!(160, 270), "Play vs AI", Some(100), &style.button),
            result: Nothing,
        })
    }
}

impl Scene<SceneResult, SceneName> for GameListScene {
    fn render(&self, graphics: &mut Graphics, mouse: &MouseData, _: &FxHashSet<KeyCode>) {
        graphics.clear(BACKGROUND);

        match &self.state {
            GameListState::PreLoad
            | GameListState::Creating
            | GameListState::Joining(_)
            | GameListState::Loading => graphics.draw_text(
                "Loading games...",
                TextPos::px(coord!(WIDTH / 2, HEIGHT / 2)),
                (WHITE, PixelFont::Standard6x7, Positioning::Center),
            ),
            GameListState::WaitingForOtherPlayer(_) => graphics.draw_text(
                "Waiting for other player...",
                TextPos::px(coord!(WIDTH / 2, HEIGHT / 2)),
                (WHITE, PixelFont::Standard6x7, Positioning::Center),
            ),
            GameListState::List(list) => {
                let start = coord!(66, 22);
                let line_height = 24;
                list.iter().enumerate().for_each(|(i, game)| {
                    graphics.draw_text(
                        &format!("{}'s Game", game.white_player_name),
                        TextPos::px(start + (0, line_height * i)),
                        (WHITE, PixelFont::Standard8x10),
                    );
                    graphics.draw_text(
                        &format!(
                            "Created at {}",
                            game.created.with_timezone(&Local::now().timezone()).time()
                        ),
                        TextPos::px(start + (130, line_height * i + 10)),
                        (WHITE, PixelFont::Standard6x7, Positioning::LeftBottom),
                    );
                });
                self.create_button.render(graphics, mouse);
                self.ai_button.render(graphics, mouse);
                for button in &self.list_buttons {
                    button.render(graphics, mouse);
                }
            }
            GameListState::Error(text) => {
                graphics.clear(BLACK);
                graphics.draw_text(
                    &format!("Error: {text}\nPlease restart client"),
                    TextPos::px(coord!(WIDTH / 2, HEIGHT / 2)),
                    (
                        RED,
                        PixelFont::Standard6x7,
                        WrappingStrategy::AtCol(60),
                        Positioning::Center,
                    ),
                )
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
            let mut join_clicked = None;
            for (i, button) in self.list_buttons.iter_mut().enumerate() {
                if button.on_mouse_click(down_at, mouse.xy) {
                    join_clicked = Some(i);
                    break;
                }
            }
            if let Some(idx) = join_clicked {
                let mut id = None;
                let mut len = None;
                if let GameListState::List(games) = &self.state
                    && let Some(game) = games.get(idx)
                {
                    id = Some(game.id.clone());
                    len = Some(game.rules.board.length());
                }
                if let Some(id) = id
                    && let Some(len) = len
                {
                    if let Err(e) = send(Packet::JoinGameRequest(id)) {
                        self.state = GameListState::Error(format!("{:?}", e));
                    } else {
                        self.state = GameListState::Joining(len);
                    }
                }
            }

            if self.create_button.on_mouse_click(down_at, mouse.xy) {
                if let Err(e) = send(Packet::CreateGameRequest) {
                    self.state = GameListState::Error(format!("{:?}", e));
                } else {
                    self.state = GameListState::Creating;
                }
            }
            if self.ai_button.on_mouse_click(down_at, mouse.xy) {
                self.result = SceneUpdateResult::Push(false, SceneName::AiSettings);
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
        match &self.state {
            GameListState::PreLoad => {
                if let Err(e) = send(Packet::PendingListRequest) {
                    self.state = GameListState::Error(format!("{:?}", e));
                } else {
                    self.state = GameListState::Loading;
                }
            }
            GameListState::Loading => match poll() {
                Ok(packets) => {
                    for packet in packets {
                        if let Packet::PendingListResponse(list) = packet {
                            self.list_buttons.iter_mut().enumerate().for_each(|(i, b)| {
                                let state = if i < list.len() {
                                    ViewState::Normal
                                } else {
                                    ViewState::Disabled
                                };
                                b.set_state(state);
                            });
                            self.state = GameListState::List(list);
                        }
                    }
                }
                Err(e) => self.state = GameListState::Error(format!("{:?}", e)),
            },
            GameListState::List(_) => {
                if self.reload_timer.update(timing) {
                    self.state = GameListState::PreLoad;
                }
                self.create_button.update(timing);
                self.ai_button.update(timing);
                for button in self.list_buttons.iter_mut() {
                    button.update(timing);
                }
            }
            GameListState::Error(_) => {}
            GameListState::Joining(len) => {
                let len = *len;
                match poll() {
                    Ok(packets) => {
                        for packet in packets {
                            if let Packet::JoinGameResponse(id, joined) = packet {
                                if joined {
                                    return SceneUpdateResult::Push(false, SceneName::Game(id, len));
                                } else {
                                    self.state = GameListState::PreLoad;
                                }
                            }
                        }
                    }
                    Err(e) => self.state = GameListState::Error(format!("{:?}", e)),
                }
            },
            GameListState::Creating => match poll() {
                Ok(packets) => {
                    for packet in packets {
                        if let Packet::CreateGameResponse(game) = packet {
                            if let Some(pending) = game {
                                self.state = GameListState::WaitingForOtherPlayer(pending)
                            } else {
                                self.state = GameListState::PreLoad;
                            }
                        }
                    }
                }
                Err(e) => self.state = GameListState::Error(format!("{:?}", e)),
            },
            GameListState::WaitingForOtherPlayer(game) => {
                let game = game.clone();
                if self.join_timer.update(timing)
                    && let Err(e) = send(Packet::PollPendingGameRequest(game.id.clone()))
                {
                    self.state = GameListState::Error(format!("{:?}", e));
                }
                match poll() {
                    Ok(packets) => {
                        for packet in packets {
                            if let Packet::PollPendingGameResponse(state) = packet {
                                match state {
                                    PendingGameState::Pending => {}
                                    PendingGameState::Joined => {
                                        return SceneUpdateResult::Push(
                                            false,
                                            SceneName::Game(game.id.clone(), game.rules.board.length()),
                                        );
                                    }
                                    PendingGameState::Invalid => {
                                        self.state = GameListState::Error(
                                            "Game state is invalid".to_string(),
                                        )
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => self.state = GameListState::Error(format!("{:?}", e)),
                }
            }
        }

        self.result.clone()
    }

    fn resuming(&mut self, _: Option<SceneResult>) {
        self.result = Nothing;
        self.state = GameListState::PreLoad;
    }
}
