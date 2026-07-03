//! Protocol definitions for daemon<->client and peer<->peer communication.

use serde::{Deserialize, Serialize};

/// ALPN protocol identifier
pub const ALPN: &[u8] = b"PAI_SHO/1";

// ============================================================================
// Client <-> Daemon (over Unix socket)
// ============================================================================

/// Request from CLI client to daemon
#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    AddPeer { ticket: String },
    RemovePeer { ticket: String },
    Expose { port: ExposedPort },
    Unexpose { port: ExposedPort },
    List,
    Ticket,
}

/// Response from daemon to CLI client
#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Ok,
    Ticket(String),
    List(ListInfo),
    Error(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListInfo {
    pub peers: Vec<PeerInfo>,
    pub exposed_ports: Vec<ExposedPort>,
    pub bindings: Vec<BindingInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PeerInfo {
    pub endpoint_id: String,
    pub connected: bool,
    pub exposed_ports: Vec<ExposedPort>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BindingInfo {
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ExposedPort {
    pub remote: u16,
    pub local: u16,
}

// ============================================================================
// Peer <-> Peer (over iroh QUIC)
// ============================================================================

/// Message sent between peers over iroh
#[derive(Debug, Serialize, Deserialize)]
pub enum PeerMessage {
    /// Announce exposed ports (sent on connect and when ports change)
    ExposedPorts(Vec<ExposedPort>),
    /// Request to connect to a specific port
    Connect { port: u16 },
    /// Error response
    Error(String),
}
