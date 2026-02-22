use crate::protocol::message::{HttpRequestMessage, HttpResponseMessage};
use reqwest::Client;
use std::time::Duration;

/// Strip these from local server responses before forwarding back through the tunnel.
const STRIP_RESPONSE_HEADERS: &[&str] = &["transfer-encoding", "connection", "keep-alive"];

pub struct HttpForwarder {
    client: Client,
    target_host: String,
    target_port: u16,
}

impl HttpForwarder {
    pub fn new(host: String, port: u16, timeout_secs: u64) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client,
            target_host: host,
            target_port: port,
        }
    }

    /// Forward a tunneled request to localhost. Never returns Err —
    /// connection/timeout errors are mapped to 502/504/500 responses.
    pub async fn forward(&self, request: HttpRequestMessage) -> HttpResponseMessage {
        let url = format!(
            "http://{}:{}{}",
            self.target_host, self.target_port, request.path
        );

        let method = request
            .method
            .parse::<reqwest::Method>()
            .unwrap_or(reqwest::Method::GET);

        let mut builder = self.client.request(method, &url);

        for (name, value) in &request.headers {
            builder = builder.header(name.as_str(), value.as_str());
        }

        if !request.body.is_empty() {
            builder = builder.body(request.body.clone());
        }

        match builder.send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let headers = resp
                    .headers()
                    .iter()
                    .filter(|(name, _)| !STRIP_RESPONSE_HEADERS.contains(&name.as_str()))
                    .filter_map(|(name, value)| {
                        value
                            .to_str()
                            .ok()
                            .map(|v| (name.to_string(), v.to_string()))
                    })
                    .collect();
                let body = resp.bytes().await.unwrap_or_default().to_vec();

                HttpResponseMessage {
                    request_id: request.request_id,
                    status_code: status,
                    headers,
                    body,
                }
            }
            Err(e) if e.is_connect() => self.error_response(request.request_id, 502),
            Err(e) if e.is_timeout() => self.error_response(request.request_id, 504),
            Err(_) => self.error_response(request.request_id, 500),
        }
    }

    fn error_response(&self, request_id: String, status: u16) -> HttpResponseMessage {
        let body: &[u8] = match status {
            502 => b"Bad Gateway: local server unreachable",
            504 => b"Gateway Timeout: local server did not respond in time",
            _ => b"Internal Server Error",
        };

        HttpResponseMessage {
            request_id,
            status_code: status,
            headers: vec![("content-type".to_string(), "text/plain".to_string())],
            body: body.to_vec(),
        }
    }
}
