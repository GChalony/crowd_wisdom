use dioxus::{fullstack::{Lazy, WebSocketOptions, Websocket}, logger::tracing, prelude::*};
#[cfg(feature = "server")]
use dioxus::fullstack::TypedWebsocket;


#[cfg(feature = "server")]
use std::sync::Arc;
use std::time::Duration;
#[cfg(feature = "server")]
use tokio::sync::{Mutex, watch};

#[cfg(feature = "server")]
#[derive(Clone)]
struct AppState {
    answers: Arc<Mutex<Vec<i32>>>,  // TODO : replace with DB
    tx: watch::Sender<usize>,
    rx: watch::Receiver<usize>
}

#[cfg(feature = "server")]
static DATABASE: Lazy<AppState> = Lazy::new(|| async move {
    let (tx, rx) = watch::channel(0usize);
    let answers = Arc::new(Mutex::new(vec![]));
    dioxus::Ok(
        AppState {answers, tx, rx}
    )
});

#[post("/question")]
pub async fn send_answer(answer: i32) -> Result<()> {
    tracing::info!("Sent answer {}", answer);
    let mut answers = DATABASE.answers.lock().await;
    answers.push(answer);
    DATABASE.tx.send(answers.len());
    Ok(())
}

#[get("/question")]
pub async fn get_question() -> Result<String> {
    Ok("Quelle est la température de fusion de l'aluminium ?".to_string())
}

#[get("/api/get_count/{question_id}")]
pub async fn get_count(question_id: u16, options: WebSocketOptions) -> Result<Websocket<usize, usize>> {
    tracing::info!("Creating websocket");

    Ok(options.on_upgrade(move |mut socket| async move {     
        let mut rx = DATABASE.rx.clone();
        let current_count = *rx.borrow();
        socket.send(current_count).await.unwrap();
        

        loop {
            rx.changed().await.unwrap();
            let count = *rx.borrow();
            _ = socket.send(count).await;
        };
    }))
}
