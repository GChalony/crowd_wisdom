use dioxus::{
    fullstack::{Lazy, WebSocketOptions, Websocket},
    logger::tracing,
    prelude::*,
    CapturedError,
};
use serde::{Deserialize, Serialize};

#[cfg(feature = "server")]
use std::sync::Arc;
#[cfg(feature = "server")]
use tokio::sync::{watch, Mutex};

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

#[cfg(feature = "server")]
#[derive(Clone)]
struct AppState {
    answers: Arc<Mutex<Vec<i32>>>, // TODO : replace with DB
    tx: watch::Sender<usize>,
    rx: watch::Receiver<usize>,
}

#[cfg(feature = "server")]
static DATABASE: Lazy<AppState> = Lazy::new(|| async move {
    let (tx, rx) = watch::channel(0usize);
    let answers = Arc::new(Mutex::new(vec![]));
    dioxus::Ok(AppState { answers, tx, rx })
});

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

#[post("/create")]
pub async fn create_quizz(quizz: Quizz) -> Result<u32> {
    tracing::info!("Creating quizz {quizz:?}");
    DB.with(|mut conn| {
        let tx = conn.unchecked_transaction()?;
        // Generate a 4-digit public id
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

#[post("/question/:question_id")]
pub async fn send_answer(question_id: u32, answer: i32) -> Result<()> {
    tracing::info!("Sent answer {}", answer);
    let mut answers = DATABASE.answers.lock().await;
    answers.push(answer);
    DATABASE.tx.send(answers.len()).unwrap();
    DB.with(|con| {
        con.execute(
            "INSERT INTO answer (question_id, value) VALUES (?1, ?2)",
            (question_id, answer),
        )
        .unwrap()
    });
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
        let mut rx = DATABASE.rx.clone();
        let current_count = *rx.borrow();
        socket.send(current_count).await.unwrap();

        loop {
            rx.changed().await.unwrap();
            let count = *rx.borrow();
            _ = socket.send(count).await;
        }
    }))
}
