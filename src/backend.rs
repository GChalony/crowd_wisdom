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
                creator_id TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS question (
                id INTEGER PRIMARY KEY,
                quizz_id INTEGER,
                kind TEXT NOT NULL DEFAULT 'numeric',
                text TEXT NOT NULL,
                answer INTEGER,
                position INTEGER NOT NULL,
                FOREIGN KEY (quizz_id)
                    REFERENCES quizz (id)
                        ON DELETE CASCADE
                        ON UPDATE NO ACTION
            );
            CREATE TABLE IF NOT EXISTS choice (
                id INTEGER PRIMARY KEY,
                question_id INTEGER NOT NULL,
                text TEXT NOT NULL,
                position INTEGER NOT NULL,
                FOREIGN KEY (question_id)
                    REFERENCES question (id)
                        ON DELETE CASCADE
                        ON UPDATE NO ACTION
            );
            CREATE TABLE IF NOT EXISTS answer (
                id INTEGER PRIMARY KEY,
                question_id INTEGER,
                user_name TEXT NOT NULL DEFAULT '',
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
    pub is_admin: bool,
}

/// One player's submitted answer, included in the Revealed phase broadcast.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct IndividualAnswer {
    pub name: String,
    pub value: i32,
}

/// How a question is answered. New variants can be added without DB migration
/// (the `kind` column is a freetext tag).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
pub enum QuestionKind {
    #[default]
    Numeric,
    /// Multiple-choice: `options` are the displayed labels; `answer` in the DB
    /// stores the 0-based index of the correct option.
    Choice { options: Vec<String> },
}

