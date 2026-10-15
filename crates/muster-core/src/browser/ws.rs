use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

#[derive(Debug, thiserror::Error)]
pub enum WsError {
    #[error("lỗi vào/ra: {0}")]
    Io(#[from] std::io::Error),

    #[error("bắt tay WebSocket thất bại: {0}")]
    Handshake(String),

    #[error("khung WebSocket không hợp lệ: {0}")]
    Frame(String),

    #[error("kết nối đã bị đóng")]
    Closed,

    #[error("không sinh được số ngẫu nhiên")]
    Random,

    #[error("payload không phải UTF-8 hợp lệ")]
    Utf8,

    #[error("địa chỉ DevTools không hợp lệ: {0}")]
    Address(String),

    #[error("không kết nối được tới cổng DevTools {address} trong {seconds} giây")]
    ConnectTimeout { address: String, seconds: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    Continuation,
    Text,
    Binary,
    Close,
    Ping,
    Pong,
}

impl OpCode {
    #[must_use]
    pub fn to_byte(self) -> u8 {
        match self {
            Self::Continuation => 0x0,
            Self::Text => 0x1,
            Self::Binary => 0x2,
            Self::Close => 0x8,
            Self::Ping => 0x9,
            Self::Pong => 0xA,
        }
    }

    #[must_use]
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x0 => Some(Self::Continuation),
            0x1 => Some(Self::Text),
            0x2 => Some(Self::Binary),
            0x8 => Some(Self::Close),
            0x9 => Some(Self::Ping),
            0xA => Some(Self::Pong),
            _ => None,
        }
    }
}

const MAX_PAYLOAD: usize = 64 * 1024 * 1024;

#[must_use]
pub fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);

    for chunk in bytes.chunks(3) {
        let first = u32::from(chunk[0]);
        let second = chunk.get(1).copied().map_or(0, u32::from);
        let third = chunk.get(2).copied().map_or(0, u32::from);
        let triple = (first << 16) | (second << 8) | third;

        out.push(ALPHABET[(triple >> 18) as usize & 0x3F] as char);
        out.push(ALPHABET[(triple >> 12) as usize & 0x3F] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(triple >> 6) as usize & 0x3F] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[triple as usize & 0x3F] as char
        } else {
            '='
        });
    }

    out
}

#[must_use]
pub fn handshake_request(host: &str, port: u16, path: &str, key: &str) -> String {
    format!(
        "GET {path} HTTP/1.1\r\n\
         Host: {host}:{port}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {key}\r\n\
         Sec-WebSocket-Version: 13\r\n\
         \r\n"
    )
}

#[must_use]
pub fn encode_frame(opcode: OpCode, payload: &[u8], mask: [u8; 4]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 14);

    out.push(0x80 | opcode.to_byte());

    let len = payload.len();
    if len < 126 {
        out.push(0x80 | len as u8);
    } else if len <= u16::MAX as usize {
        out.push(0x80 | 126);
        out.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        out.push(0x80 | 127);
        out.extend_from_slice(&(len as u64).to_be_bytes());
    }

    out.extend_from_slice(&mask);
    out.extend(
        payload
            .iter()
            .enumerate()
            .map(|(index, byte)| byte ^ mask[index % 4]),
    );

    out
}

#[must_use]
pub fn decode_frame(bytes: &[u8]) -> Option<(OpCode, Vec<u8>, usize)> {
    let header = bytes.first().copied()?;
    let second = bytes.get(1).copied()?;

    let opcode = OpCode::from_byte(header & 0x0F)?;
    let masked = second & 0x80 != 0;
    let short_len = (second & 0x7F) as usize;

    let mut offset = 2;
    let len = match short_len {
        126 => {
            let raw = bytes.get(offset..offset + 2)?;
            offset += 2;
            usize::from(u16::from_be_bytes([raw[0], raw[1]]))
        }
        127 => {
            let raw = bytes.get(offset..offset + 8)?;
            offset += 8;
            let value = u64::from_be_bytes([
                raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
            ]);
            usize::try_from(value).ok()?
        }
        other => other,
    };

    if len > MAX_PAYLOAD {
        return None;
    }

    let mask = if masked {
        let raw = bytes.get(offset..offset + 4)?;
        offset += 4;
        Some([raw[0], raw[1], raw[2], raw[3]])
    } else {
        None
    };

    let payload = bytes.get(offset..offset + len)?.to_vec();
    let payload = match mask {
        Some(mask) => payload
            .iter()
            .enumerate()
            .map(|(index, byte)| byte ^ mask[index % 4])
            .collect(),
        None => payload,
    };

    Some((opcode, payload, offset + len))
}

pub const CONNECT_ATTEMPT_TIMEOUT: Duration = Duration::from_millis(1500);
pub const CONNECT_RETRY_STEP: Duration = Duration::from_millis(150);

