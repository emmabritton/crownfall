use chrono::Utc;
use eb_crownfall_engine::{
    CrownfallGame, CrownfallPlayerAction, CrownfallPlayerKind, CrownfallTurnResult,
};
use networking::models::{PendingGame, WebGame};
use networking::packet::{GameId, PendingGameState, PerformActionState, Username};
use networking::server::ClientId;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct AppState {
    pending: HashMap<String, ServerPendingGame>,
    games: HashMap<String, ServerActiveGame>,
    max_games: usize,
    names: HashMap<ClientId, Username>,
    ids: HashMap<Username, ClientId>,
}

impl AppState {
    pub fn new(game_count: usize) -> Self {
        Self {
            pending: HashMap::with_capacity(game_count),
            games: HashMap::with_capacity(game_count),
            max_games: game_count,
            names: HashMap::new(),
            ids: HashMap::new(),
        }
    }

    pub fn game_count(&self) -> usize {
        self.pending.len() + self.games.len()
    }

    pub fn client_login(
        &mut self,
        client_id: ClientId,
        username: Username,
    ) -> Option<ServerActiveGame> {
        let old_client_id = self.ids.insert(username.clone(), client_id);

        if let Some(old_client_id) = old_client_id
            && old_client_id != client_id
        {
            self.pending
                .retain(|_, game| game.white_player != old_client_id);

            for game in self.games.values_mut() {
                if game.white_player == old_client_id {
                    game.white_player = client_id;
                }
                if game.black_player == old_client_id {
                    game.black_player = client_id;
                }
            }

            self.names.remove(&old_client_id);
        }

        self.names.insert(client_id, username);

        self.games
            .values()
            .find(|game| game.white_player == client_id || game.black_player == client_id)
            .cloned()
    }

    pub fn client_disconnect(&mut self, client_id: ClientId) {
        self.pending
            .retain(|_, game| game.white_player != client_id);
        self.ids.retain(|_, id| *id != client_id);
    }

    pub fn pending_list(&self) -> Vec<PendingGame> {
        self.pending
            .iter()
            .map(|(id, pending)| PendingGame {
                id: id.clone(),
                white_player_name: self
                    .names
                    .get(&pending.white_player)
                    .cloned()
                    .unwrap_or_default(),
                created: pending.created,
            })
            .collect()
    }

    pub fn create_game(&mut self, client_id: ClientId) -> Option<GameId> {
        if self.game_count() >= self.max_games {
            return None;
        }

        let id = Uuid::new_v4().to_string();
        self.pending.insert(
            id.clone(),
            ServerPendingGame {
                white_player: client_id,
                created: Utc::now(),
            },
        );
        Some(id)
    }

    pub fn join_game(&mut self, id: &str, client_id: ClientId) -> Option<ServerActiveGame> {
        let username = self.names.get(&client_id)?.clone();
        let pending = self.pending.remove(id)?;
        let white_player_name = self
            .names
            .get(&pending.white_player)
            .cloned()
            .unwrap_or_default();

        let active_game = ServerActiveGame {
            game: WebGame {
                id: id.to_string(),
                game: CrownfallGame::default(),
                white_player_name,
                black_player_name: username,
            },
            white_player: pending.white_player,
            black_player: client_id,
        };
        self.games.insert(id.to_string(), active_game.clone());
        Some(active_game)
    }

    pub fn poll_game(&self, id: &str) -> Option<WebGame> {
        self.games
            .get(id)
            .map(|active_game| active_game.game.clone())
    }

    pub fn poll_pending(&self, id: &str) -> PendingGameState {
        if self.pending.contains_key(id) {
            PendingGameState::Pending
        } else if self.games.contains_key(id) {
            PendingGameState::Joined
        } else {
            PendingGameState::Invalid
        }
    }

    pub fn perform_action(
        &mut self,
        id: &str,
        client_id: ClientId,
        action: CrownfallPlayerAction,
    ) -> Result<(ServerActiveGame, Option<CrownfallTurnResult>), PerformActionState> {
        let active_game = self
            .games
            .get_mut(id)
            .ok_or(PerformActionState::InvalidGame)?;

        let expected_client = match action.player() {
            CrownfallPlayerKind::White => active_game.white_player,
            CrownfallPlayerKind::Black => active_game.black_player,
        };
        if expected_client != client_id {
            return Err(PerformActionState::NotYourTurn);
        }

        match active_game.game.game.clone().handle_player_action(action) {
            Ok((game, result)) => {
                active_game.game.game = game;
                Ok((active_game.clone(), result))
            }
            Err(_) => Err(PerformActionState::NotYourTurn),
        }
    }

    pub fn leave_game(&mut self, id: &str, client_id: ClientId) {
        if let Some(pending) = self.pending.get(id) {
            if pending.white_player == client_id {
                self.pending.remove(id);
            }
            return;
        }

        if let Some(active_game) = self.games.get(id) {
            let (other_client, other_name) = if active_game.white_player == client_id {
                (
                    active_game.black_player,
                    &active_game.game.black_player_name,
                )
            } else if active_game.black_player == client_id {
                (
                    active_game.white_player,
                    &active_game.game.white_player_name,
                )
            } else {
                return;
            };

            let other_connected = self.ids.get(other_name) == Some(&other_client);
            if !other_connected {
                self.games.remove(id);
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct ServerActiveGame {
    pub game: WebGame,
    pub white_player: ClientId,
    pub black_player: ClientId,
}

#[derive(Clone, Debug)]
pub struct ServerPendingGame {
    pub white_player: ClientId,
    pub created: chrono::DateTime<Utc>,
}
