//! The relay's side of HTTP/1.1: reading one request from a browser and writing one response.
//!
//! Deliberately small: one request per connection (`Connection: close`), request bodies only
//! with `Content-Length` (browsers send one for the bodies the page gives them), heads and
//! bodies read with hard limits, responses streamed with chunked transfer coding.

use std::io::{self, BufRead, Read, Write};

/// Why a request could not be read.
#[derive(Debug)]
pub enum ReadError {
    /// The client closed the connection before sending a request.
    Closed,
    /// Not a request the relay understands (400).
    Bad(String),
    /// The head or the body is over its limit (413).
    TooLarge(&'static str),
    /// A transfer coding the relay does not take (501).
    Unsupported(String),
    /// Reading failed (a timeout, a reset).
    Io(io::Error),
}

impl From<io::Error> for ReadError {
    fn from(error: io::Error) -> Self {
        ReadError::Io(error)
    }
}

/// One request as the browser sent it.
#[derive(Debug, Clone, Default)]
pub struct Request {
    pub method: String,
    /// The target's path, without the query.
    pub path: String,
    /// In the order received, names in lowercase.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    /// The first header called `name` (lowercase).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// How many headers are called `name` (lowercase).
    pub fn count(&self, name: &str) -> usize {
        self.headers.iter().filter(|(n, _)| n == name).count()
    }
}

/// Whether `name` is an HTTP token (RFC 9110).
fn token(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}

/// Reads one line of the head, ending in CRLF or LF, counting it against `left`.
fn line<R: BufRead>(reader: &mut R, left: &mut usize) -> Result<Option<String>, ReadError> {
    let mut bytes = Vec::new();
    let read = reader
        .by_ref()
        .take(*left as u64 + 1)
        .read_until(b'\n', &mut bytes)?;
    if read == 0 {
        return Ok(None);
    }
    if read > *left || !bytes.ends_with(b"\n") {
        return Err(if read > *left {
            ReadError::TooLarge("the request head")
        } else {
            ReadError::Bad("the request head ended early".into())
        });
    }
    *left -= read;
    bytes.pop();
    if bytes.ends_with(b"\r") {
        bytes.pop();
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| ReadError::Bad("the request head is not text".into()))
}

/// Reads one request: its head (at most `max_head` bytes) and its body (at most `max_body`).
pub fn read_request<R: BufRead>(
    reader: &mut R,
    max_head: usize,
    max_body: usize,
) -> Result<Request, ReadError> {
    let mut left = max_head;
    let first = match line(reader, &mut left)? {
        Some(first) => first,
        None => return Err(ReadError::Closed),
    };
    let mut parts = first.split(' ');
    let (Some(method), Some(target), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(ReadError::Bad("a malformed request line".into()));
    };
    if !token(method) || !matches!(version, "HTTP/1.1" | "HTTP/1.0") || !target.starts_with('/') {
        return Err(ReadError::Bad("a malformed request line".into()));
    }
    let path = target.split(['?', '#']).next().unwrap_or("/").to_owned();
    let mut headers = Vec::new();
    loop {
        let Some(text) = line(reader, &mut left)? else {
            return Err(ReadError::Bad("the request head ended early".into()));
        };
        if text.is_empty() {
            break;
        }
        if text.starts_with([' ', '\t']) {
            return Err(ReadError::Bad("folded header lines are not allowed".into()));
        }
        let Some((name, value)) = text.split_once(':') else {
            return Err(ReadError::Bad("a header line without a colon".into()));
        };
        if !token(name) {
            return Err(ReadError::Bad("a bad header name".into()));
        }
        headers.push((
            name.to_ascii_lowercase(),
            value.trim_matches([' ', '\t']).to_owned(),
        ));
        if headers.len() > 256 {
            return Err(ReadError::TooLarge("the request head"));
        }
    }
    let mut request = Request {
        method: method.to_owned(),
        path,
        headers,
        body: Vec::new(),
    };
    if request.header("transfer-encoding").is_some() {
        return Err(ReadError::Unsupported(
            "request bodies need a Content-Length, not a transfer coding".into(),
        ));
    }
    let lengths: Vec<&str> = request
        .headers
        .iter()
        .filter(|(n, _)| n == "content-length")
        .map(|(_, v)| v.as_str())
        .collect();
    let length = match lengths.as_slice() {
        [] => 0,
        [first, rest @ ..] if rest.iter().all(|v| v == first) => first
            .parse::<usize>()
            .ok()
            .filter(|_| first.bytes().all(|b| b.is_ascii_digit()))
            .ok_or_else(|| ReadError::Bad("a bad Content-Length".into()))?,
        _ => return Err(ReadError::Bad("conflicting Content-Length headers".into())),
    };
    if length > max_body {
        return Err(ReadError::TooLarge("the request body"));
    }
    request.body = vec![0; length];
    reader.read_exact(&mut request.body)?;
    Ok(request)
}

/// A response body.
pub enum Body {
    Empty,
    Bytes(Vec<u8>),
    /// Streamed with chunked transfer coding.
    Stream(Box<dyn Read + Send>),
}

impl std::fmt::Debug for Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Body::Empty => f.write_str("Empty"),
            Body::Bytes(bytes) => write!(f, "Bytes({})", bytes.len()),
            Body::Stream(_) => f.write_str("Stream"),
        }
    }
}

