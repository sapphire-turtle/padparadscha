//! client ↔ game server (spec §8, §9). Local, mobile and browser differ in the route only; the
//! engine is a plugin, so this is a Router the binaries bind, never a binary of its own. The
//! gate at `lib.rs` is what lets wasm read the crate without reading any of this (Task 11).

mod rpc;
mod ticket;
mod ws;

pub use rpc::routes;
pub use ticket::{DEFAULT_TTL, Tickets};

use crate::session::Session;
use crate::toss::CoinToss;
use padparadscha_common::{Log, Queue, Seed, Seq};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// What the two routes serve: the one session, its seed, its queue and log, the tickets, the
/// capability the commands run through, and the seq the feed carries from.
pub struct State {
    pub session: Mutex<Session>,
    pub log: Mutex<Log>,
    pub queue: Mutex<Queue>,
    pub tickets: Mutex<Tickets>,
    /// The capability, over the table the session was made on (Tasks 6 and 8's types, table's own).
    pub coin_toss: CoinToss,
    /// What `session.join` answered `rows` up to (the next seq the client had), so `GET /ws` knows
    /// where to continue (spec §7.3). `None` until a join: then the feed starts at the log's own first.
    pub feed_from: Mutex<Option<Seq>>,
    /// Wake feed subscribers as soon as a row is appended.
    pub appended: tokio::sync::Notify,
}

impl State {
    /// The binaries' constructor (Task 12’s mains): the session made once with the seed made once, and
    /// the two empty sequences that one command’s one row rides between (§7.1).
    pub fn new(session: Session) -> Arc<Self> {
        Self::for_test(
            session.id.clone(),
            session.seed,
            Log::default(),
            Queue::continuing_from(Seq(0)),
        )
    }

    /// The tests' constructor: a session already made, its seed already decided, its log and the queue
    /// that filled it both already filled — the queue carries its own counter, which is what `next_seq` reads.
    pub fn for_test(id: String, seed: Seed, log: Log, queue: Queue) -> Arc<Self> {
        Arc::new(Self {
            // MEASURED (Task 8): CoinToss is a struct with a public `table`, no `Default`.
            coin_toss: CoinToss {
                table: crate::Table::default(),
            },
            feed_from: Mutex::new(None),
            appended: tokio::sync::Notify::new(),
            tickets: Mutex::new(Tickets::default()),
            session: Mutex::new(Session::new(id, seed)),
            log: Mutex::new(log),
            queue: Mutex::new(queue),
        })
    }

    /// The seq the next command is tagged with, without accepting it (what `session.join` answers with): the
    /// queue counter's own, never a length a drained queue would leave at zero.
    pub fn next_seq(&self) -> Seq {
        self.queue.lock().unwrap().next
    }

    /// The time the two routes read the clock through, so a test can hand a `now` 31 seconds on.
    pub fn now(&self) -> Instant {
        Instant::now()
    }
}

/// A running instance of the shared API and its actual bound address.
pub struct Server {
    addr: std::net::SocketAddr,
    task: tokio::task::JoinHandle<std::io::Result<()>>,
}

impl Server {
    /// The actual address, including the OS-selected port when binding port zero.
    pub fn addr(&self) -> std::net::SocketAddr {
        self.addr
    }

    /// Wait for the server to stop.
    pub async fn wait_for_shutdown(self) -> std::io::Result<()> {
        self.task.await.map_err(std::io::Error::other)?
    }
}

/// Bind the same router all clients use, keeping its one session for its lifetime.
pub async fn serve_sim(session: Session, bind: std::net::SocketAddr) -> std::io::Result<Server> {
    let listener = tokio::net::TcpListener::bind(bind).await?;
    let addr = listener.local_addr()?;
    let task =
        tokio::spawn(async move { axum::serve(listener, routes(State::new(session))).await });
    Ok(Server { addr, task })
}
