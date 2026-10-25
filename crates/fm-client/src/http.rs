//! The HTTP/1.1 the client needs, on `std::net`: one request per
//! connection (`Connection: close`), an answer read by `Content-Length`, by
//! chunks or to the end, and the chunked body of the event stream.
//!
//! Every failure is an [`ApiError`]: the app not answering
//! ([`not_reachable`]), the connection lost mid-call ([`lost`]), or an
//! answer this client cannot read ([`unreadable`]).

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};

use fm_api::{API_VERSION, ApiError, ApiResult, ApiVersion, ErrorCode, VERSION_HEADER};
use serde::de::DeserializeOwned;

use crate::CONNECT_TIMEOUT;

/// The path prefix of every route.
pub(crate) const ROUTE_PREFIX: &str = "/v1/";

/// The longest status line plus headers this client reads.
const MAX_HEAD_BYTES: u64 = 64 * 1024;
/// The largest answer body this client reads (a big specimen or preview
/// fits many times over).
const MAX_BODY_BYTES: u64 = 512 * 1024 * 1024;

/// The status line and headers of an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Head {
    pub status: u16,
    headers: Vec<(String, String)>,
}

impl Head {
    /// The first value of header `name` (any case).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// Refuses an answer from an app of another major, when it names its
    /// version (an app from before E25.3 does not).
    pub fn check_version(&self) -> ApiResult<()> {
        match self.header(VERSION_HEADER).map(str::parse::<ApiVersion>) {
            Some(Ok(served)) => fm_api::check_server_version(served),
            _ => Ok(()),
        }
    }
}

/// A whole answer.
#[derive(Debug)]
pub(crate) struct Answer {
    pub head: Head,
    pub body: Vec<u8>,
}

impl Answer {
    /// The value of a 2xx answer, or the error it stands for: the app's own
    /// [`ApiError`] (its JSON shape survives every status, ADR 0015), a major
    /// mismatch, or an answer that is not what the client expects.
    pub fn decode<T: DeserializeOwned>(self) -> ApiResult<T> {
        let status = self.head.status;
        if !(200..300).contains(&status) {
            if let Ok(e) = serde_json::from_slice::<ApiError>(&self.body) {
                return Err(if status == 401 { wrong_token(e) } else { e });
            }
            self.head.check_version()?;
            return Err(unreadable(format!("HTTP {status} without an error")));
        }
        self.head.check_version()?;
        serde_json::from_slice(&self.body).map_err(|e| unreadable(format!("answer: {e}")))
    }
}

/// The message for the app's 401: the token in `endpoint.json` is not the
/// one the app holds now. It names where the user sees the current token;
/// the app's own message stays in the detail.
pub(crate) const WRONG_TOKEN: &str = "Font Manager did not accept the token of its local API. Check the token in Font Manager under Settings > Local API, or restart Font Manager.";

fn wrong_token(e: ApiError) -> ApiError {
    let detail = match e.detail {
        Some(detail) => format!("{}; {detail}", e.message),
        None => e.message,
    };
    ApiError::new(ErrorCode::PermissionDenied, WRONG_TOKEN).with_detail(detail)
}

/// Sends `POST <path>` with `body` and reads the whole answer.
pub(crate) fn post(addr: SocketAddr, token: &str, path: &str, body: &[u8]) -> ApiResult<Answer> {
    let mut stream = open(addr)?;
    let mut req = request_head("POST", addr, token, path);
    req.push_str("Content-Type: application/json\r\n");
    req.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
    let mut bytes = req.into_bytes();
    bytes.extend_from_slice(body);
    stream.write_all(&bytes).map_err(lost)?;
    let mut reader = BufReader::new(stream);
    let head = read_head(&mut reader)?;
    let body = read_body(&head, reader)?;
    Ok(Answer { head, body })
}

/// A connection to the endpoint, within [`CONNECT_TIMEOUT`].
pub(crate) fn open(addr: SocketAddr) -> ApiResult<TcpStream> {
    TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT).map_err(not_reachable)
}

/// The request line and the headers every request carries, up to the
/// method's own headers.
pub(crate) fn request_head(method: &str, addr: SocketAddr, token: &str, path: &str) -> String {
    // The endpoint takes `127.0.0.1:<port>` or `localhost:<port>` only.
    let host = if addr.is_ipv4() {
        format!("127.0.0.1:{}", addr.port())
    } else {
        format!("localhost:{}", addr.port())
    };
    format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nAuthorization: Bearer {token}\r\n\
         {VERSION_HEADER}: {API_VERSION}\r\nConnection: close\r\n"
    )
}

