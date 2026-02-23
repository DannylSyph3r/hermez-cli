/// CLI receives: Ping, HttpRequest, HttpRequestStart/Chunk/End, TunnelClose, Error
/// CLI sends:   Pong, HttpResponse, HttpResponseStart/Chunk/End

#[derive(Debug)]
#[allow(dead_code)]
pub enum ProtocolMessage {
    Ping,
    Pong,
    HttpRequest(HttpRequestMessage),
    HttpResponse(HttpResponseMessage),
    HttpRequestStart {
        request_id: String,
        method: String,
        path: String,
        headers: Vec<(String, String)>,
    },
    HttpRequestChunk {
        request_id: String,
        data: Vec<u8>,
    },
    HttpRequestEnd {
        request_id: String,
    },
    HttpResponseStart {
        request_id: String,
        status_code: u16,
        headers: Vec<(String, String)>,
    },
    HttpResponseChunk {
        request_id: String,
        data: Vec<u8>,
    },
    HttpResponseEnd {
        request_id: String,
    },
    TunnelClose {
        reason: String,
        code: String,
    },
    Error {
        code: String,
        message: String,
        request_id: Option<String>,
    },
}

/// An HTTP request received from the server to forward to localhost.
#[derive(Debug)]
pub struct HttpRequestMessage {
    pub request_id: String,
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// An HTTP response from localhost to send back through the tunnel.
#[derive(Debug)]
pub struct HttpResponseMessage {
    pub request_id: String,
    pub status_code: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}
