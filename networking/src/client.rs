use crate::error::NetworkingError;
use crate::framed::Framed;
use crate::packet::Packet;
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

pub struct ClientApp {
    framed: Framed,
    last_packet: Instant,
    packets: Vec<Packet>,
}

impl ClientApp {
    pub fn new(server: SocketAddr) -> Result<Self, NetworkingError> {
        let socket = TcpStream::connect_timeout(&server, Duration::from_secs(10))
            .map_err(|e| NetworkingError::IoError(e, format!("Connecting to {server}")))?;

        println!("Connected to {server}");

        let framed = Framed::new(socket)?;

        Ok(Self {
            framed,
            last_packet: Instant::now(),
            packets: Vec::new(),
        })
    }
}

impl ClientApp {
    pub fn ms_since_last_packet(&self) -> u128 {
        (Instant::now() - self.last_packet).as_millis()
    }

    pub fn send(&mut self, packet: Packet) -> Result<(), NetworkingError> {
        self.framed.send(&packet)
    }

    pub fn update(&mut self) -> Result<(), NetworkingError> {
        let new_packets = self.framed.poll()?;
        if !new_packets.is_empty() {
            self.last_packet = Instant::now();
        }
        self.packets.extend(new_packets);

        Ok(())
    }

    pub fn take_packets(&mut self) -> Vec<Packet> {
        std::mem::take(&mut self.packets)
    }
}