/// The status line and headers, up to the empty line.
pub(crate) fn read_head(reader: &mut impl BufRead) -> ApiResult<Head> {
    let mut limited = reader.take(MAX_HEAD_BYTES);
    let mut status_line = String::new();
    read_line(&mut limited, &mut status_line)?;
    let status = status_line
        .strip_prefix("HTTP/1.")
        .and_then(|rest| rest.split(' ').nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| unreadable(format!("status line {:?}", status_line.trim_end())))?;
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        read_line(&mut limited, &mut line)?;
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| unreadable(format!("header line {line:?}")))?;
        headers.push((name.trim().to_owned(), value.trim().to_owned()));
    }
    Ok(Head { status, headers })
}

/// One line with its `\n`; the connection ending first is a lost call,
/// headers over [`MAX_HEAD_BYTES`] are not the endpoint's.
fn read_line<R: BufRead>(reader: &mut io::Take<R>, line: &mut String) -> ApiResult<()> {
    match reader.read_line(line) {
        Ok(_) if line.ends_with('\n') => Ok(()),
        Ok(_) if reader.limit() == 0 => Err(unreadable("headers too long")),
        Ok(_) => Err(lost(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "the connection closed before the answer's headers ended",
        ))),
        Err(e) if e.kind() == io::ErrorKind::InvalidData => Err(unreadable("headers not UTF-8")),
        Err(e) => Err(lost(e)),
    }
}

/// The whole body, by the head's framing; a body cut short is a lost call.
fn read_body<R: BufRead>(head: &Head, reader: R) -> ApiResult<Vec<u8>> {
    let mut body = Vec::new();
    let mut limited = Body::new(head, reader)?.take(MAX_BODY_BYTES + 1);
    limited.read_to_end(&mut body).map_err(lost)?;
    if body.len() as u64 > MAX_BODY_BYTES {
        return Err(unreadable("the answer is too large"));
    }
    if let Some(expected) = content_length(head)?
        && body.len() as u64 != expected
    {
        return Err(lost(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!("the answer ended after {} of {expected} bytes", body.len()),
        )));
    }
    Ok(body)
}

fn content_length(head: &Head) -> ApiResult<Option<u64>> {
    head.header("content-length")
        .map(|v| {
            v.parse::<u64>()
                .map_err(|_| unreadable(format!("Content-Length {v:?}")))
        })
        .transpose()
}

/// A body read by its framing: chunked, a known length, or to the end.
#[derive(Debug)]
pub(crate) enum Body<R> {
    Chunked(Chunked<R>),
    Length(io::Take<R>),
    ToEnd(R),
}

impl<R: BufRead> Body<R> {
    pub fn new(head: &Head, reader: R) -> ApiResult<Self> {
        let chunked = head
            .header("transfer-encoding")
            .is_some_and(|v| v.to_ascii_lowercase().contains("chunked"));
        Ok(if chunked {
            Self::Chunked(Chunked::new(reader))
        } else if let Some(len) = content_length(head)? {
            Self::Length(reader.take(len))
        } else {
            Self::ToEnd(reader)
        })
    }
}

impl<R: BufRead> Read for Body<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Chunked(r) => r.read(buf),
            Self::Length(r) => r.read(buf),
            Self::ToEnd(r) => r.read(buf),
        }
    }
}

/// A `Transfer-Encoding: chunked` body, decoded. A stream that ends inside
/// a chunk, or before the last (empty) chunk, is `UnexpectedEof`.
#[derive(Debug)]
pub(crate) struct Chunked<R> {
    inner: R,
    /// Bytes left in the current chunk.
    left: u64,
    done: bool,
}

impl<R: BufRead> Chunked<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            left: 0,
            done: false,
        }
    }

    /// A line of at most 4 KiB (a chunk size, its CRLF or a trailer).
    fn line(&mut self) -> io::Result<String> {
        let mut bytes = Vec::new();
        (&mut self.inner).take(4096).read_until(b'\n', &mut bytes)?;
        if !bytes.ends_with(b"\n") {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "the stream ended inside a chunk header",
            ));
        }
        String::from_utf8(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

impl<R: BufRead> Read for Chunked<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.done || buf.is_empty() {
            return Ok(0);
        }
        if self.left == 0 {
            let line = self.line()?;
            let size = line.trim().split(';').next().unwrap_or("").trim();
            let size = u64::from_str_radix(size, 16).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, format!("chunk size {size:?}"))
            })?;
            if size == 0 {
                // Trailers, up to the empty line.
                while !self.line()?.trim().is_empty() {}
                self.done = true;
                return Ok(0);
            }
            self.left = size;
        }
        let want = buf
            .len()
            .min(usize::try_from(self.left).unwrap_or(usize::MAX));
        let n = self.inner.read(&mut buf[..want])?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "the stream ended inside a chunk",
            ));
        }
        self.left -= n as u64;
        if self.left == 0 && !self.line()?.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "a chunk did not end with CRLF",
            ));
        }
        Ok(n)
    }
}

/// The app is not there: nothing listens, or it did not accept in time.
pub(crate) fn not_reachable(e: io::Error) -> ApiError {
    ApiError::new(
        ErrorCode::Io,
        "Font Manager is not running, or its local API does not answer.",
    )
    .with_detail(e.to_string())
}

