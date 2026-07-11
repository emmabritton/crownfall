mod create_game;
mod game_list_scene;
mod join_game;
mod joining;
mod game;
mod game_renderer;

use crate::create_game::CreateGameScene;
use crate::game_list_scene::GameListScene;
use crate::join_game::JoinGameScene;
use crate::joining::JoiningScene;
use anyhow::Result;
use pixels_graphics_lib::prelude::*;
use crate::game::GameScene;

const WIDTH: usize = 600;
const HEIGHT: usize = 600;
const DOMAIN: &'static str = "https://crownfall-production-e14d.up.railway.app";

fn main() -> Result<()> {
    let window_prefs = WindowPreferences::new("com", "emmabritton", "crownfall", 1)?;
    let options = Options::default();
    let switcher: SceneSwitcher<SceneResult, SceneName> =
        |style, scene_stack, new_scene| match new_scene {
            SceneName::GameList => {
                scene_stack.clear();
                scene_stack.push(GameListScene::new(style));
            }
            SceneName::CreateGame => scene_stack.push(CreateGameScene::new(style)),
            SceneName::JoinGame(id) => scene_stack.push(JoinGameScene::new(id, style)),
            SceneName::Joining(id) => scene_stack.push(JoiningScene::new(id)),
            SceneName::Game(id, is_white) => scene_stack.push(GameScene::new(id, is_white)),
        };
    let first_scene = GameListScene::new(&options.style);
    run_scenes(
        WIDTH,
        HEIGHT,
        "Crownfall",
        Some(window_prefs),
        switcher,
        first_scene,
        options,
        empty_pre_post(),
    )?;
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
enum SceneName {
    GameList,
    CreateGame,
    JoinGame(String),
    Joining(String),
    Game(String, bool),
}

#[derive(Clone, Debug, PartialEq)]
enum SceneResult {}
