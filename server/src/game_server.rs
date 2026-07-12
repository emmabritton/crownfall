use crate::models::AppState;
use networking::error::NetworkingError;
use networking::packet::{NetGameState, Packet, PerformActionState};
use networking::server::{ServerApp, ServerEvent};
use std::time::Duration;

pub struct GameServer {
    app_state: AppState,
    server_app: ServerApp,
}

impl GameServer {
    pub fn new(app_state: AppState, server_app: ServerApp) -> Self {
        Self {
            app_state,
            server_app,
        }
    }
}

impl GameServer {
    pub fn sleep_duration(&mut self) -> Duration {
        if self.app_state.game_count() == 0 {
            Duration::from_secs(2)
        } else {
            Duration::from_secs_f32(0.5)
        }
    }

    pub fn update(&mut self) -> Result<(), NetworkingError> {
        let events = self.server_app.update()?;

        for event in events {
            match event {
                ServerEvent::Connected(id) => {
                    println!("Client {id} connected")
                }
                ServerEvent::Disconnected(id) => {
                    println!("Client {id} disconnected");
                    self.app_state.client_disconnect(id);
                }
                ServerEvent::Packet(client_id, packet) => {
                    println!("Client {client_id} sent {packet:?}");
                    match packet {
                        Packet::LoginRequest(username) => {
                            let existing_game = self.app_state.client_login(client_id, username);
                            let packet = Packet::LoginResponse(existing_game.map(|sag| sag.game));
                            self.server_app.send_to(client_id, &packet)?;
                        }
                        Packet::PendingListRequest => {
                            let list = self.app_state.pending_list();
                            self.server_app
                                .send_to(client_id, &Packet::PendingListResponse(list))?;
                        }
                        Packet::CreateGameRequest => {
                            let response = self.app_state.create_game(client_id);
                            self.server_app
                                .send_to(client_id, &Packet::CreateGameResponse(response))?;
                        }
                        Packet::JoinGameRequest(game_id) => {
                            match self.app_state.join_game(&game_id, client_id) {
                                Some(active_game) => {
                                    self.server_app.send_to(
                                        client_id,
                                        &Packet::JoinGameResponse(game_id, true),
                                    )?;
                                    let update =
                                        Packet::GameUpdateCommand(active_game.game.clone(), None);
                                    self.server_app.send_to(active_game.white_player, &update)?;
                                }
                                None => {
                                    self.server_app.send_to(
                                        client_id,
                                        &Packet::JoinGameResponse(game_id, false),
                                    )?;
                                }
                            }
                        }
                        Packet::PollGameRequest(game_id) => {
                            let state = match self.app_state.poll_game(&game_id) {
                                Some(game) => NetGameState::Active(game),
                                None => NetGameState::InvalidGame,
                            };
                            self.server_app
                                .send_to(client_id, &Packet::PollGameResponse(state))?;
                        }
                        Packet::PollPendingGameRequest(game_id) => {
                            let state = self.app_state.poll_pending(&game_id);
                            self.server_app
                                .send_to(client_id, &Packet::PollPendingGameResponse(state))?;
                        }
                        Packet::PerformActionRequest(game_id, action) => {
                            match self.app_state.perform_action(&game_id, client_id, action) {
                                Ok((active_game, result)) => {
                                    self.server_app.send_to(
                                        client_id,
                                        &Packet::PerformActionResponse(PerformActionState::Done),
                                    )?;
                                    let update =
                                        Packet::GameUpdateCommand(active_game.game.clone(), result);
                                    self.server_app.send_to(active_game.white_player, &update)?;
                                    self.server_app.send_to(active_game.black_player, &update)?;
                                }
                                Err(state) => {
                                    self.server_app.send_to(
                                        client_id,
                                        &Packet::PerformActionResponse(state),
                                    )?;
                                }
                            }
                        }
                        Packet::LeaveGame(game_id) => {
                            self.app_state.leave_game(&game_id, client_id);
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(())
    }
}
