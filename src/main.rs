use anyhow::Result;
use clap::{Parser, Subcommand};
use std::net::IpAddr;

mod client;
mod daemon;
mod peer;
mod protocol;
mod tunnel;

#[derive(Parser)]
#[clap(
    name = "pai-sho",
    about = "What happens when you want dumbpipe to stay running, handle a few ports at once, and reconnect when your laptop wakes up",
    version
)]
struct Cli {
    /// Path to Unix socket
    #[arg(long, default_value = "/tmp/pai-sho.sock")]
    socket: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Start the daemon
    Daemon {
        /// Host address for forwarding exposed ports
        #[arg(long, default_value = "127.0.0.1")]
        host: IpAddr,
        /// Add peer(s) on startup
        #[arg(short = 'a', long = "add")]
        peers: Vec<String>,
        /// Expose port(s) on startup
        #[arg(short = 'e', long = "expose")]
        remote: Vec<u16>,
        /// Local Ports (optional, same number as --expose)
        #[arg(long)]
        local: Vec<u16>,
    },

    /// Add a peer (returns assigned IP)
    AddPeer {
        /// Peer's ticket (endpoint ID)
        ticket: String,
    },

    /// Remove a peer
    RemovePeer {
        /// Peer's ticket
        ticket: String,
    },

    /// Expose a port to peers
    Expose {
        /// Remote port
        remote: u16,

        /// Optional local Port (default = remote)
        #[arg(long)]
        local: Option<u16>,
    },

    /// Stop exposing a port
    Unexpose {
        remote: u16,
        #[arg(long)]
        local: Option<u16>,
    },

    /// List peers, exposed ports, and bindings
    List,

    /// Print daemon's ticket
    Ticket,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("pai_sho=info".parse()?),
        )
        .init();

    let cli = Cli::parse();

    let socket_path = std::path::Path::new(&cli.socket);

    match cli.command {
        Command::Daemon { host, peers, remote, local } => {
            daemon::run(host, socket_path, peers, remote, local).await?;
        }
        _ => {
            client::send_command(socket_path, cli.command).await?;
        }
    }

    Ok(())
}
