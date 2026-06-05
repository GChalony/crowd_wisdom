use dioxus::{
    fullstack::{Lazy, WebSocketOptions, Websocket},
    logger::tracing,
    prelude::*,
    CapturedError,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "server")]
use std::{collections::HashMap, sync::Arc};
#[cfg(feature = "server")]
use tokio::sync::{watch, Mutex};

// ---------------------------------------------------------------------------
// Database
// ---------------------------------------------------------------------------

#[cfg(feature = "server")]
thread_local! {
    pub static DB: rusqlite::Connection = {
        let conn = rusqlite::Connection::open("crowd_wisdom.db").expect("Failed to open database");

        conn.execute_batch(
            "BEGIN;
            CREATE TABLE IF NOT EXISTS quizz (
                id INTEGER PRIMARY KEY,
                public_id INTEGER NOT NULL UNIQUE,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS question (
                id INTEGER PRIMARY KEY,
                quizz_id INTEGER,
                text TEXT NOT NULL,
                answer INTEGER,
                position INTEGER NOT NULL,
                FOREIGN KEY (quizz_id)
                    REFERENCES quizz (id)
                        ON DELETE CASCADE
                        ON UPDATE NO ACTION
            );
            CREATE TABLE IF NOT EXISTS answer (
                id INTEGER PRIMARY KEY,
                question_id INTEGER,
                value INTEGER,
                FOREIGN KEY (question_id)
                    REFERENCES question (id)
                        ON DELETE CASCADE
                        ON UPDATE NO ACTION
            );
            COMMIT;",
        ).unwrap();

        conn
    };
}

// ---------------------------------------------------------------------------
// Shared types (compiled on both client and server)
// ---------------------------------------------------------------------------

/// A connected lobby participant. Sent from the server to all clients.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct User {
    pub id: Uuid,
    pub name: String,
}

// ---------------------------------------------------------------------------
// Server-only state
// ---------------------------------------------------------------------------

/// Per-quizz broadcast channel + current user list.
#[cfg(feature = "server")]
struct LobbyChannels {
    users: Mutex<Vec<User>>,
    tx: watch::Sender<Vec<User>>,
    rx: watch::Receiver<Vec<User>>,
}

#[cfg(feature = "server")]
#[derive(Clone)]
struct AppState {
    /// For the existing answer-count WebSocket.
    count_tx: watch::Sender<usize>,
    count_rx: watch::Receiver<usize>,
    /// One entry per active quizz lobby.
    lobbies: Arc<Mutex<HashMap<u32, Arc<LobbyChannels>>>>,
}

#[cfg(feature = "server")]
impl AppState {
    async fn get_or_create_lobby(&self, quizz_id: u32) -> Arc<LobbyChannels> {
        let mut lobbies = self.lobbies.lock().await;
        if let Some(lobby) = lobbies.get(&quizz_id) {
            return lobby.clone();
        }
        let (tx, rx) = watch::channel(vec![]);
        let lobby = Arc::new(LobbyChannels {
            users: Mutex::new(vec![]),
            tx,
            rx,
        });
        lobbies.insert(quizz_id, lobby.clone());
        lobby
    }
}

#[cfg(feature = "server")]
static STATE: Lazy<AppState> = Lazy::new(|| async move {
    let (count_tx, count_rx) = watch::channel(0usize);
    dioxus::Ok(AppState {
        count_tx,
        count_rx,
        lobbies: Arc::new(Mutex::new(HashMap::new())),
    })
});

// ---------------------------------------------------------------------------
// Quizz data types
// ---------------------------------------------------------------------------

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub question: String,
    pub answer: i32,
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct Quizz {
    pub questions: Vec<Question>,
}

impl Quizz {
    pub fn new() -> Quizz {
        Quizz { questions: vec![] }
    }
}

// ---------------------------------------------------------------------------
// Server functions
// ---------------------------------------------------------------------------

