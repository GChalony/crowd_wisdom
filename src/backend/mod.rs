use dioxus::{logger::tracing, prelude::*};

#[post("/question")]
pub async fn send_answer(answer: String) -> Result<()> {
    tracing::info!("Sent answer {}", answer);
    Ok(())
}

#[get("/question")]
pub async fn get_question() -> Result<String> {
    Ok("Quelle est la température de fusion de l'aluminium ?".to_string())
}