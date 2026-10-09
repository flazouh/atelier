use std::{
    collections::HashMap,
    io,
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::Path,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};

use atelier_capabilities::Actor;

use super::helpers::serve;
use crate::{Grant, McpConfig, SessionAccess, ToolSet, session::random_hex};

/// What the connection threads share with the gateway that started them.
pub(crate) struct Shared {
    pub(super) port: u16,
    pub(super) tokens: Mutex<HashMap<String, Actor>>,
    pub(super) sets: RwLock<Vec<Arc<dyn ToolSet>>>,
    pub(super) stop: AtomicBool,
}

impl Shared {
    pub(crate) fn revoke(&self, token: &str) {
        self.tokens
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(token);
    }

    /// Who a token names, when it is live.
    pub(super) fn actor_of(&self, token: &str) -> Option<Actor> {
        self.tokens
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(token)
            .cloned()
    }

    pub(super) fn tool_sets(&self) -> Vec<Arc<dyn ToolSet>> {
        self.sets.read().unwrap_or_else(|p| p.into_inner()).clone()
    }
}

/// The MCP server. It listens on 127.0.0.1, on a port the system picks, and stops when it is dropped.
///
/// Open connections are not waited for: each ends on its own with its read timeout, and holds nothing but the shared
/// state.
pub struct Gateway {
    shared: Arc<Shared>,
    accept: Option<JoinHandle<()>>,
}

impl Gateway {
    /// Starts the server with `sets` as its tools. More come with [`register`](Self::register).
    pub fn start(sets: Vec<Arc<dyn ToolSet>>) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr()?.port();
        let shared = Arc::new(Shared {
            port,
            tokens: Mutex::new(HashMap::new()),
            sets: RwLock::new(sets),
            stop: AtomicBool::new(false),
        });
        let serving = shared.clone();
        let accept = std::thread::Builder::new()
            .name("atelier-gateway".into())
            .spawn(move || serve(listener, serving))?;
        Ok(Self {
            shared,
            accept: Some(accept),
        })
    }

    /// Adds a capability's tools. A call that is already running does not see them.
    pub fn register(&self, set: Arc<dyn ToolSet>) {
        self.shared
            .sets
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .push(set);
    }

    /// `http://127.0.0.1:<port>/mcp`.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/mcp", self.shared.port)
    }

    /// A new token for one agent session. Every write made with it is attributed to `actor`.
    pub fn session(&self, actor: Actor) -> io::Result<SessionAccess> {
        let token = random_hex(32)?;
        self.shared
            .tokens
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(token.clone(), actor);
        Ok(SessionAccess {
            url: self.url(),
            token,
        })
    }

    /// Makes the token worthless: the next call with it gets 401. An unknown token is ignored.
    pub fn revoke(&self, token: &str) {
        self.shared.revoke(token);
    }

    /// A token and the config file that carries it, written into `dir`. They end together when the grant drops.
    pub fn grant(&self, actor: Actor, dir: &Path) -> io::Result<Grant> {
        let access = self.session(actor)?;
        // The token is live from here. If the file fails, the token must not stay.
        match McpConfig::write(dir, &access) {
            Ok(config) => Ok(Grant {
                shared: self.shared.clone(),
                access,
                config,
            }),
            Err(error) => {
                self.shared.revoke(&access.token);
                Err(error)
            }
        }
    }
}

impl Drop for Gateway {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        // The accept thread sleeps in `accept`. One connection wakes it, and it sees the flag.
        let _ = TcpStream::connect(SocketAddr::from((Ipv4Addr::LOCALHOST, self.shared.port)));
        if let Some(accept) = self.accept.take() {
            let _ = accept.join();
        }
    }
}
