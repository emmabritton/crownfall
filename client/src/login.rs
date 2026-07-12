use crate::net::{poll, send};
use crate::{BACKGROUND, HEIGHT, SceneName, SceneResult, Settings, WIDTH};
use networking::packet::Packet;
use pixels_graphics_lib::prelude::*;
use pixels_graphics_lib::scenes::SceneUpdateResult::Nothing;
use pixels_graphics_lib::ui::layout::relative::LayoutContext;
use pixels_graphics_lib::ui::prelude::*;
use pixels_graphics_lib::ui::styles::UiStyle;
use pixels_graphics_lib::{layout, px, render};

#[derive(Debug, Clone, Eq, PartialEq)]
enum LoginState {
    Input,
    Loading,
    Error(String),
}

pub struct LoginScene {
    title_left: IndexedImage,
    title_right: IndexedImage,
    label: Label,
    username: TextField,
    submit: Button,
    join: bool,
    state: LoginState,
    prefs: AppPrefs<Settings>,
}

impl LoginScene {
    pub fn new(prefs: AppPrefs<Settings>, style: &UiStyle) -> Box<LoginScene> {
        let title_left =
            IndexedImage::from_file_contents(include_bytes!("../resources/title_left.ici"))
                .unwrap()
                .0;
        let title_right =
            IndexedImage::from_file_contents(include_bytes!("../resources/title_right.ici"))
                .unwrap()
                .0;

        let mut label = Label::new(Text::new(
            "Please enter your name",
            TextPos::px(Coord::default()),
            (WHITE, PixelFont::Standard6x7),
        ));
        let mut username = TextField::new(
            Coord::default(),
            10,
            PixelFont::Standard6x7,
            (Some(80), Some(80)),
            "",
            &[TextFilter::Letters, TextFilter::Numbers],
            &style.text_field,
        );
        let mut submit = Button::new(Coord::default(), "Login", Some(50), &style.button);

        username.focus();

        let context = LayoutContext::new(Rect::new((16, 100), (200, 300)));

        layout!(context, label, align_top);
        layout!(context, label, align_left);

        layout!(context, username, left_to_left_of label);
        layout!(context, username, top_to_bottom_of label, px!(16));

        layout!(context, submit, left_to_left_of username);
        layout!(context, submit, top_to_bottom_of username, px!(16));

        Box::new(Self {
            title_right,
            title_left,
            label,
            username,
            submit,
            join: false,
            prefs,
            state: LoginState::Input,
        })
    }
}

impl Scene<SceneResult, SceneName> for LoginScene {
    fn render(&self, graphics: &mut Graphics, mouse: &MouseData, _: &FxHashSet<KeyCode>) {
        graphics.clear(BACKGROUND);

        graphics.draw_indexed_image(coord!(140, 32), &self.title_left);
        graphics.draw_indexed_image(coord!(202, 32), &self.title_right);

        match &self.state {
            LoginState::Input => {
                render!(graphics, mouse, self.submit, self.username, self.label);
            }
            LoginState::Loading => graphics.draw_text(
                "Logging in...",
                TextPos::px(coord!(WIDTH, HEIGHT) / 2),
                (WHITE, PixelFont::Standard6x7, Positioning::Center),
            ),
            LoginState::Error(err) => {
                graphics.clear(BLACK);
                graphics.draw_text(
                    &format!("Error: {err}"),
                    TextPos::px(coord!(WIDTH, HEIGHT) / 2),
                    (RED, PixelFont::Standard6x7, WrappingStrategy::AtCol(100)),
                );
            }
        }
    }

    fn on_key_up(&mut self, key: KeyCode, _: &MouseData, held: &FxHashSet<KeyCode>) {
        if self.state == LoginState::Input {
            self.username.on_key_press(key, held);
            if key == KeyCode::Enter {
                self.join = true;
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
        if self.state == LoginState::Input && mouse_button == MouseButton::Left {
            if self.submit.on_mouse_click(down_at, mouse.xy) {
                self.join = true;
            }
            self.username.on_mouse_click(down_at, mouse.xy);
        }
    }

    fn update(
        &mut self,
        timing: &Timing,
        _: &MouseData,
        _: &FxHashSet<KeyCode>,
        _: &Window,
    ) -> SceneUpdateResult<SceneResult, SceneName> {
        self.username.update(timing);

        if self.join && !self.username.content().is_empty() {
            self.state = LoginState::Loading;
            self.prefs.data.username = Some(self.username.content().to_string());
            if let Err(e) = send(Packet::LoginRequest(self.username.content().to_string())) {
                self.state = LoginState::Error(format!("{:?}", e));
            }
        }

        if self.state == LoginState::Loading {
            match poll() {
                Ok(packets) => {
                    for packet in packets {
                        if let Packet::LoginResponse(packet) = packet {
                            self.prefs.save();
                            return if let Some(game) = packet {
                                SceneUpdateResult::Push(true, SceneName::RejoinGame(game.id))
                            } else {
                                SceneUpdateResult::Push(true, SceneName::GameList)
                            };
                        }
                    }
                }
                Err(err) => self.state = LoginState::Error(format!("{:?}", err)),
            }
        }

        self.join = false;

        Nothing
    }
}
