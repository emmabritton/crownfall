use chrono::{DateTime, Utc};
use game::{Game, PlayerAction, TurnResult};
use serde::{Deserialize, Serialize};

pub const URL_PENDING: &str = "/pending";
pub const URL_CREATE: &str = "/create";
pub const URL_JOIN: &str = "/join";
pub const URL_PLAY: &str = "/play";

#[derive(Clone, Debug, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum ErrorKind {
    General,
    TooManyGames,
    InvalidMove,
    GameOver,
    InvalidGame,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateGameRequest {
    pub player_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JoinGameRequest {
    pub id: String,
    pub player_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CreateGameResponse {
    Success(String),
    Error(ErrorKind),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum JoinGameResponse {
    Success(WebGame),
    Error(ErrorKind),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PendingGameListResponse {
    Success(Vec<PendingGame>),
    Error(ErrorKind),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PerformActionRequest {
    pub id: String,
    pub action: PlayerAction,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PerformActionResponse {
    Success {
        game: WebGame,
        result: Option<TurnResult>,
    },
    Error(ErrorKind),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WebGame {
    pub id: String,
    pub game: Game,
    pub white_player_name: String,
    pub black_player_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingGame {
    pub id: String,
    pub white_player_name: String,
    pub created: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PendingPollResult {
    Waiting,
    Joined(WebGame),
    Error(ErrorKind),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GamePollResponse {
    Active(WebGame),
    Error(ErrorKind),
}
