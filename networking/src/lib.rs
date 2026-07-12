pub mod client;
pub mod error;
mod framed;
pub mod models;
pub mod packet;
pub mod server;

type PacketSize = u16;
const LEN_PREFIX_SIZE: usize = 2; //must be byte count of PacketSize

const BUFFER_SIZE: usize = 4000;
