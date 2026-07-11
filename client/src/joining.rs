use crate::{DOMAIN, HEIGHT, SceneName, SceneResult, WIDTH};
use common::PendingPollResult;
use pixels_graphics_lib::prelude::*;
use pixels_graphics_lib::scenes::SceneUpdateResult::Nothing;
use reqwest::blocking::Client;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum JoiningState {
    Waiting,
    Error(String),
}

pub struct JoiningScene {
    id: String,
    timer: Timer,
    client: Client,
    state: JoiningState,
}

impl JoiningScene {
    pub fn new(id: String) -> Box<JoiningScene> {
        Box::new(JoiningScene {
            id,
            timer: Timer::new_with_delay(1.0, 3.0),
            client: Client::new(),
            state: JoiningState::Waiting,
        })
    }
}

impl Scene<SceneResult, SceneName> for JoiningScene {
    fn render(&self, graphics: &mut Graphics, _: &MouseData, _: &FxHashSet<KeyCode>) {
        graphics.clear(BLACK);
        match &self.state {
            JoiningState::Waiting => graphics.draw_text(
                "Waiting...",
                TextPos::px(coord!(WIDTH / 2, HEIGHT / 2)),
                (WHITE, PixelFont::Standard6x7, Positioning::Center),
            ),
            JoiningState::Error(txt) => graphics.draw_text(
                &format!("Error: {}", txt),
                TextPos::px(coord!(WIDTH / 2, HEIGHT / 2)),
                (
                    RED,
                    PixelFont::Standard6x7,
                    WrappingStrategy::AtCol(60),
                    Positioning::Center,
                ),
            ),
        }
    }

    fn update(
        &mut self,
        timing: &Timing,
        _: &MouseData,
        _: &FxHashSet<KeyCode>,
        _: &Window,
    ) -> SceneUpdateResult<SceneResult, SceneName> {
        if self.state == JoiningState::Waiting && self.timer.update(timing) {
            match self.client.get(format!("{DOMAIN}/pending_poll/{}", self.id)).send() {
                Ok(response) => match response.json::<PendingPollResult>() {
                    Ok(resp) => match resp {
                        PendingPollResult::Waiting => {}
                        PendingPollResult::Joined(game) => {
                            return SceneUpdateResult::Push(true, SceneName::Game(game.id, true));
                        }
                        PendingPollResult::Error(err) => {
                            self.state = JoiningState::Error(format!("{err:?}"))
                        }
                    },
                    Err(err) => self.state = JoiningState::Error(err.to_string()),
                },
                Err(err) => self.state = JoiningState::Error(err.to_string()),
            }
        }
        Nothing
    }
}
