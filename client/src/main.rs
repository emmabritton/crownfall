mod game;
mod game_list_scene;
mod game_renderer;
mod login;
mod net;

use crate::game::GameScene;
use crate::game_list_scene::GameListScene;
use anyhow::Result;
use pixels_graphics_lib::prelude::*;
use std::net::{SocketAddr, ToSocketAddrs};

const WIDTH: usize = 400;
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
    let window_prefs = WindowPreferences::new("com", "emmabritton", "crownfall", 2)?;
    let options = Options::default();
    let switcher: SceneSwitcher<SceneResult, SceneName> =
        |style, scene_stack, new_scene| match new_scene {
            SceneName::GameList => {
                scene_stack.clear();
                scene_stack.push(GameListScene::new(style));
            }
            SceneName::Game(id) => scene_stack.push(GameScene::new(id)),
            SceneName::RejoinGame(game_id) => {
                scene_stack.push(GameListScene::new(style));
                scene_stack.push(GameScene::new(game_id));
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
    Game(GameId),
    RejoinGame(GameId),
}

#[derive(Clone, Debug, PartialEq)]
enum SceneResult {}
