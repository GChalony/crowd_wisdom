use crate::components::Echo;
use dioxus::logger::tracing;
use dioxus::{
    core::ElementId,
    html::{button::value, u::flex_direction},
    prelude::*,
};
use crate::backend::{get_question, send_answer};

/// The Home page component that will be rendered when the current route is `[Route::Home]`
#[component]
pub fn Home() -> Element {
    rsx! {
        Question {}
    }
}

#[component]
pub fn Question() -> Element {
    let answer: Signal<Option<i32>> = use_signal(|| Option::None);
    tracing::info!("Answered ? {:?}", *answer.read());

    let question = use_server_future(async || {get_question().await})?;

    rsx! {
        if answer.read().is_none() {
            PromptQuestion { question: question().unwrap().unwrap(), answer }
        } else {
            ShowAnswer { answer }
        }
    }
}

#[component]
fn PromptQuestion(question: String, answer: Signal<Option<i32>>) -> Element {
    let mut pending_answer = use_signal(|| Some(0));

    let fill_answer = move |text: Event<FormData>| {
        pending_answer.set(text.value().parse::<i32>().ok());
    };

    let submit_answer = move |_| async move {
        let resp = *pending_answer.read(); 
        
        send_answer(resp.map(|v| v.to_string()).unwrap_or("".to_string())).await.unwrap();
        answer.set(resp);
    };

    rsx! {
        div { style: "
                display: flex;
                justify-content: center;
                align-items: center;
                min-height: 100vh;
                background: linear-gradient(135deg, #1e293b, #0f172a);
                font-family: Inter, sans-serif;
            ",

            div { style: "
                    background: rgba(255,255,255,0.08);
                    backdrop-filter: blur(12px);
                    border: 1px solid rgba(255,255,255,0.12);
                    border-radius: 24px;
                    padding: 2rem;
                    width: 100%;
                    max-width: 420px;
                    box-shadow: 0 10px 30px rgba(0,0,0,0.35);
                    color: white;
                    text-align: center;
                ",

                h2 { style: "
                        margin-bottom: 0.5rem;
                        font-size: 1.8rem;
                        font-weight: 700;
                    ",
                    "🔥 Quiz Science"
                }

                p { style: "
                        margin-bottom: 1.5rem;
                        color: rgba(255,255,255,0.8);
                        font-size: 1rem;
                        line-height: 1.5;
                    ",
                    "{question}"
                }

                input {
                    style: "
                        width: 100%;
                        padding: 0.9rem 1rem;
                        border-radius: 14px;
                        border: none;
                        outline: none;
                        font-size: 1rem;
                        background: rgba(255,255,255,0.12);
                        color: white;
                        box-sizing: border-box;
                        transition: all 0.2s ease;
                    ",

                    r#type: "number",
                    placeholder: "Entrez votre réponse...",
                    oninput: fill_answer,
                    step: 100,

                    value: format!("{}", (*pending_answer.read()).map(|v| v.to_string()).unwrap_or_default()),
                }

                button {
                    style: "
                        margin-top: 1.5rem;
                        width: 100%;
                        padding: 0.9rem;
                        border: none;
                        border-radius: 14px;
                        background: linear-gradient(135deg, #3b82f6, #8b5cf6);
                        color: white;
                        font-size: 1rem;
                        font-weight: 600;
                        cursor: pointer;
                        transition: transform 0.15s ease, opacity 0.15s ease;
                    ",

                    onclick: submit_answer,

                    onmouseover: move |_| {},
                    "Validate"
                }
            }
        }
    }
}

#[component]
fn ShowAnswer(answer: Signal<Option<i32>>) -> Element {
    let resp = answer.read().clone().unwrap();

    let restart = move |_| async move {
        answer.set(None);
    };

    rsx! {
        div { style: "
                display: flex;
                justify-content: center;
                align-items: center;
                min-height: 100vh;
                background: linear-gradient(135deg, #1e293b, #0f172a);
                font-family: Inter, sans-serif;
                padding: 2rem;
            ",

            div { style: "
                    background: rgba(255,255,255,0.08);
                    backdrop-filter: blur(12px);
                    border: 1px solid rgba(255,255,255,0.12);
                    border-radius: 24px;
                    padding: 2rem;
                    width: 100%;
                    max-width: 420px;
                    box-shadow: 0 10px 30px rgba(0,0,0,0.35);
                    color: white;
                    text-align: center;
                ",

                div { style: "
                        font-size: 4rem;
                        margin-bottom: 1rem;
                    ",
                    "✅"
                }

                h2 { style: "
                        font-size: 1.8rem;
                        font-weight: 700;
                        margin-bottom: 0.5rem;
                    ",
                    "Answer submitted!"
                }

                p { style: "
                        color: rgba(255,255,255,0.75);
                        margin-bottom: 1rem;
                        font-size: 1rem;
                    ",
                    "Your answer:"
                }

                div { style: "
                        font-size: 3rem;
                        font-weight: 800;
                        margin-bottom: 1.5rem;
                        background: linear-gradient(135deg, #60a5fa, #a78bfa);
                        -webkit-background-clip: text;
                        -webkit-text-fill-color: transparent;
                    ",
                    "{resp}"
                }

                div { style: "
                        display: inline-flex;
                        align-items: center;
                        gap: 0.5rem;
                        padding: 0.75rem 1rem;
                        border-radius: 999px;
                        background: rgba(255,255,255,0.08);
                        color: rgba(255,255,255,0.8);
                        font-size: 0.95rem;
                        margin-bottom: 1.8rem;
                    ",

                    span { "⏳" }
                    span { "Waiting for others..." }
                }

                button {
                    style: "
                        width: 100%;
                        padding: 0.9rem;
                        border: none;
                        border-radius: 14px;
                        background: linear-gradient(135deg, #ef4444, #f97316);
                        color: white;
                        font-size: 1rem;
                        font-weight: 600;
                        cursor: pointer;
                        transition: all 0.2s ease;
                        box-shadow: 0 6px 20px rgba(239,68,68,0.35);
                    ",

                    onclick: restart,

                    "Restart"
                }
            }
        }
    }
}

#[component]
pub fn Column(children: Element) -> Element {
    rsx! {
        div { display: "flex", flex_direction: "column", {children} }
    }
}



#[component]
pub fn Row(children: Element) -> Element {
    rsx! {
        div { display: "flex", flex_direction: "row", {children} }
    }
}