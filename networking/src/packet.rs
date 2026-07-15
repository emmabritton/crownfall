use crate::error::NetworkingError;
use crate::models::{PendingGame, WebGame};
use eb_crownfall_engine::{CrownfallPlayerAction, CrownfallTurnResult};
use serde::{Deserialize, Serialize};

pub type GameId = String;
pub type Username = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PendingGameState {
    Pending,
    Joined,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformActionState {
    Done,
    NotYourTurn,
    InvalidGame,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetGameState {
    Active(WebGame),
    InvalidGame,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Packet {
    //send on client start
    LoginRequest(Username),
    //return an active game
    LoginResponse(Option<WebGame>),
    //request list of pending games
    PendingListRequest,
    //return list of pending games
    PendingListResponse(Vec<PendingGame>),
    //create game
    CreateGameRequest,
    //get new game id if it worked
    CreateGameResponse(Option<GameId>),
    //join a game
    JoinGameRequest(GameId),
    //return true if it worked, false means game invalid/already joined
    JoinGameResponse(GameId, bool),
    //get active game state
    PollGameRequest(GameId),
    //return game state
    PollGameResponse(NetGameState),
    //check if a pending game is still pending
    PollPendingGameRequest(GameId),
    //return state of pending game
    PollPendingGameResponse(PendingGameState),
    //sent by server when game changes
    GameUpdateCommand(WebGame, Option<CrownfallTurnResult>),
    //perform action
    PerformActionRequest(GameId, CrownfallPlayerAction),
    //return state of action
    PerformActionResponse(PerformActionState),
    //client has left game
    LeaveGame(GameId),
}

impl Packet {
    pub fn as_bytes(&self) -> Vec<u8> {
        serde_json::to_string(self)
            .unwrap_or_else(|_| panic!("{self:?} serialization failed"))
            .as_bytes()
            .to_vec()
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Packet, NetworkingError> {
        let json_str = String::from_utf8(bytes.clone())
            .map_err(|e| NetworkingError::UtfError(e, format!("Invalid packet: {bytes:?}")))?;
        let packet = serde_json::from_str::<Packet>(&json_str)
            .map_err(|e| NetworkingError::JsonError(e, format!("Invalid packet: {json_str}")))?;
        Ok(packet)
    }
}
