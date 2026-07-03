//! CLI client - sends commands to daemon over Unix socket or Windows named pipe.

use crate::protocol::{ExposedPort, Request, Response};
use crate::Command;
use anyhow::{Context, Result};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[cfg(unix)]
use tokio::net::UnixStream;

#[cfg(windows)]
use tokio::net::windows::named_pipe::ClientOptions;

enum PlatformStream {
    #[cfg(unix)]
    Unix(UnixStream),

    #[cfg(windows)]
    Pipe(tokio::net::windows::named_pipe::NamedPipeClient),
}

impl PlatformStream {
    fn split(
        self,
    ) -> (
        Box<dyn tokio::io::AsyncRead + Send + Unpin>,
        Box<dyn tokio::io::AsyncWrite + Send + Unpin>,
    ) {
        match self {
            #[cfg(unix)]
            PlatformStream::Unix(stream) => {
                let (r, w) = stream.into_split();
                (Box::new(r), Box::new(w))
            }

            #[cfg(windows)]
            PlatformStream::Pipe(client) => {
                let (r, w) = tokio::io::split(client);
                (Box::new(r), Box::new(w))
            }
        }
    }
}

async fn connect_to_daemon(socket_path: &Path) -> Result<PlatformStream> {
    #[cfg(unix)]
    {
        let stream = UnixStream::connect(socket_path)
            .await
            .context("failed to connect to daemon, is it running?")?;

        Ok(PlatformStream::Unix(stream))
    }

    #[cfg(windows)]
    {
        let pipe_name = r"\\.\pipe\iroh_daemon";

        let client = ClientOptions::new()
            .open(pipe_name)
            .context("failed to connect to daemon pipe, is it running?")?;

        Ok(PlatformStream::Pipe(client))
    }
}

pub async fn send_command(socket_path: &Path, command: Command) -> Result<()> {
    let request = match command {
        Command::AddPeer { ticket } => Request::AddPeer { ticket },
        Command::RemovePeer { ticket } => Request::RemovePeer { ticket },
        Command::Expose { remote, local } => {
            let port = ExposedPort {
                remote,
                local: local.unwrap_or(remote),
            };
            Request::Expose { port }
        }

        Command::Unexpose { remote, local } => {
            let port = ExposedPort {
                remote,
                local: local.unwrap_or(remote),
            };
            Request::Unexpose { port }
        }

        Command::List => Request::List,
        Command::Ticket => Request::Ticket,
        Command::Daemon { .. } => unreachable!("daemon handled separately"),
    };

    let stream = connect_to_daemon(socket_path).await?;

    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);

    let request_json = serde_json::to_string(&request)?;
    writer.write_all(request_json.as_bytes()).await?;
    writer.write_all(b"\n").await?;

    let mut line = String::new();
    reader.read_line(&mut line).await?;
    let response: Response = serde_json::from_str(&line)?;

    match response {
        Response::Ok => println!("OK"),
        Response::Ticket(ticket) => println!("{}", ticket),
        Response::List(info) => {
            println!("PEERS:");
            for peer in &info.peers {
                let status = if peer.connected { "connected" } else { "disconnected" };
                println!("  {} ({}) - ports:", peer.endpoint_id, status);
                for p in &peer.exposed_ports {
                    println!("      remote {} -> local {}", p.remote, p.local);
                }
            }

            println!("\nEXPOSED PORTS:");
            for p in &info.exposed_ports {
                println!("  remote {} -> local {}", p.remote, p.local);
            }

            println!("\nBINDINGS:");
            for binding in &info.bindings {
                println!("  127.0.0.1:{}", binding.port);
            }
        }
        Response::Error(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

pub fn parse_exposed_port(s: &str) -> Result<ExposedPort, String> {
    if let Some((remote, local)) = s.split_once(':') {
        let remote: u16 = remote.parse().map_err(|_| "invalid remote port")?;
        let local: u16 = local.parse().map_err(|_| "invalid local port")?;
        Ok(ExposedPort { remote, local })
    } else {
        let remote: u16 = s.parse().map_err(|_| "invalid port")?;
        Ok(ExposedPort { remote, local: remote })
    }
}
