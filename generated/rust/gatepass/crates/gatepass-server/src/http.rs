// generated from gatepass v1
// model digest f2e0f8ff51c077fa1c713d8151544379bafac36a5a927e71c685042d53ab6e61
// contract digest e6e58e055d24f8f494dcff274f55e723d967f9d1f9aea16641bb8dacbb71171e
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize`

//! HTTP/1.1, as much of it as a synthesised surface needs and no more.
//!
//! Not a framework and not a deployment. One connection at a time, in accept order: read the
//! request line, read the headers, read exactly `Content-Length` bytes, answer, close. There is no
//! keep-alive, no pipelining, no compression, no TLS and no thread pool, and every one of those is
//! a decision a deployment gets to make rather than one a generator makes for it. What this file
//! *does* guarantee is the part the specification determines: the status codes and the bodies.
//!
//! Written here rather than taken from a crate for the reason the JSON reader beside it is: the
//! emitted tree builds with zero third-party crates, inside a gate that reaches no network.

use std::io::{BufRead, Read, Write};

/// The largest body this surface reads, in bytes.
///
/// A caller can claim any length, and a server that allocated whatever it was told to is a server
/// anyone can stop by saying a large number. A megabyte is far past any command input this model
/// can describe.
pub const MAX_BODY: usize = 1_048_576;

/// The media type every answer derived from the model carries.
pub const JSON: &str = "application/json";

/// The media type the prose answer carries.
///
/// The bytes served are the committed Markdown, unrendered: rendering it to HTML here would be a
/// second rendering of the documentation, and the two would differ the first time either moved.
pub const MARKDOWN: &str = "text/markdown; charset=utf-8";

/// One request, as much of it as this surface reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The method, verbatim.
    pub method: String,
    /// The target, with any query string removed.
    ///
    /// The model declares no parameter, so a query string names nothing on this surface. It is
    /// dropped rather than refused, because a caller that appends one has not made a different
    /// request.
    pub path: String,
    /// The body: exactly the `Content-Length` bytes the caller announced.
    pub body: Vec<u8>,
}

/// One answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// The status code.
    pub status: u16,
    /// The media type of the body.
    pub content_type: &'static str,
    /// The body.
    pub body: String,
}

impl Response {
    /// An answer carrying a body.
    pub fn new(status: u16, content_type: &'static str, body: impl Into<String>) -> Self {
        Self {
            status,
            content_type,
            body: body.into(),
        }
    }

    /// A refusal this surface makes rather than the specification.
    ///
    /// A malformed request, a path nothing declares, a method a path does not answer, an
    /// obligation nothing has satisfied. None of these is a declared outcome, and none is
    /// published in the contract, because each is a fact about a transport rather than about a
    /// command. The body is JSON with one member: a caller that has just failed to satisfy a
    /// contract should not have to parse a second format to read why.
    pub fn refusal(status: u16, detail: &str) -> Self {
        let mut body = String::from("{");
        crate::json::member(&mut body, "refused");
        crate::json::push_text(&mut body, detail);
        body.push('}');
        Self::new(status, JSON, body)
    }
}

/// The answer for a path this surface holds under a different method.
pub fn method_not_allowed(allowed: &str) -> Response {
    Response::refusal(
        405,
        &format!("this path answers `{allowed}`, and the contract declares no other method for it"),
    )
}

/// Reads one request, or the refusal that says why it could not be read.
///
/// # Errors
///
/// Never as an `Err` of the outer kind: everything that can go wrong with a request is an answer
/// the caller should receive, so the failure arm is the [`Response`] to send back.
pub fn read(reader: &mut std::io::BufReader<std::net::TcpStream>) -> Result<Request, Response> {
    let mut line = String::new();
    match reader.read_line(&mut line) {
        Ok(0) => {
            return Err(Response::refusal(
                400,
                "the connection closed before a request line arrived",
            ))
        }
        Ok(_) => {}
        Err(error) => {
            return Err(Response::refusal(
                400,
                &format!("the request line could not be read: {error}"),
            ))
        }
    }
    let mut parts = line.trim_end().split(' ');
    let method = parts.next().unwrap_or_default().to_owned();
    let target = parts.next().unwrap_or_default().to_owned();
    let version = parts.next().unwrap_or_default().to_owned();
    if method.is_empty() || target.is_empty() || !version.starts_with("HTTP/1.") {
        return Err(Response::refusal(
            400,
            "the request line is not `METHOD TARGET HTTP/1.1`",
        ));
    }
    let path = target
        .split('?')
        .next()
        .unwrap_or(target.as_str())
        .to_owned();

    let mut length = 0_usize;
    let mut chunked = false;
    loop {
        let mut header = String::new();
        match reader.read_line(&mut header) {
            Ok(0) => {
                return Err(Response::refusal(
                    400,
                    "the connection closed inside the headers",
                ))
            }
            Ok(_) => {}
            Err(error) => {
                return Err(Response::refusal(
                    400,
                    &format!("a header could not be read: {error}"),
                ))
            }
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        let Some((name, value)) = header.split_once(':') else {
            return Err(Response::refusal(400, "a header line has no `:`"));
        };
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();
        if name == "content-length" {
            match value.parse::<usize>() {
                Ok(parsed) => length = parsed,
                Err(_) => {
                    return Err(Response::refusal(
                        400,
                        "`Content-Length` is not a number of bytes",
                    ))
                }
            }
        } else if name == "transfer-encoding" && value.eq_ignore_ascii_case("chunked") {
            chunked = true;
        }
    }
    if chunked {
        return Err(Response::refusal(
            411,
            "this surface reads a body announced by `Content-Length`; chunked transfer is not read",
        ));
    }
    if length > MAX_BODY {
        return Err(Response::refusal(
            413,
            &format!("the body is {length} bytes and this surface reads at most {MAX_BODY}"),
        ));
    }
    let mut body = vec![0_u8; length];
    if let Err(error) = reader.read_exact(&mut body) {
        return Err(Response::refusal(
            400,
            &format!("the body was shorter than `Content-Length` announced: {error}"),
        ));
    }
    Ok(Request { method, path, body })
}

/// Writes one answer, and lets the connection close behind it.
///
/// # Errors
///
/// Whatever the socket refuses.
pub fn write(stream: &mut std::net::TcpStream, answer: &Response) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        answer.status,
        reason(answer.status),
        answer.content_type,
        answer.body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(answer.body.as_bytes())?;
    stream.flush()
}

/// The reason phrase for every status this surface can answer with.
///
/// Every one of them is either a status the contract declares for a branch, or one of the four this
/// surface answers about the request itself. A status not in this list is one nothing emits.
pub fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        411 => "Length Required",
        413 => "Content Too Large",
        422 => "Unprocessable Content",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        _ => "Unknown",
    }
}
