use std::{
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use serde_json::json;

use super::structs::Shared;
use crate::{
    http::{ReadError, Request, Response, read_request, write_response},
    protocol::{Outcome, handle, supported_version},
};

/// The only path the server answers.
const PATH: &str = "/mcp";
/// A client that sends nothing for this long is dropped.
const READ_TIMEOUT: Duration = Duration::from_secs(10);
/// More connections than one app needs: a session makes one call at a time.
const CONNECTIONS_MAX: usize = 64;

/// The accept loop. It ends when the gateway is dropped.
pub(super) fn serve(listener: TcpListener, shared: Arc<Shared>) {
    let open = Arc::new(AtomicUsize::new(0));
    for stream in listener.incoming() {
        if shared.stop.load(Ordering::SeqCst) {
            return;
        }
        let Ok(stream) = stream else { continue };
        if open.fetch_add(1, Ordering::SeqCst) >= CONNECTIONS_MAX {
            open.fetch_sub(1, Ordering::SeqCst);
            continue;
        }
        let (shared, open_here) = (shared.clone(), open.clone());
        let spawned = std::thread::Builder::new()
            .name("atelier-gateway-call".into())
            .spawn(move || {
                connection(stream, &shared);
                open_here.fetch_sub(1, Ordering::SeqCst);
            });
        if spawned.is_err() {
            open.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

fn connection(mut stream: TcpStream, shared: &Shared) {
    if stream.set_read_timeout(Some(READ_TIMEOUT)).is_err()
        || stream.set_write_timeout(Some(READ_TIMEOUT)).is_err()
    {
        return;
    }
    let response = match read_request(&mut stream) {
        Ok(request) => answer(&request, shared),
        Err(ReadError::Gone) => return,
        Err(ReadError::Malformed) => Response::empty(400),
        Err(ReadError::TooLarge) => Response::empty(413),
        Err(ReadError::Chunked) => Response::empty(411),
    };
    // A client that left has nothing to hear, and nothing to do about it.
    let _ = write_response(&mut stream, &response);
}

/// The checks in the order that gives an attacker the least: who may ask comes before what was asked.
fn answer(request: &Request, shared: &Shared) -> Response {
    // DNS rebinding: a page on another name that resolves to 127.0.0.1 still sends its own name in Host.
    if !host_is_ours(request.header("host"), shared.port) {
        return Response::empty(403);
    }
    // A browser always sends Origin; an agent never does. Anything but our own origin is a page, not an agent.
    if let Some(origin) = request.header("origin")
        && !origin_is_ours(origin, shared.port)
    {
        return Response::empty(403);
    }
    let Some(actor) = bearer(request).and_then(|token| shared.actor_of(token)) else {
        return Response::empty(401).with_header("WWW-Authenticate", "Bearer");
    };
    if request.path != PATH {
        return Response::empty(404);
    }
    // No event stream and no session to end: GET and DELETE have nothing to do here.
    if request.method != "POST" {
        return Response::empty(405).with_header("Allow", "POST");
    }
    if let Some(version) = request.header("mcp-protocol-version")
        && supported_version(version).is_none()
    {
        let body = json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32600, "message": format!("protocol version {version} is not supported") } });
        return Response::json(400, &body);
    }
    match handle(&request.body, &shared.tool_sets(), &actor) {
        Outcome::Reply(body) => Response::json(200, &body),
        Outcome::Accepted => Response::empty(202),
        Outcome::Refused(body) => Response::json(400, &body),
    }
}

fn bearer(request: &Request) -> Option<&str> {
    let value = request.header("authorization")?;
    let (scheme, token) = value.split_once(' ')?;
    scheme
        .eq_ignore_ascii_case("bearer")
        .then_some(token.trim())
}

fn host_is_ours(host: Option<&str>, port: u16) -> bool {
    host.is_some_and(|host| {
        host == format!("127.0.0.1:{port}") || host == format!("localhost:{port}")
    })
}

fn origin_is_ours(origin: &str, port: u16) -> bool {
    origin == format!("http://127.0.0.1:{port}") || origin == format!("http://localhost:{port}")
}