pub fn connect_with_retry(address: SocketAddr, timeout: Duration) -> Result<TcpStream, WsError> {
    let deadline = Instant::now() + timeout;

    loop {
        if let Ok(stream) = TcpStream::connect_timeout(&address, CONNECT_ATTEMPT_TIMEOUT) {
            return Ok(stream);
        }

        if Instant::now() >= deadline {
            return Err(WsError::ConnectTimeout {
                address: address.to_string(),
                seconds: timeout.as_secs(),
            });
        }

        std::thread::sleep(CONNECT_RETRY_STEP);
    }
}

pub struct WebSocket {
    stream: TcpStream,
    pending: Vec<u8>,
}

impl WebSocket {
    pub fn connect(host: &str, port: u16, path: &str, timeout: Duration) -> Result<Self, WsError> {
        let address: SocketAddr = format!("{host}:{port}")
            .parse()
            .map_err(|_| WsError::Address(format!("{host}:{port}")))?;

        let stream = connect_with_retry(address, timeout)?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        stream.set_nodelay(true)?;

        let mut key_bytes = [0u8; 16];
        getrandom::getrandom(&mut key_bytes).map_err(|_| WsError::Random)?;
        let key = base64_encode(&key_bytes);

        let mut stream = stream;
        stream.write_all(handshake_request(host, port, path, &key).as_bytes())?;
        stream.flush()?;

        let (response, pending) = read_until_headers_end(&mut stream)?;
        let status_line = response.lines().next().unwrap_or_default();
        if !status_line.contains(" 101 ") {
            return Err(WsError::Handshake(format!(
                "server trả về: {}",
                status_line.trim()
            )));
        }

        Ok(Self { stream, pending })
    }

    pub fn send_text(&mut self, text: &str) -> Result<(), WsError> {
        self.send_frame(OpCode::Text, text.as_bytes())
    }

    pub fn send_frame(&mut self, opcode: OpCode, payload: &[u8]) -> Result<(), WsError> {
        let mut mask = [0u8; 4];
        getrandom::getrandom(&mut mask).map_err(|_| WsError::Random)?;
        self.stream
            .write_all(&encode_frame(opcode, payload, mask))?;
        self.stream.flush()?;
        Ok(())
    }

    pub fn set_read_timeout(&self, timeout: Duration) -> Result<(), WsError> {
        self.stream.set_read_timeout(Some(timeout))?;
        Ok(())
    }

    pub fn recv_text(&mut self) -> Result<String, WsError> {
        let mut chunk = [0u8; 8192];

        loop {
            if let Some((opcode, payload, consumed)) = decode_frame(&self.pending) {
                self.pending.drain(..consumed);

                match opcode {
                    OpCode::Text => {
                        return String::from_utf8(payload).map_err(|_| WsError::Utf8);
                    }
                    OpCode::Close => return Err(WsError::Closed),
                    OpCode::Ping => self.send_frame(OpCode::Pong, &payload)?,
                    _ => {}
                }
                continue;
            }

            let read = self.stream.read(&mut chunk)?;
            if read == 0 {
                return Err(WsError::Closed);
            }
            self.pending.extend_from_slice(&chunk[..read]);
            if self.pending.len() > MAX_PAYLOAD + 14 {
                return Err(WsError::Frame("payload quá lớn".to_owned()));
            }
        }
    }

    pub fn close(&mut self) {
        let _ = self.send_frame(OpCode::Close, &[0x03, 0xE8]);
    }
}

