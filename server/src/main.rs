use axum::extract::{Path, State};
use axum::routing::post;
use axum::{Json, Router, routing::get};
use chrono::Utc;
use common::{
    CreateGameRequest, CreateGameResponse, ErrorKind, GamePollResponse, JoinGameRequest,
    JoinGameResponse, PendingGame, PendingGameListResponse, PendingPollResult,
    PerformActionRequest, PerformActionResponse, URL_CREATE, URL_JOIN, URL_PENDING, URL_PLAY,
    WebGame,
};
use game::Game;
use game::errors::GameError;
use std::collections::HashMap;
use std::future::Pending;
use std::net::SocketAddr;
use std::sync::{Arc, LockResult, Mutex};
use tokio::net::TcpListener;
use uuid::Uuid;

#[derive(Clone, Debug)]
struct AppState {
    pending: Arc<Mutex<HashMap<String, PendingGame>>>,
    games: Arc<Mutex<HashMap<String, WebGame>>>,
    max_games: usize,
}

impl AppState {
    pub fn new(game_count: usize) -> Self {
        Self {
            pending: Arc::new(Mutex::new(HashMap::with_capacity(game_count))),
            games: Arc::new(Mutex::new(HashMap::with_capacity(game_count))),
            max_games: game_count,
        }
    }

    fn game_count(&self) -> usize {
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).len()
            + self.games.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let max_games: usize = std::env::var("MAX_GAMES")
        .unwrap_or("10".to_string())
        .parse()
        .unwrap();

    let game_list_state = AppState::new(max_games);

    let reset_password: String = std::env::var("RESET")
        .unwrap_or("/reset".to_string())
        .parse()
        .unwrap();

    let app = Router::new()
        .route(URL_PENDING, get(pending))
        .route("/poll/{id}", get(poll))
        .route("/pending_poll/{id}", get(poll_pending))
        .route(URL_CREATE, post(create_game))
        .route(URL_JOIN, post(join_game))
        .route(URL_PLAY, post(perform_action))
        .route("/", get(root))
        .route(&reset_password, get(reset))
        .with_state(game_list_state);

    let port: u16 = std::env::var("PORT")
        .unwrap_or("3000".into())
        .parse()
        .expect("failed to convert to number");

    let ipv6 = SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 0], port));
    let ipv6_listener = TcpListener::bind(&ipv6).await.unwrap();

    tracing::info!("Listening at {}!", ipv6);

    axum::serve(ipv6_listener, app).await.unwrap();
}

async fn root() -> &'static str {
    "Crownfall!"
}

async fn pending(State(state): State<AppState>) -> Json<PendingGameListResponse> {
    match state.pending.lock() {
        Ok(map) => Json(PendingGameListResponse::Success(
            map.values().cloned().collect(),
        )),
        Err(error) => {
            tracing::error!("{}", error);
            Json(PendingGameListResponse::Error(ErrorKind::General))
        }
    }
}

async fn poll(State(state): State<AppState>, Path(id): Path<String>) -> Json<GamePollResponse> {
    match state.games.lock() {
        Ok(map) => match map.get(&id) {
            None => Json(GamePollResponse::Error(ErrorKind::InvalidGame)),
            Some(game) => Json(GamePollResponse::Active(game.clone())),
        },
        Err(error) => {
            tracing::error!("{}", error);
            Json(GamePollResponse::Error(ErrorKind::General))
        }
    }
}

async fn poll_pending(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<PendingPollResult> {
    match state.pending.lock() {
        Ok(map) => match map.get(&id) {
            None => match state.games.lock() {
                Ok(map) => match map.get(&id) {
                    None => Json(PendingPollResult::Error(ErrorKind::InvalidGame)),
                    Some(game) => Json(PendingPollResult::Joined(game.clone())),
                },
                Err(error) => {
                    tracing::error!("{}", error);
                    Json(PendingPollResult::Error(ErrorKind::General))
                }
            },
            Some(_) => Json(PendingPollResult::Waiting),
        },
        Err(error) => {
            tracing::error!("{}", error);
            Json(PendingPollResult::Error(ErrorKind::General))
        }
    }
}

async fn create_game(
    State(state): State<AppState>,
    Json(payload): Json<CreateGameRequest>,
) -> Json<CreateGameResponse> {
    if state.game_count() < state.max_games {
        let id = Uuid::new_v4();
        let pending_game = PendingGame {
            id: id.to_string(),
            white_player_name: payload.player_name,
            created: Utc::now(),
        };
        state
            .pending
            .lock()
            .unwrap()
            .insert(id.to_string(), pending_game);
        Json(CreateGameResponse::Success(id.to_string()))
    } else {
        Json(CreateGameResponse::Error(ErrorKind::TooManyGames))
    }
}

async fn join_game(
    State(state): State<AppState>,
    Json(payload): Json<JoinGameRequest>,
) -> Json<JoinGameResponse> {
    let id = payload.id;
    match state.pending.lock() {
        Ok(mut map) => {
            if let Some(pending) = map.remove(&id) {
                let game = WebGame {
                    id: id.clone(),
                    game: Default::default(),
                    white_player_name: pending.white_player_name,
                    black_player_name: payload.player_name,
                };
                state.games.lock().unwrap().insert(id, game.clone());
                Json(JoinGameResponse::Success(game))
            } else {
                Json(JoinGameResponse::Error(ErrorKind::General))
            }
        }
        Err(error) => {
            tracing::error!("{}", error);
            Json(JoinGameResponse::Error(ErrorKind::General))
        }
    }
}

async fn perform_action(
    State(state): State<AppState>,
    Json(payload): Json<PerformActionRequest>,
) -> Json<PerformActionResponse> {
    match state.games.lock() {
        Ok(mut map) => match map.get(&payload.id) {
            None => Json(PerformActionResponse::Error(ErrorKind::InvalidGame)),
            Some(web_game) => {
                let mut updated = web_game.clone();
                match updated.game.clone().handle_player_action(payload.action) {
                    Ok((game, result)) => {
                        updated.game = game.clone();
                        map.insert(payload.id.clone(), updated.clone());
                        Json(PerformActionResponse::Success { game: updated, result })
                    }
                    Err(error) => {
                        tracing::warn!("{}", error);
                        Json(PerformActionResponse::Error(game_error_to_kind(&error)))
                    }
                }
            }
        },
        Err(error) => {
            tracing::error!("{}", error);
            Json(PerformActionResponse::Error(ErrorKind::General))
        }
    }
}

fn game_error_to_kind(error: &GameError) -> ErrorKind {
    match error {
        GameError::GameOver(_) => ErrorKind::GameOver,
        _ => ErrorKind::InvalidMove,
    }
}

async fn reset(State(state): State<AppState>) -> &'static str {
    match (state.pending.lock(), state.games.lock()) {
        (Ok(mut pending), Ok(mut games)) => {
            pending.clear();
            games.clear();
            "OK"
        }
        _ => {
            tracing::error!("failed to lock state during reset");
            "Error"
        }
    }
}
