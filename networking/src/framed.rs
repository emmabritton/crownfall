use crate::error::NetworkingError;
use crate::packet::Packet;
use crate::{BUFFER_SIZE, LEN_PREFIX_SIZE, PacketSize};
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;

pub struct Framed {
    socket: TcpStream,
    scratch: [u8; BUFFER_SIZE],
    read_buf: Vec<u8>,
}

impl Framed {
    pub fn new(socket: TcpStream) -> Result<Self, NetworkingError> {
        socket
            .set_nonblocking(true)
            .map_err(|e| NetworkingError::IoError(e, "Setting nonblocking".to_string()))?;
        Ok(Self {
            socket,
            scratch: [0; BUFFER_SIZE],
            read_buf: Vec::new(),
        })
    }

    pub fn poll(&mut self) -> Result<Vec<Packet>, NetworkingError> {
        let mut disconnected = false;
        loop {
            match self.socket.read(&mut self.scratch) {
                Ok(0) => {
                    disconnected = true;
                    break;
                }
                Ok(len) => self.read_buf.extend_from_slice(&self.scratch[..len]),
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) => return Err(NetworkingError::IoError(e, "reading".into())),
            }
        }

        let mut packets = Vec::new();
        loop {
            if self.read_buf.len() < LEN_PREFIX_SIZE {
                break;
            }
            let body_len =
                PacketSize::from_be_bytes(self.read_buf[..LEN_PREFIX_SIZE].try_into().unwrap())
                    as usize;
            let total_len = LEN_PREFIX_SIZE + body_len;
            if self.read_buf.len() < total_len {
                break;
            }
            packets.push(Packet::from_bytes(
                self.read_buf[LEN_PREFIX_SIZE..total_len].to_vec(),
            )?);
            self.read_buf.drain(..total_len);
        }

        if disconnected {
            return if packets.is_empty() {
                Err(NetworkingError::Disconnected)
            } else {
                Ok(packets)
            };
        }
        Ok(packets)
    }

    pub fn send(&mut self, packet: &Packet) -> Result<(), NetworkingError> {
        let bytes = packet.as_bytes();
        let len_bytes = (bytes.len() as PacketSize).to_be_bytes();
        self.socket
            .write_all(&len_bytes)
            .and_then(|_| self.socket.write_all(&bytes))
            .map_err(|e| NetworkingError::IoError(e, format!("sending {packet:?}")))
    }
}
