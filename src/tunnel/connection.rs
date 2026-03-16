use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_tungstenite::{
    connect_async, tungstenite::Message, tungstenite::client::IntoClientRequest,
};

use crate::protocol::decoder::MessageDecoder;
use crate::protocol::encoder::MessageEncoder;
use crate::protocol::message::{HttpRequestMessage, ProtocolMessage};
use crate::tunnel::forwarder::HttpForwarder;
use crate::tunnel::heartbeat::PingTracker;

const OUTBOUND_CHANNEL_SIZE: usize = 64;

#[derive(Debug, Error)]
pub enum TunnelError {
    #[error("{0}")]
    FatalClose(String),

    #[error("Tunnel closed by server: {reason} ({code})")]
    TunnelClosed { reason: String, code: String },

    #[error("Connection lost: no heartbeat received within 15 seconds")]
    HeartbeatTimeout,

    #[error("Connection rejected with HTTP {0}")]
    ConnectionFailed(u16),

    #[error("Protocol error: {0}")]
    ProtocolError(String),

    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
}

/// Maps a known fatal WebSocket close reason to a user-friendly error message.
/// Returns None for transient/unknown reasons, which fall through to the reconnect loop.
fn fatal_close_message(reason: &str) -> Option<&'static str> {
    match reason {
        "Tunnel limit reached" => Some(
            "Error: You've reached your active tunnel limit. Close an existing tunnel or upgrade your plan.",
        ),
        "Subdomain not reserved" => Some(
            "Error: Subdomain not reserved. Reserve it first or omit --subdomain for a random subdomain.",
        ),
        "Subdomain reserved by another user" | "Subdomain in use" => {
            Some("Error: That subdomain is not available.")
        }
        "Subdomain in use by your session" => {
            Some("Error: That subdomain is already in use by one of your active tunnels.")
        }
        "Invalid or expired token" => {
            Some("Error: Not authenticated. Run 'hermez login' to re-authenticate.")
        }
        "Invalid subdomain" => Some("Error: Invalid subdomain format."),
        "Subdomain not allowed" => Some("Error: That subdomain is not allowed."),
        "Custom domain not found" => Some(
            "Error: Custom domain not found. Register it first under Custom Domains in the dashboard.",
        ),
        "Custom domain is not active" => Some(
            "Error: Custom domain is not yet active. Complete DNS verification in the dashboard first.",
        ),
        "Custom domain not owned by authenticated user" => {
            Some("Error: This custom domain belongs to a different account.")
        }
        _ => None,
    }
}

/// Metadata returned by the server in the WebSocket upgrade response headers.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TunnelInfo {
    pub tunnel_id: String,
    pub subdomain: String,
    pub public_url: String,
    pub local_port: u16,
}

/// Parameters needed to establish a tunnel connection.
#[allow(dead_code)]
pub struct ConnectionConfig {
    pub token: String,
    pub tunnel_url: String,
    pub local_host: String,
    pub local_port: u16,
    pub subdomain: Option<String>,
    pub custom_domain: Option<String>,
    pub request_timeout: u64,
}

/// An established tunnel connection, ready to run the message loop.
pub struct TunnelConnection {
    pub tunnel_info: TunnelInfo,
    stream: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
}

