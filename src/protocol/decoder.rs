use byteorder::{BigEndian, ReadBytesExt};
use std::io::{self, Cursor, Read};

use crate::protocol::message::{HttpRequestMessage, ProtocolMessage};

/// Binary protocol decoding Errors.
/// Converts to HermezError::Protocol at the tunnel boundary.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("Frame too short to be valid")]
    TooShort,

    #[error("Frame payload is incomplete")]
    IncompleteMessage,

    #[error("Unknown message type: 0x{0:02X}")]
    UnknownType(u8),

    #[error("Invalid UTF-8 in string field")]
    InvalidUtf8,

    #[error("IO error reading frame: {0}")]
    Io(#[from] io::Error),

    #[error("Invalid JSON payload: {0}")]
    InvalidJson(String),
}

pub struct MessageDecoder;

impl MessageDecoder {
    /// Decode a raw WebSocket binary frame into a ProtocolMessage.
    pub fn decode(data: &[u8]) -> Result<ProtocolMessage, DecodeError> {
        if data.len() < 5 {
            return Err(DecodeError::TooShort);
        }

        let mut cursor = Cursor::new(data);
        let length = cursor.read_u32::<BigEndian>()? as usize;
        let message_type = cursor.read_u8()?;

        if data.len() < 4 + length {
            return Err(DecodeError::IncompleteMessage);
        }

        match message_type {
            0x01 => Ok(ProtocolMessage::Ping),
            0x10 => Self::decode_http_request(&mut cursor),
            0x12 => Self::decode_http_request_start(&mut cursor),
            0x13 => Self::decode_http_request_chunk(&mut cursor),
            0x14 => Self::decode_http_request_end(&mut cursor),
            0x20 => Self::decode_tunnel_close(&mut cursor, length),
            0xFF => Self::decode_error(&mut cursor, length),
            other => Err(DecodeError::UnknownType(other)),
        }
    }

    // private decoder functions

    fn decode_http_request(cursor: &mut Cursor<&[u8]>) -> Result<ProtocolMessage, DecodeError> {
        let request_id = read_request_id(cursor)?;

        let method_len = cursor.read_u8()? as usize;
        let method = read_string(cursor, method_len)?;

        let path_len = cursor.read_u16::<BigEndian>()? as usize;
        let path = read_string(cursor, path_len)?;

        let headers = read_headers(cursor)?;

        let body_len = cursor.read_u32::<BigEndian>()? as usize;
        let mut body = vec![0u8; body_len];
        cursor.read_exact(&mut body)?;

        Ok(ProtocolMessage::HttpRequest(HttpRequestMessage {
            request_id,
            method,
            path,
            headers,
            body,
        }))
    }

    fn decode_http_request_start(
        cursor: &mut Cursor<&[u8]>,
    ) -> Result<ProtocolMessage, DecodeError> {
        let request_id = read_request_id(cursor)?;

        let method_len = cursor.read_u8()? as usize;
        let method = read_string(cursor, method_len)?;

        let path_len = cursor.read_u16::<BigEndian>()? as usize;
        let path = read_string(cursor, path_len)?;

        let headers = read_headers(cursor)?;

        Ok(ProtocolMessage::HttpRequestStart {
            request_id,
            method,
            path,
            headers,
        })
    }

    fn decode_http_request_chunk(
        cursor: &mut Cursor<&[u8]>,
    ) -> Result<ProtocolMessage, DecodeError> {
        let request_id = read_request_id(cursor)?;
        let chunk_len = cursor.read_u32::<BigEndian>()? as usize;
        let mut data = vec![0u8; chunk_len];
        cursor.read_exact(&mut data)?;

        Ok(ProtocolMessage::HttpRequestChunk { request_id, data })
    }

    fn decode_http_request_end(cursor: &mut Cursor<&[u8]>) -> Result<ProtocolMessage, DecodeError> {
        let request_id = read_request_id(cursor)?;
        Ok(ProtocolMessage::HttpRequestEnd { request_id })
    }

    fn decode_tunnel_close(
        cursor: &mut Cursor<&[u8]>,
        length: usize,
    ) -> Result<ProtocolMessage, DecodeError> {
        // Payload is raw JSON: {"reason":"...","code":"..."}
        // length includes the type byte, so JSON is length - 1 bytes
        let json_len = length - 1;
        let mut json_bytes = vec![0u8; json_len];
        cursor.read_exact(&mut json_bytes)?;

        let v: serde_json::Value = serde_json::from_slice(&json_bytes)
            .map_err(|e| DecodeError::InvalidJson(e.to_string()))?;

        let reason = v["reason"].as_str().unwrap_or("Unknown reason").to_string();
        let code = v["code"].as_str().unwrap_or("unknown").to_string();

        Ok(ProtocolMessage::TunnelClose { reason, code })
    }

    fn decode_error(
        cursor: &mut Cursor<&[u8]>,
        length: usize,
    ) -> Result<ProtocolMessage, DecodeError> {
        // Payload is raw JSON: {"code":"...","message":"...","request_id":"..."|null}
        let json_len = length - 1;
        let mut json_bytes = vec![0u8; json_len];
        cursor.read_exact(&mut json_bytes)?;

        let v: serde_json::Value = serde_json::from_slice(&json_bytes)
            .map_err(|e| DecodeError::InvalidJson(e.to_string()))?;

        let code = v["code"].as_str().unwrap_or("unknown").to_string();
        let message = v["message"].as_str().unwrap_or("Unknown error").to_string();
        let request_id = v["request_id"].as_str().map(|s| s.to_string());

        Ok(ProtocolMessage::Error {
            code,
            message,
            request_id,
        })
    }
}

// Shared read helpers

/// Read the 32-byte ASCII hex request ID and return as String.
fn read_request_id(cursor: &mut Cursor<&[u8]>) -> Result<String, DecodeError> {
    let mut bytes = [0u8; 32];
    cursor.read_exact(&mut bytes)?;
    String::from_utf8(bytes.to_vec()).map_err(|_| DecodeError::InvalidUtf8)
}

/// Read `len` bytes and decode as UTF-8 string.
fn read_string(cursor: &mut Cursor<&[u8]>, len: usize) -> Result<String, DecodeError> {
    let mut bytes = vec![0u8; len];
    cursor.read_exact(&mut bytes)?;
    String::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8)
}

/// Read headers: [count: u16] then for each: [name_len: u16][name][value_len: u16][value]
fn read_headers(cursor: &mut Cursor<&[u8]>) -> Result<Vec<(String, String)>, DecodeError> {
    let count = cursor.read_u16::<BigEndian>()? as usize;
    let mut headers = Vec::with_capacity(count);

    for _ in 0..count {
        let name_len = cursor.read_u16::<BigEndian>()? as usize;
        let name = read_string(cursor, name_len)?;

        let value_len = cursor.read_u16::<BigEndian>()? as usize;
        let value = read_string(cursor, value_len)?;

        headers.push((name, value));
    }

    Ok(headers)
}