#[post("/create")]
pub async fn create_quizz(quizz: Quizz) -> Result<u32> {
    tracing::info!("Creating quizz {quizz:?}");
    DB.with(|mut conn| {
        let tx = conn.unchecked_transaction()?;
        let public_id: u32 = rand::random_range(1000..9999);
        tx.execute("INSERT INTO quizz (public_id) VALUES (?1)", (public_id,))?;
        let quizz_id = tx.last_insert_rowid();

        for (i, question) in quizz.questions.into_iter().enumerate() {
            tx.execute(
                "INSERT INTO question (quizz_id, text, answer, position) VALUES (?1, ?2, ?3, ?4)",
                (quizz_id, question.question, question.answer, i),
            )
            .unwrap();
        }
        tx.commit().unwrap();

        Ok::<_, CapturedError>(public_id)
    })
}

/// WebSocket endpoint for the lobby.
///
/// The client connects with `quizz_id` (path) and `user_name` (path).
/// The server assigns a fresh UUID to the connection, adds the user to the
/// lobby broadcast, streams `Vec<User>` on every change, and removes the
/// user when the socket is closed.
#[get("/api/lobby/:quizz_id/:user_name")]
pub async fn get_lobby_state(
    quizz_id: u32,
    user_name: String,
    options: WebSocketOptions,
) -> Result<Websocket<String, Vec<User>>> {
    // Each connection gets its own UUID so cleanup is unambiguous.
    let connection_id = Uuid::new_v4();
    let lobby = STATE.get_or_create_lobby(quizz_id).await;

    Ok(options.on_upgrade(move |mut socket| async move {
        // --- register user ---
        {
            let mut users = lobby.users.lock().await;
            users.push(User {
                id: connection_id,
                name: user_name,
            });
            let _ = lobby.tx.send(users.clone());
        }

        let mut rx = lobby.rx.clone();

        // Send current state immediately (mark as seen so `changed()` only
        // fires on genuinely new updates).
        let initial = rx.borrow_and_update().clone();
        if socket.send(initial).await.is_ok() {
            // Stream all future updates until the socket dies.
            loop {
                match rx.changed().await {
                    Err(_) => break,
                    Ok(()) => {
                        let users = rx.borrow_and_update().clone();
                        if socket.send(users).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }

        // --- remove user on disconnect ---
        let mut users = lobby.users.lock().await;
        users.retain(|u| u.id != connection_id);
        let _ = lobby.tx.send(users.clone());
    }))
}

#[post("/question/:question_id")]
pub async fn send_answer(question_id: u32, answer: i32) -> Result<()> {
    tracing::info!("Sent answer {}", answer);
    DB.with(|con| {
        con.execute(
            "INSERT INTO answer (question_id, value) VALUES (?1, ?2)",
            (question_id, answer),
        )
        .unwrap()
    });
    // TODO emit number of answers to all websockets
    Ok(())
}

#[get("/question/:quizz_id/:position")]
pub async fn get_question(quizz_id: u32, position: u32) -> Result<String> {
    let res: String = DB.with(|conn| {
        conn.prepare(
            "SELECT text FROM quizz
             JOIN question ON question.quizz_id = quizz.id
             WHERE public_id == (?1)",
        )
        .unwrap()
        .query([quizz_id])
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .get(0)
        .unwrap()
    });
    Ok(res)
}

#[get("/api/get_count/{question_id}")]
pub async fn get_count(
    question_id: u16,
    options: WebSocketOptions,
) -> Result<Websocket<usize, usize>> {
    tracing::info!("Creating websocket");

    Ok(options.on_upgrade(move |mut socket| async move {
        let mut rx = STATE.count_rx.clone();
        let current_count = *rx.borrow();
        socket.send(current_count).await.unwrap();

        loop {
            rx.changed().await.unwrap();
            let count = *rx.borrow();
            _ = socket.send(count).await;
        }
    }))
}
