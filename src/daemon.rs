//! Daemon - manages iroh endpoint, peers, and tunnels.

use crate::peer::PeerManager;
use crate::protocol::{ALPN, ExposedPort, ListInfo, Request, Response};
use anyhow::{Context, Result};
use iroh::{Endpoint, SecretKey};
use std::collections::HashSet;
use std::net::IpAddr;
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::RwLock;
use tracing::{error, info};

#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};

#[cfg(windows)]
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

enum PlatformListener {
    #[cfg(unix)]
    Unix(UnixListener),

    #[cfg(windows)]
    Pipe(String),
}

enum PlatformStream {
    #[cfg(unix)]
    Unix(UnixStream),

    #[cfg(windows)]
    Pipe(NamedPipeServer),
}

impl PlatformListener {
    async fn accept(&self) -> Result<PlatformStream> {
        #[cfg(unix)]
        {
            let (stream, _) = match self {
                PlatformListener::Unix(l) => l.accept().await?,
            };
            Ok(PlatformStream::Unix(stream))
        }

        #[cfg(windows)]
        {
            match self {
                PlatformListener::Pipe(pipe_name) => {
                    let server = ServerOptions::new()
                        .first_pipe_instance(true)
                        .create(pipe_name)?;

                    server.connect().await?;

                    Ok(PlatformStream::Pipe(server))
                }
            }
        }
    }
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
            PlatformStream::Pipe(server) => {
                let (r, w) = tokio::io::split(server);
                (Box::new(r), Box::new(w))
            }
        }
    }
}

async fn create_listener(socket_path: &Path) -> Result<PlatformListener> {
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(socket_path);
        let listener = UnixListener::bind(socket_path)?;
        Ok(PlatformListener::Unix(listener))
    }

    #[cfg(windows)]
    {
        Ok(PlatformListener::Pipe(r"\\.\pipe\iroh_daemon".to_string()))
    }
}

pub struct Daemon {
    endpoint: Endpoint,
    exposed_ports: Arc<RwLock<HashSet<ExposedPort>>>,
    peers: PeerManager,
}

impl Daemon {
    pub async fn new(host: IpAddr) -> Result<Arc<Self>> {
        let secret_key = SecretKey::generate(&mut rand::rng());

        let endpoint = Endpoint::builder(iroh::endpoint::presets::N0)
            .secret_key(secret_key)
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await
            .context("failed to create iroh endpoint")?;

        let exposed_ports = Arc::new(RwLock::new(HashSet::new()));

        Ok(Arc::new(Self {
            peers: PeerManager::new(endpoint.clone(), host, exposed_ports.clone()),
            endpoint,
            exposed_ports,
        }))
    }

    pub fn ticket(&self) -> String {
        self.endpoint.id().to_string()
    }

    pub async fn expose(&self, port: ExposedPort) -> Result<()> {
        self.exposed_ports.write().await.insert(port.clone());
        self.peers
            .broadcast_exposed_ports(self.get_exposed_ports().await)
            .await;
        info!("exposed port remote={} local={}", port.remote, port.local);
        Ok(())
    }


    pub async fn unexpose(&self, port: ExposedPort) -> Result<()> {
        self.exposed_ports.write().await.remove(&port);
        self.peers
            .broadcast_exposed_ports(self.get_exposed_ports().await)
            .await;
        info!("unexposed port remote={} local={}", port.remote, port.local);
        Ok(())
    }


    pub async fn get_exposed_ports(&self) -> Vec<ExposedPort> {
        self.exposed_ports.read().await.iter().cloned().collect()
    }


    pub async fn list(&self) -> ListInfo {
        ListInfo {
            peers: self.peers.list().await,
            exposed_ports: self.get_exposed_ports().await,
            bindings: self.peers.list_bindings().await,
        }
    }

    pub async fn accept_loop(self: Arc<Self>) {
        loop {
            match self.endpoint.accept().await {
                Some(incoming) => {
                    let this = self.clone();
                    tokio::spawn(async move {
                        if let Err(e) = this.handle_incoming(incoming).await {
                            error!("error handling incoming connection: {}", e);
                        }
                    });
                }
                None => {
                    info!("endpoint closed");
                    break;
                }
            }
        }
    }

    async fn handle_incoming(&self, incoming: iroh::endpoint::Incoming) -> Result<()> {
        let conn = incoming.accept()?.await?;
        self.peers.handle_connection(conn).await
    }

    pub async fn handle_request(self: &Arc<Self>, request: Request) -> Response {
        match request {
            Request::AddPeer { ticket } => match self.peers.add_peer(&ticket).await {
                Ok(()) => {
                    let ports = self.get_exposed_ports().await;
                    self.peers.broadcast_exposed_ports(ports).await;
                    Response::Ok
                }
                Err(e) => Response::Error(e.to_string()),
            },
            Request::RemovePeer { ticket } => match self.peers.remove_peer(&ticket).await {
                Ok(()) => Response::Ok,
                Err(e) => Response::Error(e.to_string()),
            },
            Request::Expose { port } => match self.expose(port).await {
                Ok(()) => Response::Ok,
                Err(e) => Response::Error(e.to_string()),
            },
            Request::Unexpose { port } => match self.unexpose(port).await {
                Ok(()) => Response::Ok,
                Err(e) => Response::Error(e.to_string()),
            },
            Request::List => Response::List(self.list().await),
            Request::Ticket => Response::Ticket(self.ticket()),
        }
    }
}

pub async fn run(
    host: IpAddr,
    socket_path: &Path,
    peers: Vec<String>,
    remote: Vec<u16>,
    local: Vec<u16>,
) -> Result<()> {
    #[cfg(unix)]
    let _ = std::fs::remove_file(socket_path);

    let daemon = Daemon::new(host).await?;

    println!("Ticket: {}", daemon.ticket());
    info!("daemon started, host={}", host);

    if !local.is_empty() && local.len() != remote.len() {
        return Err(anyhow::anyhow!(
            "--local must be given the same number of times as --expose"
        ));
    }

    for (i, r) in remote.iter().copied().enumerate() {
        let l = local.get(i).copied().unwrap_or(r);
        daemon.expose(ExposedPort { remote: r, local: l }).await?;
    }


    for ticket in &peers {
        match daemon.peers.add_peer(ticket).await {
            Ok(()) => info!("added peer {}", ticket),
            Err(e) => error!("failed to add peer {}: {}", ticket, e),
        }
    }

    if !peers.is_empty() {
        let ports = daemon.get_exposed_ports().await;
        daemon.peers.broadcast_exposed_ports(ports).await;
    }

    let accept_daemon = daemon.clone();
    tokio::spawn(async move {
        accept_daemon.accept_loop().await;
    });

    let listener = create_listener(socket_path).await?;
    info!("listening on {:?}", socket_path);

    loop {
        let stream = listener.accept().await?;
        let daemon = daemon.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, daemon).await {
                error!("client error: {}", e);
            }
        });
    }
}

async fn handle_client(stream: PlatformStream, daemon: Arc<Daemon>) -> Result<()> {
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    reader.read_line(&mut line).await?;
    let request: Request = serde_json::from_str(&line)?;

    let response = daemon.handle_request(request).await;
    let response_json = serde_json::to_string(&response)?;

    writer.write_all(response_json.as_bytes()).await?;
    writer.write_all(b"\n").await?;

    Ok(())
}