/// Crowd-aggregated result, produced at reveal time and broadcast to all clients.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum CrowdResult {
    /// Mean of all numeric submissions.
    Mean { average: f64 },
    /// Per-option vote counts; `winners` contains all tied top-vote indices.
    Votes {
        counts: Vec<usize>,
        winners: Vec<usize>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum LobbyUpdate {
    Players(Vec<User>),
    GameStarted,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum PlayPhase {
    Answering,
    Revealed {
        correct_answer: i32,
        answers: Vec<IndividualAnswer>,
        crowd_result: CrowdResult,
    },
    /// Broadcast by the admin after the last question. All clients navigate home.
    Finished,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PlayState {
    pub position: u32,
    pub total: u32,
    pub question: String,
    pub kind: QuestionKind,
    pub answer_count: usize,
    pub phase: PlayPhase,
}

// ---------------------------------------------------------------------------
// Server-only state
// ---------------------------------------------------------------------------

/// Per-quizz broadcast channel + current user list.
#[cfg(feature = "server")]
struct LobbyChannels {
    /// Stored UUID string of the quiz creator; used to set `is_admin` on join.
    creator_id: String,
    users: Mutex<Vec<User>>,
    tx: watch::Sender<LobbyUpdate>,
    rx: watch::Receiver<LobbyUpdate>,
}

#[cfg(feature = "server")]
struct GameChannels {
    creator_id: String,
    /// In-memory answer buffer — source of truth for the active question.
    /// Using RAM avoids any DB thread-local / timing issues.
    answers: Mutex<Vec<IndividualAnswer>>,
    tx: watch::Sender<Option<PlayState>>,
    rx: watch::Receiver<Option<PlayState>>,
}

#[cfg(feature = "server")]
#[derive(Clone)]
struct AppState {
    /// One entry per active quizz lobby.
    lobbies: Arc<Mutex<HashMap<u32, Arc<LobbyChannels>>>>,
    /// One entry per active game.
    games: Arc<Mutex<HashMap<u32, Arc<GameChannels>>>>,
}

#[cfg(feature = "server")]
impl AppState {
    async fn get_or_create_lobby(&self, quizz_id: u32, creator_id: String) -> Arc<LobbyChannels> {
        let mut lobbies = self.lobbies.lock().await;
        if let Some(lobby) = lobbies.get(&quizz_id) {
            return lobby.clone();
        }
        let (tx, rx) = watch::channel(LobbyUpdate::Players(vec![]));
        let lobby = Arc::new(LobbyChannels {
            creator_id,
            users: Mutex::new(vec![]),
            tx,
            rx,
        });
        lobbies.insert(quizz_id, lobby.clone());
        lobby
    }

    async fn get_lobby(&self, quizz_id: u32) -> Option<Arc<LobbyChannels>> {
        let lobbies = self.lobbies.lock().await;
        lobbies.get(&quizz_id).cloned()
    }

    async fn get_or_create_game(&self, quizz_id: u32, creator_id: String) -> Arc<GameChannels> {
        let mut games = self.games.lock().await;
        if let Some(game) = games.get(&quizz_id) {
            return game.clone();
        }
        let (tx, rx) = watch::channel(None);
        let game = Arc::new(GameChannels {
            creator_id,
            answers: Mutex::new(vec![]),
            tx,
            rx,
        });
        games.insert(quizz_id, game.clone());
        game
    }

    async fn get_game(&self, quizz_id: u32) -> Option<Arc<GameChannels>> {
        let games = self.games.lock().await;
        games.get(&quizz_id).cloned()
    }
}

#[cfg(feature = "server")]
static STATE: Lazy<AppState> = Lazy::new(|| async move {
    dioxus::Ok(AppState {
        lobbies: Arc::new(Mutex::new(HashMap::new())),
        games: Arc::new(Mutex::new(HashMap::new())),
    })
});

// ---------------------------------------------------------------------------
// Quizz data types
// ---------------------------------------------------------------------------

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub question: String,
    pub answer: i32,
    pub kind: QuestionKind,
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

/// Load question text, kind (with choices), and DB row id for a given
/// quizz public_id + position. Returns None if the question doesn't exist.
#[cfg(feature = "server")]
fn load_question_data(
    conn: &rusqlite::Connection,
    quizz_public_id: u32,
    position: u32,
) -> Option<(String, QuestionKind, i64)> {
    let (text, kind_tag, question_id): (String, String, i64) = conn
        .query_row(
            "SELECT text, kind, question.id FROM question
             JOIN quizz ON question.quizz_id = quizz.id
             WHERE quizz.public_id = ?1 AND question.position = ?2",
            (quizz_public_id, position),
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .ok()?;

    let kind = if kind_tag == "choice" {
        let options: Vec<String> = conn
            .prepare("SELECT text FROM choice WHERE question_id = ?1 ORDER BY position")
            .ok()?
            .query_map([question_id], |row| row.get(0))
            .ok()?
            .filter_map(|r| r.ok())
            .collect();
        QuestionKind::Choice { options }
    } else {
        QuestionKind::Numeric
    };

    Some((text, kind, question_id))
}

#[post("/create")]
pub async fn create_quizz(quizz: Quizz, creator_id: String) -> Result<u32> {
    tracing::info!("Creating quizz {quizz:?}");
    DB.with(|conn| {
        let tx = conn.unchecked_transaction()?;
        let public_id: u32 = rand::random_range(1000..9999);
        tx.execute(
            "INSERT INTO quizz (public_id, creator_id) VALUES (?1, ?2)",
            (public_id, &creator_id),
        )?;
        let quizz_id = tx.last_insert_rowid();

        for (i, question) in quizz.questions.into_iter().enumerate() {
            tx.execute(
                "INSERT INTO question (quizz_id, kind, text, answer, position) VALUES (?1, ?2, ?3, ?4, ?5)",
                (
                    quizz_id,
                    match &question.kind { QuestionKind::Numeric => "numeric", QuestionKind::Choice { .. } => "choice" },
                    &question.question,
                    question.answer,
                    i,
                ),
            ).unwrap();
            let question_db_id = tx.last_insert_rowid();

            if let QuestionKind::Choice { options } = &question.kind {
                for (j, option) in options.iter().enumerate() {
                    tx.execute(
                        "INSERT INTO choice (question_id, text, position) VALUES (?1, ?2, ?3)",
                        (question_db_id, option.as_str(), j),
                    ).unwrap();
                }
            }
        }
        tx.commit().unwrap();

        Ok::<_, CapturedError>(public_id)
    })
}

/// WebSocket endpoint for the lobby.
#[get("/api/lobby/:quizz_id/:user_id/:user_name")]
pub async fn get_lobby_state(
    quizz_id: u32,
    user_id: String,
    user_name: String,
    options: WebSocketOptions,
) -> Result<Websocket<String, LobbyUpdate>> {
    // Synchronous DB lookup — no await yet, so thread_local DB is safe.
    let creator_id: String = DB.with(|conn| {
        conn.query_row(
            "SELECT creator_id FROM quizz WHERE public_id = ?1",
            [quizz_id],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_default()
    });

    let is_admin = user_id == creator_id;
    let connection_id = Uuid::new_v4();
    let lobby = STATE.get_or_create_lobby(quizz_id, creator_id).await;

    Ok(options.on_upgrade(move |mut socket| async move {
        // --- register user ---
        {
            let mut users = lobby.users.lock().await;
            users.push(User {
                id: connection_id,
                name: user_name,
                is_admin,
            });
            let _ = lobby.tx.send(LobbyUpdate::Players(users.clone()));
        }

        let mut rx = lobby.rx.clone();
        let mut game_started = false;

        // Send current state immediately
        let initial = rx.borrow_and_update().clone();
        if socket.send(initial).await.is_ok() {
            loop {
                match rx.changed().await {
                    Err(_) => break,
                    Ok(()) => {
                        let update = rx.borrow_and_update().clone();
                        let is_game_started = update == LobbyUpdate::GameStarted;
                        if socket.send(update).await.is_err() {
                            break;
                        }
                        if is_game_started {
                            game_started = true;
                            break;
                        }
                    }
                }
            }
        }

        // --- remove user on disconnect (only if game hasn't started) ---
        if !game_started {
            let mut users = lobby.users.lock().await;
            users.retain(|u| u.id != connection_id);
            let _ = lobby.tx.send(LobbyUpdate::Players(users.clone()));
        }
    }))
}

#[get("/api/is_admin/:quizz_id/:user_id")]
pub async fn check_is_admin(quizz_id: u32, user_id: String) -> Result<bool> {
    let creator_id: String = DB.with(|conn| {
        conn.query_row(
            "SELECT creator_id FROM quizz WHERE public_id = ?1",
            [quizz_id],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_default()
    });
    Ok(user_id == creator_id)
}

#[post("/api/start")]
pub async fn admin_start_game(quizz_id: u32, user_id: String) -> Result<()> {
    let creator_id: String = DB.with(|conn| {
        conn.query_row(
            "SELECT creator_id FROM quizz WHERE public_id = ?1",
            [quizz_id],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_default()
    });

    if user_id != creator_id {
        return Ok(());
    }

    let (question_text, question_kind, total) = DB.with(|conn| {
        let (text, kind, _) = load_question_data(conn, quizz_id, 0).unwrap_or_default();
        let total: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM question
                 JOIN quizz ON question.quizz_id = quizz.id
                 WHERE quizz.public_id = ?1",
                [quizz_id],
                |row| row.get::<_, u32>(0),
            )
            .unwrap_or(0);
        (text, kind, total)
    });

    let play_state = PlayState {
        position: 0,
        total,
        question: question_text,
        kind: question_kind,
        answer_count: 0,
        phase: PlayPhase::Answering,
    };

    let game = STATE.get_or_create_game(quizz_id, creator_id).await;
    game.answers.lock().await.clear();
    let _ = game.tx.send(Some(play_state));

    if let Some(lobby) = STATE.get_lobby(quizz_id).await {
        let _ = lobby.tx.send(LobbyUpdate::GameStarted);
    }

    Ok(())
}

#[get("/api/play/:quizz_id")]
pub async fn get_play_state(
    quizz_id: u32,
    options: WebSocketOptions,
) -> Result<Websocket<String, PlayState>> {
    let creator_id: String = DB.with(|conn| {
        conn.query_row(
            "SELECT creator_id FROM quizz WHERE public_id = ?1",
            [quizz_id],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_default()
    });

    let game = STATE.get_or_create_game(quizz_id, creator_id).await;

    Ok(options.on_upgrade(move |mut socket| async move {
        let mut rx = game.rx.clone();

        let initial = rx.borrow_and_update().clone();
        if let Some(state) = initial {
            if socket.send(state).await.is_err() {
                return;
            }
        }

        loop {
            match rx.changed().await {
                Err(_) => break,
                Ok(()) => {
                    let update = rx.borrow_and_update().clone();
                    if let Some(state) = update {
                        if socket.send(state).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    }))
}

#[post("/api/answer")]
pub async fn submit_answer(
    quizz_id: u32,
    position: u32,
    value: i32,
    user_name: String,
) -> Result<()> {
    let game = match STATE.get_game(quizz_id).await {
        Some(g) => g,
        None => return Ok(()),
    };

    let current_state = game.rx.borrow().clone();
    let state = match current_state {
        None => return Ok(()),
        Some(s) => s,
    };

    if state.position != position {
        return Ok(());
    }
    if matches!(state.phase, PlayPhase::Revealed { .. }) {
        return Ok(());
    }

    // 1. Push to the in-memory buffer — this is the source of truth for
    //    admin_reveal, so it is never subject to a DB timing race.
    let new_count = {
        let mut guard = game.answers.lock().await;
        guard.push(IndividualAnswer {
            name: user_name.clone(),
            value,
        });
        guard.len()
    };

    // 2. Broadcast the updated count immediately.
    let mut new_state = state;
    new_state.answer_count = new_count;
    let _ = game.tx.send(Some(new_state));

    // 3. Persist to DB (best-effort: a failure here is logged but never panics,
    //    because the in-memory buffer is the source of truth).
    DB.with(|conn| {
        let question_id: i64 = conn
            .query_row(
                "SELECT question.id FROM question
                 JOIN quizz ON question.quizz_id = quizz.id
                 WHERE quizz.public_id = ?1 AND question.position = ?2",
                (quizz_id, position),
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(-1);

        if question_id >= 0 {
            if let Err(e) = conn.execute(
                "INSERT INTO answer (question_id, user_name, value) VALUES (?1, ?2, ?3)",
                (question_id, user_name.as_str(), value),
            ) {
                tracing::warn!("Failed to persist answer to DB: {e}");
            }
        }
    });

    Ok(())
}

#[post("/api/reveal")]
pub async fn admin_reveal(quizz_id: u32, user_id: String) -> Result<()> {
    let creator_id: String = DB.with(|conn| {
        conn.query_row(
            "SELECT creator_id FROM quizz WHERE public_id = ?1",
            [quizz_id],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_default()
    });

    if user_id != creator_id {
        return Ok(());
    }

    let game = match STATE.get_game(quizz_id).await {
        Some(g) => g,
        None => return Ok(()),
    };

    let current_state = game.rx.borrow().clone();
    let state = match current_state {
        None => return Ok(()),
        Some(s) => s,
    };

    if matches!(state.phase, PlayPhase::Revealed { .. }) {
        return Ok(());
    }

    // Correct answer comes from the DB (one synchronous read, no write — safe).
    let correct_answer: i32 = DB.with(|conn| {
        conn.query_row(
            "SELECT answer FROM question
             JOIN quizz ON question.quizz_id = quizz.id
             WHERE quizz.public_id = ?1 AND question.position = ?2",
            (quizz_id, state.position),
            |row| row.get::<_, i32>(0),
        )
        .unwrap_or(0)
    });

    // Answers come from the in-memory buffer — no DB race possible.
    let answers: Vec<IndividualAnswer> = game.answers.lock().await.clone();

    // Sort by distance to correct answer (for numeric; for choice, sort by value for consistency)
    let mut sorted_answers = answers;
    match &state.kind {
        QuestionKind::Numeric => {
            sorted_answers.sort_by_key(|a| (a.value - correct_answer).abs());
        }
        QuestionKind::Choice { .. } => {
            sorted_answers.sort_by_key(|a| a.value);
        }
    }

    let crowd_result = match &state.kind {
        QuestionKind::Numeric => {
            let average = if sorted_answers.is_empty() {
                0.0
            } else {
                sorted_answers.iter().map(|a| a.value as f64).sum::<f64>()
                    / sorted_answers.len() as f64
            };
            CrowdResult::Mean { average }
        }
        QuestionKind::Choice { options } => {
            let n = options.len();
            let mut counts = vec![0usize; n];
            for a in &sorted_answers {
                let idx = a.value as usize;
                if idx < n {
                    counts[idx] += 1;
                }
            }
            let max_votes = counts.iter().max().copied().unwrap_or(0);
            let winners: Vec<usize> = if max_votes == 0 {
                vec![]
            } else {
                counts
                    .iter()
                    .enumerate()
                    .filter(|(_, &c)| c == max_votes)
                    .map(|(i, _)| i)
                    .collect()
            };
            CrowdResult::Votes { counts, winners }
        }
    };

    let mut new_state = state.clone();
    new_state.phase = PlayPhase::Revealed {
        correct_answer,
        answers: sorted_answers,
        crowd_result,
    };
    let _ = game.tx.send(Some(new_state));

    Ok(())
}

#[post("/api/next")]
pub async fn admin_next(quizz_id: u32, user_id: String) -> Result<()> {
    let creator_id: String = DB.with(|conn| {
        conn.query_row(
            "SELECT creator_id FROM quizz WHERE public_id = ?1",
            [quizz_id],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_default()
    });

    if user_id != creator_id {
        return Ok(());
    }

    let game = match STATE.get_game(quizz_id).await {
        Some(g) => g,
        None => return Ok(()),
    };

    let current_state = game.rx.borrow().clone();
    let state = match current_state {
        None => return Ok(()),
        Some(s) => s,
    };

    if state.position + 1 >= state.total {
        return Ok(());
    }

    let next_position = state.position + 1;
    let (next_question, next_kind) = DB.with(|conn| {
        load_question_data(conn, quizz_id, next_position)
            .map(|(text, kind, _)| (text, kind))
            .unwrap_or_default()
    });

    game.answers.lock().await.clear();

    let new_state = PlayState {
        position: next_position,
        total: state.total,
        question: next_question,
        kind: next_kind,
        answer_count: 0,
        phase: PlayPhase::Answering,
    };
    let _ = game.tx.send(Some(new_state));

    Ok(())
}

/// Admin ends the quiz. Broadcasts `PlayPhase::Finished` to every connected
/// play-page client, which causes them to navigate back to the home page.
#[post("/api/finish")]
pub async fn admin_finish(quizz_id: u32, user_id: String) -> Result<()> {
    let creator_id: String = DB.with(|conn| {
        conn.query_row(
            "SELECT creator_id FROM quizz WHERE public_id = ?1",
            [quizz_id],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_default()
    });

    if user_id != creator_id {
        return Ok(());
    }

    let game = match STATE.get_game(quizz_id).await {
        Some(g) => g,
        None => return Ok(()),
    };

    // Only valid from the Revealed phase of the last question.
    let current = game.rx.borrow().clone();
    if let Some(state) = current {
        if matches!(state.phase, PlayPhase::Revealed { .. }) && state.position + 1 >= state.total {
            let finished = PlayState {
                phase: PlayPhase::Finished,
                ..state
            };
            let _ = game.tx.send(Some(finished));
        }
    }

    Ok(())
}