/// In-progress chunked HTTP request waiting for its END frame.
struct PartialRequest {
    request_id: String,
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl TunnelConnection {
    /// Connect to the tunnel server and extract metadata from the upgrade response.
    pub async fn connect(config: &ConnectionConfig) -> Result<Self, TunnelError> {
        let connect_url = format!("{}/connect", config.tunnel_url.trim_end_matches('/'));

        let mut request = connect_url
            .as_str()
            .into_client_request()
            .expect("failed to build WebSocket request");

        request.headers_mut().insert(
            "Authorization",
            format!("Bearer {}", config.token).parse().unwrap(),
        );
        request
            .headers_mut()
            .insert("X-Hermez-Protocol-Version", "1".parse().unwrap());
        request.headers_mut().insert(
            "X-Hermez-Local-Port",
            config.local_port.to_string().parse().unwrap(),
        );

        // Attach either the subdomain or the custom domain header — mutually exclusive.
        // Omitting both tells the server to assign a random subdomain.
        if let Some(ref subdomain) = config.subdomain {
            request
                .headers_mut()
                .insert("X-Hermez-Subdomain", subdomain.parse().unwrap());
        } else if let Some(ref domain) = config.custom_domain {
            request
                .headers_mut()
                .insert("X-Hermez-Custom-Domain", domain.parse().unwrap());
        }

        let (mut stream, _response) = match connect_async(request).await {
            Ok(result) => result,
            Err(tokio_tungstenite::tungstenite::Error::Http(resp)) => {
                return Err(TunnelError::ConnectionFailed(resp.status().as_u16()));
            }
            Err(e) => return Err(TunnelError::WebSocket(e)),
        };

        // The server sends TUNNEL_CONNECTED as the very first message after registration,
        let tunnel_info = match stream.next().await {
            Some(Ok(Message::Binary(data))) => match MessageDecoder::decode(data.as_ref()) {
                Ok(ProtocolMessage::TunnelConnected {
                    tunnel_id,
                    subdomain,
                    public_url,
                }) => TunnelInfo {
                    tunnel_id,
                    subdomain,
                    public_url,
                    local_port: config.local_port,
                },
                Ok(_) => {
                    return Err(TunnelError::ProtocolError(
                        "Expected TUNNEL_CONNECTED as first message from server".to_string(),
                    ));
                }
                Err(e) => return Err(TunnelError::ProtocolError(e.to_string())),
            },
            Some(Ok(Message::Close(frame))) => {
                let reason = frame.as_ref().map(|f| f.reason.as_ref()).unwrap_or("");
                if let Some(fatal_msg) = fatal_close_message(reason) {
                    return Err(TunnelError::FatalClose(fatal_msg.to_string()));
                }
                return Err(TunnelError::ConnectionFailed(0));
            }
            None => {
                return Err(TunnelError::ConnectionFailed(0));
            }
            Some(Err(e)) => return Err(TunnelError::WebSocket(e)),
            _ => {
                return Err(TunnelError::ProtocolError(
                    "Unexpected first message type from server".to_string(),
                ));
            }
        };

        Ok(Self {
            tunnel_info,
            stream,
        })
    }

    pub fn tunnel_info(&self) -> &TunnelInfo {
        &self.tunnel_info
    }

    /// Drive the message loop until the tunnel closes, heartbeat times out,
    /// or the user presses Ctrl+C.
    pub async fn run(self, forwarder: Arc<HttpForwarder>) -> Result<(), TunnelError> {
        let (mut write, mut read) = self.stream.split();
        let (tx, mut rx) = mpsc::channel::<Message>(OUTBOUND_CHANNEL_SIZE);

        // Dedicated writer task: drains the outbound channel into the WebSocket sink.
        let writer_handle = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if write.send(msg).await.is_err() {
                    break;
                }
            }
        });

        let mut ping_tracker = PingTracker::new();
        let mut pending_chunks: HashMap<String, PartialRequest> = HashMap::new();
        let mut heartbeat_check = tokio::time::interval(Duration::from_secs(1));

        let result = loop {
            tokio::select! {
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Binary(data))) => {
                            match MessageDecoder::decode(data.as_ref()) {
                                Ok(decoded) => {
                                    if let Err(e) = handle_message(
                                        decoded,
                                        &tx,
                                        &forwarder,
                                        &mut ping_tracker,
                                        &mut pending_chunks,
                                    )
                                    .await
                                    {
                                        break Err(e);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Protocol decode error: {}", e);
                                }
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => {
                            break Ok(());
                        }
                        _ => {}
                    }
                }
                _ = heartbeat_check.tick() => {
                    if ping_tracker.is_stale() {
                        break Err(TunnelError::HeartbeatTimeout);
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    let _ = tx.send(Message::Close(None)).await;
                    break Ok(());
                }
            }
        };

        // Drop tx so the writer task's rx.recv() returns None and it exits cleanly.
        drop(tx);
        let _ = writer_handle.await;
        result
    }
}

/// Dispatch one decoded inbound message.
async fn handle_message(
    message: ProtocolMessage,
    tx: &mpsc::Sender<Message>,
    forwarder: &Arc<HttpForwarder>,
    ping_tracker: &mut PingTracker,
    pending_chunks: &mut HashMap<String, PartialRequest>,
) -> Result<(), TunnelError> {
    match message {
        ProtocolMessage::Ping => {
            ping_tracker.record_ping();
            let pong = MessageEncoder::encode_pong();
            let _ = tx.send(Message::Binary(pong.into())).await;
        }

        ProtocolMessage::HttpRequest(request) => {
            spawn_forward(request, tx.clone(), Arc::clone(forwarder));
        }

        ProtocolMessage::HttpRequestStart {
            request_id,
            method,
            path,
            headers,
        } => {
            pending_chunks.insert(
                request_id.clone(),
                PartialRequest {
                    request_id,
                    method,
                    path,
                    headers,
                    body: Vec::new(),
                },
            );
        }

        ProtocolMessage::HttpRequestChunk { request_id, data } => {
            if let Some(partial) = pending_chunks.get_mut(&request_id) {
                partial.body.extend_from_slice(&data);
            }
        }

        ProtocolMessage::HttpRequestEnd { request_id } => {
            if let Some(partial) = pending_chunks.remove(&request_id) {
                let request = HttpRequestMessage {
                    request_id: partial.request_id,
                    method: partial.method,
                    path: partial.path,
                    headers: partial.headers,
                    body: partial.body,
                };
                spawn_forward(request, tx.clone(), Arc::clone(forwarder));
            }
        }

        ProtocolMessage::TunnelClose { reason, code } => {
            return Err(TunnelError::TunnelClosed { reason, code });
        }

        ProtocolMessage::Error { message, .. } => {
            eprintln!("Server error: {}", message);
        }

        // Pong and outbound response types should never be received by the CLI.
        _ => {}
    }

    Ok(())
}

/// Spawn a task to forward one HTTP request and send the response frames back.
fn spawn_forward(
    request: HttpRequestMessage,
    tx: mpsc::Sender<Message>,
    forwarder: Arc<HttpForwarder>,
) {
    tokio::spawn(async move {
        let started = Instant::now();
        let method = request.method.clone();
        let path = request.path.clone();

        let response = forwarder.forward(request).await;
        let status = response.status_code;

        for frame in MessageEncoder::encode_response(&response) {
            if tx.send(Message::Binary(frame.into())).await.is_err() {
                return; // tunnel closed while forwarding
            }
        }

        crate::display::request_log::log_request(&method, &path, status, started);
    });
}
