use networking::client::ClientApp;
use networking::error::NetworkingError;
use networking::packet::Packet;
use std::net::SocketAddr;
use std::sync::{Mutex, OnceLock};

static CLIENT: OnceLock<Mutex<ClientApp>> = OnceLock::new();

pub fn init(server: SocketAddr) -> Result<(), NetworkingError> {
    let client = ClientApp::new(server)?;
    CLIENT.set(Mutex::new(client)).ok();
    Ok(())
}

fn client() -> &'static Mutex<ClientApp> {
    CLIENT.get().expect("net::init was not called before use")
}

pub fn send(packet: Packet) -> Result<(), NetworkingError> {
    client().lock().unwrap().send(packet)
}

pub fn poll() -> Result<Vec<Packet>, NetworkingError> {
    let mut client = client().lock().unwrap();
    client.update()?;
    let packets = client.take_packets();
    for packet in &packets {
        println!("Received: {:?}", packet);
    }
    Ok(packets)
}
