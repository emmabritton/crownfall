use crate::DOMAIN;
use crate::{HEIGHT, SceneName, SceneResult, WIDTH};
use chrono::Local;
use common::{PendingGame, PendingGameListResponse, URL_PENDING};
use pixels_graphics_lib::prelude::SceneUpdateResult::Nothing;
use pixels_graphics_lib::prelude::*;
use pixels_graphics_lib::ui::button::Button;
use pixels_graphics_lib::ui::styles::UiStyle;
use pixels_graphics_lib::ui::{PixelView, ViewState};
use reqwest::blocking::Client;

enum GameListState {
    Loading,
    List(Vec<PendingGame>),
    Error(String),
}

pub struct GameListScene {
    client: Client,
    state: GameListState,
    reload_timer: Timer,
    list_buttons: Vec<Button>,
    create_button: Button,
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
            client: Client::new(),
            state: GameListState::Loading,
            reload_timer: Timer::new_with_delay(8.0, 4.0),
            list_buttons,
            create_button: Button::new(coord!(16, 500), "Create game", Some(100), &style.button),
            result: Nothing,
        })
    }
}

impl Scene<SceneResult, SceneName> for GameListScene {
    fn render(&self, graphics: &mut Graphics, mouse: &MouseData, _: &FxHashSet<KeyCode>) {
        match &self.state {
            GameListState::Loading => {
                graphics.clear(BLACK);
                graphics.draw_text(
                    "Loading...",
                    TextPos::px(coord!(WIDTH / 2, HEIGHT / 2)),
                    (WHITE, PixelFont::Standard6x7, Positioning::Center),
                )
            }
            GameListState::List(list) => {
                graphics.clear(BLACK);
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
                            game.created
                                .with_timezone(&Local::now().timezone())
                                .time()
                                .to_string()
                        ),
                        TextPos::px(start + (130, line_height * i + 10)),
                        (WHITE, PixelFont::Standard6x7, Positioning::LeftBottom),
                    );
                });
                self.create_button.render(graphics, mouse);
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
            if let GameListState::List(list) = &self.state {
                if self.create_button.on_mouse_click(down_at, mouse.xy) {
                    self.result = SceneUpdateResult::Push(false, SceneName::CreateGame)
                }
                for (i, button) in self.list_buttons.iter_mut().enumerate() {
                    if button.on_mouse_click(down_at, mouse.xy) {
                        self.result =
                            SceneUpdateResult::Push(false, SceneName::JoinGame(list[i].id.clone()));
                    }
                }
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
        match self.state {
            GameListState::Loading => {
                match self.client.get(format!("{DOMAIN}{URL_PENDING}")).send() {
                    Ok(response) => match response.json::<PendingGameListResponse>() {
                        Ok(resp) => match resp {
                            PendingGameListResponse::Success(mut list) => {
                                list.sort_by(|l, r| l.created.cmp(&r.created));
                                self.list_buttons.iter_mut().enumerate().for_each(|(i, b)| {
                                    let state = if i < list.len() {
                                        ViewState::Normal
                                    } else {
                                        ViewState::Disabled
                                    };
                                    b.set_state(state);
                                });
                                self.reload_timer.reset();
                                self.state = GameListState::List(list);
                            }
                            PendingGameListResponse::Error(error) => {
                                self.state = GameListState::Error(format!("{error:?}"))
                            }
                        },
                        Err(error) => self.state = GameListState::Error(error.to_string()),
                    },
                    Err(error) => self.state = GameListState::Error(error.to_string()),
                }
            }
            GameListState::List(_) => {
                if self.reload_timer.update(timing) {
                    self.state = GameListState::Loading;
                }
                self.create_button.update(timing);
                for button in self.list_buttons.iter_mut() {
                    button.update(timing);
                }
            }
            GameListState::Error(_) => {}
        }

        self.result.clone()
    }

    fn resuming(&mut self, _: Option<SceneResult>) {
        self.result = Nothing;
        self.state = GameListState::Loading;
    }
}
