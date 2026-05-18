use dioxus::{fullstack::{Lazy, WebSocketOptions, Websocket}, logger::tracing, prelude::*};
#[cfg(feature = "server")]
use dioxus::fullstack::TypedWebsocket;


#[cfg(feature = "server")]
use std::sync::Arc;
use std::time::Duration;
#[cfg(feature = "server")]
use tokio::sync::{broadcast, Mutex};

#[cfg(feature = "server")]
#[derive(Clone)]
struct AppState {
    answers: Arc<Mutex<Vec<i32>>>,  // TODO : replace with DB
    tx: broadcast::Sender<usize>,
}

#[cfg(feature = "server")]
static DATABASE: Lazy<AppState> = Lazy::new(|| async move {
    let (tx, _) = broadcast::channel(16);
    let answers = Arc::new(Mutex::new(vec![]));
    dioxus::Ok(
        AppState {answers, tx}
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

#[get("/api/get_count")]
pub async fn get_count(options: WebSocketOptions) -> Result<Websocket<usize, usize>> {
    use tokio::time::sleep;

    let mut rx = DATABASE.tx.subscribe();
    tracing::info!("Creating websocket");

    Ok(options.on_upgrade(move |mut socket| async move {
        let mut count = 0;
        loop {
            tracing::info!("Count: {}", count);
            sleep(Duration::from_secs(1)).await;
            count += 1;
            socket.send(count).await;
        }

        while let Ok(count) = rx.recv().await {
            _ = socket.send(count).await;
        };
    }))
}
