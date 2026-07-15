mod ai_game;
mod ai_settings_scene;
mod game;
mod game_list_scene;
mod game_renderer;
mod login;
mod net;
mod rules_settings_scene;

use crate::ai_game::AiGameScene;
use crate::ai_settings_scene::AiSettingsScene;
use crate::game::GameScene;
use crate::game_list_scene::GameListScene;
use crate::rules_settings_scene::{RulesPurpose, RulesSettingsScene};
use ::eb_crownfall_engine::CrownfallRules;
use ::eb_crownfall_engine::ai::{CrownfallDifficulty, CrownfallPersonality};
use anyhow::Result;
use pixels_graphics_lib::prelude::*;
use std::net::{SocketAddr, ToSocketAddrs};

const WIDTH: usize = 500;
const HEIGHT: usize = 300;

const BACKGROUND: Color = Color {
    r: 30,
    g: 30,
    b: 140,
    a: 255,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Settings {
    pub username: Option<String>,
}

fn settings() -> AppPrefs<Settings> {
    AppPrefs::new("app", "emmabritton", "crownfall", || Settings {
        username: None,
    })
    .expect("Unable to create prefs file")
}

fn username() -> Option<String> {
    match std::env::var("NAME") {
        Ok(txt) => Some(txt),
        Err(_) => settings().data.username,
    }
}

use crate::login::LoginScene;
use crate::net::send;
use networking::packet::{GameId, Packet};
use serde::{Deserialize, Serialize};

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    if std::env::args().any(|x| x == "--reset") {
        let mut prefs = settings();
        prefs.data.username = None;
        prefs.save();
        return Ok(());
    }

    let addr = if std::env::var("LOCAL").is_ok() {
        SocketAddr::from(([127, 0, 0, 1], 3000))
    } else {
        "caboose.proxy.rlwy.net:55421"
            .to_socket_addrs()
            .expect("creating socket addr")
            .next()
            .expect("no match found for socket")
    };

    net::init(addr)?;
    let window_prefs = WindowPreferences::new("com", "emmabritton", "crownfall", 3)?;
    let options = Options::default();
    let switcher: SceneSwitcher<SceneResult, SceneName> =
        |style, scene_stack, new_scene| match new_scene {
            SceneName::GameList => {
                scene_stack.clear();
                scene_stack.push(GameListScene::new(style));
            }
            SceneName::Game(id, board_length) => scene_stack.push(GameScene::new(id, board_length)),
            SceneName::RejoinGame(game_id, board_length) => {
                scene_stack.push(GameListScene::new(style));
                scene_stack.push(GameScene::new(game_id, board_length));
            }
            SceneName::AiSettings => scene_stack.push(AiSettingsScene::new(style)),
            SceneName::RulesSettings(purpose) => {
                scene_stack.push(RulesSettingsScene::new(style, purpose))
            }
            SceneName::AiGame(difficulty, personality, rules) => {
                scene_stack.push(AiGameScene::new(difficulty, personality, rules))
            }
        };
    run_scenes(
        WIDTH,
        HEIGHT,
        "Crownfall",
        Some(window_prefs),
        switcher,
        if username().is_none() {
            LoginScene::new(settings(), &options.style)
        } else {
            send(Packet::LoginRequest(
                username().expect("Username is required"),
            ))
            .expect("Failed to login");
            GameListScene::new(&options.style)
        },
        options,
        empty_pre_post(),
    )?;
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
enum SceneName {
    GameList,
    Game(GameId, usize),
    RejoinGame(GameId, usize),
    AiSettings,
    RulesSettings(RulesPurpose),
    AiGame(CrownfallDifficulty, CrownfallPersonality, CrownfallRules),
}

#[derive(Clone, Debug, PartialEq)]
enum SceneResult {
    GameCreationRequested,
    GameCreationFailed(String),
}
