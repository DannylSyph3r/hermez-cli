use byteorder::{BigEndian, WriteBytesExt};

use crate::protocol::message::HttpResponseMessage;

// Match backend constants exactly
const CHUNK_THRESHOLD: usize = 64 * 1024;
const CHUNK_SIZE: usize = 32 * 1024;

pub struct MessageEncoder;

impl MessageEncoder {
    /// Encode a PONG frame. (Empty payload - length of 1 for the type byte)
    pub fn encode_pong() -> Vec<u8> {
        let mut buf = Vec::with_capacity(5);
        buf.write_u32::<BigEndian>(1).unwrap();
        buf.push(0x02);
        buf
    }

    /// Encode an HTTP response. Returns one frame if body < 64KB,
    /// Multiple frames (START + CHUNKs + END) if body >= 64KB.
    pub fn encode_response(response: &HttpResponseMessage) -> Vec<Vec<u8>> {
        if response.body.len() < CHUNK_THRESHOLD {
            vec![Self::encode_single_response(response)]
        } else {
            Self::encode_chunked_response(response)
        }
    }

    // private helpers functions

    fn encode_single_response(response: &HttpResponseMessage) -> Vec<u8> {
        let mut payload = Vec::new();

        // Request ID — 32 ASCII hex bytes
        payload.extend_from_slice(response.request_id.as_bytes());
        // Status code
        payload
            .write_u16::<BigEndian>(response.status_code)
            .unwrap();
        // Headers
        Self::write_headers(&mut payload, &response.headers);
        // Body
        payload
            .write_u32::<BigEndian>(response.body.len() as u32)
            .unwrap();
        payload.extend_from_slice(&response.body);

        Self::frame(0x11, payload)
    }

    fn encode_chunked_response(response: &HttpResponseMessage) -> Vec<Vec<u8>> {
        let mut frames = Vec::new();
        let id = response.request_id.as_bytes();

        // START — metadata only, no body
        let mut start_payload = Vec::new();
        start_payload.extend_from_slice(id);
        start_payload
            .write_u16::<BigEndian>(response.status_code)
            .unwrap();
        Self::write_headers(&mut start_payload, &response.headers);
        frames.push(Self::frame(0x15, start_payload));

        // CHUNKs
        for chunk in response.body.chunks(CHUNK_SIZE) {
            let mut chunk_payload = Vec::new();
            chunk_payload.extend_from_slice(id);
            chunk_payload
                .write_u32::<BigEndian>(chunk.len() as u32)
                .unwrap();
            chunk_payload.extend_from_slice(chunk);
            frames.push(Self::frame(0x16, chunk_payload));
        }

        // END
        let mut end_payload = Vec::new();
        end_payload.extend_from_slice(id);
        frames.push(Self::frame(0x17, end_payload));

        frames
    }

    /// Write headers as: [count: u16] then for each: [name_len: u16][name][value_len: u16][value]
    fn write_headers(buf: &mut Vec<u8>, headers: &[(String, String)]) {
        buf.write_u16::<BigEndian>(headers.len() as u16).unwrap();
        for (name, value) in headers {
            buf.write_u16::<BigEndian>(name.len() as u16).unwrap();
            buf.extend_from_slice(name.as_bytes());
            buf.write_u16::<BigEndian>(value.len() as u16).unwrap();
            buf.extend_from_slice(value.as_bytes());
        }
    }

    /// Wrap a payload with the standard frame header: [length: u32][type: u8][payload]
    fn frame(type_byte: u8, payload: Vec<u8>) -> Vec<u8> {
        let mut message = Vec::with_capacity(5 + payload.len());
        message
            .write_u32::<BigEndian>(1 + payload.len() as u32)
            .unwrap();
        message.push(type_byte);
        message.extend_from_slice(&payload);
        message
    }
}
