use chrono::{DateTime, Utc};
use eb_crownfall_engine::Game;
use serde::{Deserialize, Serialize};

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