/// One response.
#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Body,
}

impl Response {
    pub fn new(status: u16) -> Response {
        Response {
            status,
            headers: Vec::new(),
            body: Body::Empty,
        }
    }

    pub fn header(mut self, name: &str, value: impl Into<String>) -> Response {
        self.headers.push((name.to_owned(), value.into()));
        self
    }

    pub fn body(mut self, body: Body) -> Response {
        self.body = body;
        self
    }

    /// The first header called `name`, in any case.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Content Too Large",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "",
    }
}

/// Writes `response` and closes the exchange (`Connection: close`). Returns the body bytes
/// written.
pub fn write_response<W: Write>(out: &mut W, response: Response) -> io::Result<u64> {
    let mut head = format!("HTTP/1.1 {} {}\r\n", response.status, reason(response.status));
    for (name, value) in &response.headers {
        // Values come from the relay or from JSON it wrote; never pass a line break on.
        let value: String = value.chars().filter(|c| *c != '\r' && *c != '\n').collect();
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("Connection: close\r\n");
    let mut written = 0;
    match response.body {
        Body::Empty => {
            head.push_str("Content-Length: 0\r\n\r\n");
            out.write_all(head.as_bytes())?;
        }
        Body::Bytes(bytes) => {
            head.push_str(&format!("Content-Length: {}\r\n\r\n", bytes.len()));
            out.write_all(head.as_bytes())?;
            out.write_all(&bytes)?;
            written = bytes.len() as u64;
        }
        Body::Stream(mut reader) => {
            head.push_str("Transfer-Encoding: chunked\r\n\r\n");
            out.write_all(head.as_bytes())?;
            let mut buffer = vec![0; 64 * 1024];
            loop {
                let n = match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    // The upstream answer broke off: end without the last chunk, so the browser
                    // sees an incomplete response rather than a short one.
                    Err(error) => return Err(error),
                };
                out.write_all(format!("{n:x}\r\n").as_bytes())?;
                out.write_all(&buffer[..n])?;
                out.write_all(b"\r\n")?;
                written += n as u64;
            }
            out.write_all(b"0\r\n\r\n")?;
        }
    }
    out.flush()?;
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn read(text: &str) -> Result<Request, ReadError> {
        read_request(&mut Cursor::new(text.as_bytes().to_vec()), 4096, 64)
    }

    #[test]
    fn reads_a_request_with_a_body() {
        let request = read(
            "POST /v1/forward?x=1 HTTP/1.1\r\nHost: 127.0.0.1\r\nJf-Relay-Url:  https://a.example/ \r\nContent-Length: 5\r\n\r\nhello",
        )
        .unwrap();
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/v1/forward");
        assert_eq!(request.header("jf-relay-url"), Some("https://a.example/"));
        assert_eq!(request.body, b"hello");
    }

    #[test]
    fn refuses_what_it_does_not_take() {
        assert!(matches!(read(""), Err(ReadError::Closed)));
        assert!(matches!(
            read("GET http://a.example/ HTTP/1.1\r\n\r\n"),
            Err(ReadError::Bad(_))
        ));
        assert!(matches!(read("GET / HTTP/2\r\n\r\n"), Err(ReadError::Bad(_))));
        assert!(matches!(
            read("GET / HTTP/1.1\r\nA: b\r\n folded\r\n\r\n"),
            Err(ReadError::Bad(_))
        ));
        assert!(matches!(
            read("GET / HTTP/1.1\r\nBad Name: b\r\n\r\n"),
            Err(ReadError::Bad(_))
        ));
        assert!(matches!(
            read("POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n"),
            Err(ReadError::Unsupported(_))
        ));
        assert!(matches!(
            read("POST / HTTP/1.1\r\nContent-Length: 5\r\nContent-Length: 6\r\n\r\nhello!"),
            Err(ReadError::Bad(_))
        ));
        assert!(matches!(
            read("POST / HTTP/1.1\r\nContent-Length: +5\r\n\r\nhello"),
            Err(ReadError::Bad(_))
        ));
        assert!(matches!(
            read("POST / HTTP/1.1\r\nContent-Length: 65\r\n\r\n"),
            Err(ReadError::TooLarge(_))
        ));
        let long = format!("GET / HTTP/1.1\r\nA: {}\r\n\r\n", "x".repeat(5000));
        assert!(matches!(read(&long), Err(ReadError::TooLarge(_))));
        assert!(matches!(
            read("GET / HTTP/1.1\r\nA: b\r\n"),
            Err(ReadError::Bad(_))
        ));
    }

    #[test]
    fn writes_streams_in_chunks() {
        let mut out = Vec::new();
        let response = Response::new(200)
            .header("A", "b\r\nInjected: yes")
            .body(Body::Stream(Box::new(Cursor::new(b"abc".to_vec()))));
        write_response(&mut out, response).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("HTTP/1.1 200 OK\r\nA: bInjected: yes\r\nConnection: close\r\n"));
        assert!(text.ends_with("Transfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n0\r\n\r\n"));
    }
}