fn read_until_headers_end(stream: &mut TcpStream) -> Result<(String, Vec<u8>), WsError> {
    let mut buffer: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 1024];

    loop {
        if let Some(position) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            let pending = buffer.split_off(position + 4);
            buffer.truncate(position);
            return Ok((String::from_utf8_lossy(&buffer).into_owned(), pending));
        }
        if buffer.len() > 64 * 1024 {
            return Err(WsError::Handshake("header quá lớn".to_owned()));
        }

        let read = stream.read(&mut chunk)?;
        if read == 0 {
            return Err(WsError::Handshake("server đóng kết nối".to_owned()));
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64_encode(&[0x00, 0x01, 0x02, 0x03]), "AAECAw==");
    }

    #[test]
    fn the_handshake_request_carries_every_header_chrome_expects() {
        let request = handshake_request("127.0.0.1", 9222, "/devtools/page/ABC", "KEY");

        assert!(request.starts_with("GET /devtools/page/ABC HTTP/1.1\r\n"));
        assert!(request.contains("Host: 127.0.0.1:9222\r\n"));
        assert!(request.contains("Upgrade: websocket\r\n"));
        assert!(request.contains("Connection: Upgrade\r\n"));
        assert!(request.contains("Sec-WebSocket-Key: KEY\r\n"));
        assert!(request.contains("Sec-WebSocket-Version: 13\r\n"));
        assert!(request.ends_with("\r\n\r\n"));
    }

    #[test]
    fn a_masked_text_frame_round_trips() {
        let mask = [0xAA, 0xBB, 0xCC, 0xDD];
        let frame = encode_frame(OpCode::Text, b"hello", mask);

        assert_eq!(frame[0], 0x81, "FIN + opcode Text");
        assert_eq!(frame[1], 0x80 | 5, "MASK + do dai 5");

        let (opcode, payload, consumed) = decode_frame(&frame).expect("giai ma");
        assert_eq!(opcode, OpCode::Text);
        assert_eq!(payload, b"hello");
        assert_eq!(consumed, frame.len());
    }

    #[test]
    fn an_unmasked_server_frame_is_decoded() {
        let frame = vec![0x81, 0x03, b'a', b'b', b'c'];
        let (opcode, payload, consumed) = decode_frame(&frame).expect("giai ma");

        assert_eq!(opcode, OpCode::Text);
        assert_eq!(payload, b"abc");
        assert_eq!(consumed, 5);
    }

    #[test]
    fn the_extended_length_encodings_are_handled() {
        let mask = [1, 2, 3, 4];

        let medium = vec![0x41u8; 200];
        let frame = encode_frame(OpCode::Binary, &medium, mask);
        assert_eq!(frame[1], 0x80 | 126);
        let (opcode, payload, _) = decode_frame(&frame).expect("giai ma");
        assert_eq!(opcode, OpCode::Binary);
        assert_eq!(payload.len(), 200);

        let large = vec![0x42u8; 70_000];
        let frame = encode_frame(OpCode::Binary, &large, mask);
        assert_eq!(frame[1], 0x80 | 127);
        let (_, payload, _) = decode_frame(&frame).expect("giai ma");
        assert_eq!(payload.len(), 70_000);
    }

    #[test]
    fn an_incomplete_frame_reads_as_none_so_the_caller_can_read_more() {
        let mask = [9, 9, 9, 9];
        let frame = encode_frame(OpCode::Text, b"payload", mask);

        for cut in 0..frame.len() {
            assert_eq!(decode_frame(&frame[..cut]), None, "cat tai {cut}");
        }
        assert!(decode_frame(&frame).is_some());
    }

    #[test]
    fn the_opcode_table_covers_the_frames_a_devtools_server_sends() {
        assert_eq!(OpCode::from_byte(0x1), Some(OpCode::Text));
        assert_eq!(OpCode::from_byte(0x8), Some(OpCode::Close));
        assert_eq!(OpCode::from_byte(0x9), Some(OpCode::Ping));
        assert_eq!(OpCode::from_byte(0xA), Some(OpCode::Pong));
        assert_eq!(OpCode::from_byte(0x2), Some(OpCode::Binary));
        assert_eq!(OpCode::from_byte(0x3), None);
    }

    #[test]
    fn handshake_keeps_a_frame_arriving_with_the_http_headers() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let server = std::thread::spawn(move || {
            let (mut sender, _) = listener.accept().expect("accept");
            let mut request = [0u8; 1024];
            let _ = sender.read(&mut request).expect("request");
            sender
                .write_all(
                    b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n\r\n\x81\x02ok",
                )
                .expect("handshake and frame");
        });
        let mut socket = WebSocket::connect(
            "127.0.0.1",
            port,
            "/devtools/page/x",
            Duration::from_secs(2),
        )
        .expect("handshake");
        server.join().expect("server finished");
        assert_eq!(socket.recv_text().expect("frame"), "ok");
    }

    #[test]
    fn two_frames_in_one_read_are_not_lost_between_calls() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let stream = TcpStream::connect(listener.local_addr().expect("addr")).expect("connect");
        let (mut sender, _) = listener.accept().expect("accept");
        sender
            .write_all(&[0x81, 0x03, b'o', b'n', b'e', 0x81, 0x03, b't', b'w', b'o'])
            .expect("send frames");
        let mut socket = WebSocket {
            stream,
            pending: Vec::new(),
        };
        assert_eq!(socket.recv_text().expect("first"), "one");
        assert_eq!(socket.recv_text().expect("second"), "two");
    }

    #[test]
    fn a_dead_devtools_port_times_out_instead_of_hanging() {
        let address: SocketAddr = "127.0.0.1:1".parse().expect("dia chi hop le");
        let start = Instant::now();

        let error = connect_with_retry(address, Duration::from_millis(300)).expect_err("phai loi");

        assert!(
            matches!(error, WsError::ConnectTimeout { .. }),
            "gap: {error:?}"
        );
        assert!(start.elapsed() >= Duration::from_millis(250));
        assert!(start.elapsed() < Duration::from_secs(5), "khong duoc treo");
    }

    #[test]
    fn an_invalid_devtools_address_is_refused_before_connecting() {
        let result = WebSocket::connect(
            "khong-phai-host",
            9222,
            "/devtools/page/x",
            Duration::from_millis(50),
        )
        .map(|_| ());

        assert!(
            matches!(result, Err(WsError::Address(_))),
            "gap: {result:?}"
        );
    }
}