/// The connection broke after the request started.
pub(crate) fn lost(e: io::Error) -> ApiError {
    ApiError::new(ErrorCode::Io, "The connection to Font Manager was lost.")
        .with_detail(e.to_string())
}

/// The other side answered something that is not the local API.
pub(crate) fn unreadable(detail: impl Into<String>) -> ApiError {
    ApiError::new(
        ErrorCode::Internal,
        "Font Manager sent an answer this program cannot read.",
    )
    .with_detail(detail)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn head(raw: &str) -> ApiResult<Head> {
        read_head(&mut raw.as_bytes())
    }

    #[test]
    fn a_head_gives_its_status_and_headers_in_any_case() {
        let h = head("HTTP/1.1 422 Unprocessable Entity\r\nFm-Api-Version: 0.22.0\r\ncontent-length: 2\r\n\r\n{}")
            .unwrap();
        assert_eq!(h.status, 422);
        assert_eq!(h.header("fm-api-version"), Some("0.22.0"));
        assert_eq!(h.header("Content-Length"), Some("2"));
        assert_eq!(h.header("Allow"), None);
    }

    #[test]
    fn a_head_cut_off_is_a_lost_connection_and_garbage_is_unreadable() {
        let cut = head("HTTP/1.1 200 OK\r\nContent-Le").unwrap_err();
        assert_eq!(cut.code, ErrorCode::Io);
        assert_eq!(cut.message, "The connection to Font Manager was lost.");
        assert_eq!(head("").unwrap_err().code, ErrorCode::Io);
        let garbage = head("SSH-2.0-OpenSSH_9.6\r\n\r\n").unwrap_err();
        assert_eq!(garbage.code, ErrorCode::Internal);
    }

    #[test]
    fn a_chunked_body_is_decoded_with_extensions_and_trailers() {
        let raw = "5;x=1\r\nhello\r\n1\r\n \r\n5\r\nworld\r\n0\r\nTrailer: t\r\n\r\n";
        let mut out = String::new();
        Chunked::new(raw.as_bytes())
            .read_to_string(&mut out)
            .unwrap();
        assert_eq!(out, "hello world");
    }

    #[test]
    fn a_chunked_body_cut_short_fails() {
        for raw in ["5\r\nhel", "5\r\nhello\r\n", "5\r\nhello", "zz\r\n"] {
            let mut out = Vec::new();
            assert!(
                Chunked::new(raw.as_bytes()).read_to_end(&mut out).is_err(),
                "{raw:?}"
            );
        }
    }

    #[test]
    fn a_body_shorter_than_its_length_is_a_lost_connection() {
        let h = head("HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n").unwrap();
        let e = read_body(&h, &b"null"[..]).unwrap_err();
        assert_eq!(e.code, ErrorCode::Io);
        let h = head("HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\n").unwrap();
        assert_eq!(read_body(&h, &b"null"[..]).unwrap(), b"null");
    }

    #[test]
    fn an_error_answer_keeps_the_apps_error() {
        let answer = Answer {
            head: head("HTTP/1.1 404 Not Found\r\n\r\n").unwrap(),
            body: br#"{"code":"notFound","message":"face 9 was not found."}"#.to_vec(),
        };
        let e = answer.decode::<()>().unwrap_err();
        assert_eq!(e, ApiError::not_found("face 9"));
    }

    #[test]
    fn an_answer_from_another_major_is_unsupported() {
        let other = format!("{}.0.0", API_VERSION.major + 1);
        let answer = Answer {
            head: head(&format!(
                "HTTP/1.1 200 OK\r\nFm-Api-Version: {other}\r\n\r\n"
            ))
            .unwrap(),
            body: b"null".to_vec(),
        };
        let e = answer.decode::<()>().unwrap_err();
        assert_eq!(e.code, ErrorCode::Unsupported);
        assert!(e.message.contains(&other), "{}", e.message);
        // The same major at another minor is served.
        let same = format!("{}.{}.9", API_VERSION.major, API_VERSION.minor + 1);
        let answer = Answer {
            head: head(&format!(
                "HTTP/1.1 200 OK\r\nFm-Api-Version: {same}\r\n\r\n"
            ))
            .unwrap(),
            body: b"null".to_vec(),
        };
        assert!(answer.decode::<()>().is_ok());
    }

    #[test]
    fn the_request_names_host_token_and_version() {
        let addr: SocketAddr = "127.0.0.1:4100".parse().unwrap();
        let head = request_head("POST", addr, "abc", "/v1/app_info");
        assert!(head.starts_with("POST /v1/app_info HTTP/1.1\r\n"));
        assert!(head.contains("Host: 127.0.0.1:4100\r\n"));
        assert!(head.contains("Authorization: Bearer abc\r\n"));
        assert!(head.contains(&format!("Fm-Api-Version: {API_VERSION}\r\n")));
        assert!(!head.contains("Origin"), "the endpoint refuses any Origin");
    }
}
