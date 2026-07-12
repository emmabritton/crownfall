mod game_server;
mod models;
mod timing;

use crate::game_server::GameServer;
use crate::models::AppState;
use networking::error::NetworkingError;
use networking::server::ServerApp;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::thread;

fn init() -> Result<ServerApp, NetworkingError> {
    let port: u16 = std::env::var("PORT")
        .unwrap_or("3000".to_string())
        .parse()
        .expect("PORT must be a number");

    let addr = if let Ok(addr) = std::env::var("ADDR") {
        let addr = IpAddr::from_str(&addr).map_err(|e| NetworkingError::InvalidAddress(e, addr))?;
        SocketAddr::new(addr, port)
    } else {
        SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 0], port))
    };

    let server_client = ServerApp::bind(addr)?;

    println!("Server listening on {}", addr);

    Ok(server_client)
}

fn main() -> Result<(), NetworkingError> {
    let max_games: usize = std::env::var("MAX_GAMES")
        .unwrap_or("10".to_string())
        .parse()
        .unwrap();

    let server_app = init()?;
    let app_state = AppState::new(max_games);
    let mut server = GameServer::new(app_state, server_app);

    loop {
        server.update()?;

        thread::sleep(server.sleep_duration());
    }
}
