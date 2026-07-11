use crate::{DOMAIN, HEIGHT, SceneName, SceneResult, WIDTH};
use common::{CreateGameRequest, CreateGameResponse, URL_CREATE};
use pixels_graphics_lib::prelude::*;
use pixels_graphics_lib::scenes::SceneUpdateResult::Nothing;
use pixels_graphics_lib::ui::prelude::relative::*;
use pixels_graphics_lib::ui::prelude::*;
use pixels_graphics_lib::{layout, px};
use reqwest::blocking::Client;

pub enum CreateGameState {
    Input,
    Error(String),
}

pub struct CreateGameScene {
    player_name: TextField,
    submit: Button,
    background: ShapeCollection,
    label: Label,
    join: bool,
    state: CreateGameState,
}

impl CreateGameScene {
    pub fn new(style: &UiStyle) -> Box<CreateGameScene> {
        let background = dialog_background(WIDTH, HEIGHT, &style.dialog);

        let context = LayoutContext::new(style.dialog.bounds.clone());
        let mut label = Label::new(Text::new(
            "Enter your name",
            TextPos::px((0, 0)),
            (WHITE, PixelFont::Standard8x10),
        ));
        let mut player_name = TextField::new(
            (0, 0),
            10,
            PixelFont::Standard6x7,
            (Some(70), Some(70)),
            "",
            &[TextFilter::Letters, TextFilter::Numbers],
            &style.text_field,
        );
        let mut submit = Button::new((0, 0), "Create", Some(50), &style.button);
        player_name.focus();

        layout!(context, label, align_top, px!(8));
        layout!(context, label, align_left, px!(8));
        layout!(context, player_name, align_left, px!(8));
        layout!(context, player_name, top_to_bottom_of label, px!(8));
        layout!(context, submit, align_left, px!(8));
        layout!(context, submit, top_to_bottom_of player_name, px!(8));

        Box::new(Self {
            background,
            player_name,
            submit,
            label,
            join: false,
            state: CreateGameState::Input,
        })
    }
}

impl Scene<SceneResult, SceneName> for CreateGameScene {
    fn render(&self, graphics: &mut Graphics, mouse: &MouseData, _: &FxHashSet<KeyCode>) {
        self.background.render(graphics);
        match &self.state {
            CreateGameState::Input => {
                self.label.render(graphics, mouse);
                self.player_name.render(graphics, mouse);
                self.submit.render(graphics, mouse);
            }
            CreateGameState::Error(txt) => {
                graphics.draw_text(
                    txt,
                    TextPos::px(self.label.bounds().top_left()),
                    (WHITE, PixelFont::Standard6x7, WrappingStrategy::AtCol(50)),
                );
            }
        }
    }

    fn on_key_up(&mut self, key: KeyCode, _: &MouseData, held: &FxHashSet<KeyCode>) {
        self.player_name.on_key_press(key, held);
        if key == KeyCode::Enter {
            self.join = true;
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
            if self.submit.on_mouse_click(down_at, mouse.xy) {
                self.join = true;
            }
            self.player_name.on_mouse_click(down_at, mouse.xy);
        }
    }

    fn update(
        &mut self,
        timing: &Timing,
        _: &MouseData,
        _: &FxHashSet<KeyCode>,
        _: &Window,
    ) -> SceneUpdateResult<SceneResult, SceneName> {
        self.player_name.update(timing);
        self.submit.update(timing);

        if !self.player_name.content().is_empty() && self.join {
            match Client::new()
                .post(format!("{DOMAIN}{URL_CREATE}"))
                .json(&CreateGameRequest {
                    player_name: self.player_name.content().to_string(),
                })
                .send()
            {
                Ok(result) => match result.json::<CreateGameResponse>() {
                    Ok(response) => match response {
                        CreateGameResponse::Success(id) => {
                            return SceneUpdateResult::Push(true, SceneName::Joining(id));
                        }
                        CreateGameResponse::Error(err) => {
                            self.state = CreateGameState::Error(format!("{err:?}"))
                        }
                    },
                    Err(err) => {
                        self.state = CreateGameState::Error(err.to_string());
                    }
                },
                Err(err) => {
                    self.state = CreateGameState::Error(err.to_string());
                }
            }
        }

        self.join = false;

        Nothing
    }

    fn is_dialog(&self) -> bool {
        true
    }
}
