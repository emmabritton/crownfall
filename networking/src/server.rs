use crate::error::NetworkingError;
use crate::framed::Framed;
use crate::packet::Packet;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::{SocketAddr, TcpListener};

pub type ClientId = u64;

pub struct ServerApp {
    listener: TcpListener,
    next_id: ClientId,
    connections: HashMap<ClientId, Framed>,
}

pub enum ServerEvent {
    Connected(ClientId),
    Disconnected(ClientId),
    Packet(ClientId, Packet),
}

impl ServerApp {
    pub fn bind(addr: SocketAddr) -> Result<Self, NetworkingError> {
        let listener = TcpListener::bind(addr)
            .map_err(|e| NetworkingError::IoError(e, format!("binding {addr}")))?;
        listener
            .set_nonblocking(true)
            .map_err(|e| NetworkingError::IoError(e, "setting nonblocking".to_string()))?;
        Ok(Self {
            listener,
            next_id: 0,
            connections: HashMap::new(),
        })
    }

    pub fn update(&mut self) -> Result<Vec<ServerEvent>, NetworkingError> {
        let mut events = Vec::new();

        loop {
            match self.listener.accept() {
                Ok((socket, _addr)) => {
                    let framed = Framed::new(socket)?;
                    let id = self.next_id;
                    self.next_id += 1;
                    self.connections.insert(id, framed);
                    events.push(ServerEvent::Connected(id));
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) => return Err(NetworkingError::IoError(e, "accepting connection".into())),
            }
        }

        let mut dead = Vec::new();
        for (&id, conn) in self.connections.iter_mut() {
            match conn.poll() {
                Ok(packets) => {
                    events.extend(packets.into_iter().map(|p| ServerEvent::Packet(id, p)));
                }
                Err(NetworkingError::Disconnected) => dead.push(id),
                Err(e) => return Err(e), // or push a per-connection error event instead of bailing the whole server
            }
        }
        for id in dead {
            self.connections.remove(&id);
            events.push(ServerEvent::Disconnected(id));
        }

        Ok(events)
    }

    pub fn send_to(&mut self, id: ClientId, packet: &Packet) -> Result<(), NetworkingError> {
        match self.connections.get_mut(&id) {
            Some(conn) => conn.send(packet),
            None => Err(NetworkingError::Disconnected),
        }
    }

    pub fn broadcast(&mut self, packet: &Packet) {
        for conn in self.connections.values_mut() {
            let _ = conn.send(packet); // decide: log and drop, or collect failures
        }
    }
}
