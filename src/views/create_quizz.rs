use dioxus::{core::ElementId, html::div, logger::tracing, prelude::*};
use dioxus_free_icons::{Icon, icons::fa_solid_icons::{FaPlus, FaTrash}};

use crate::{Route, backend::create_quizz, views::home::{Column, Row}};
use crate::backend::{Quizz, Question};

#[component]
pub fn CreateQuizz() -> Element {
    let mut quizz = use_signal(|| Quizz::new());

    let add_question = move |q: Question| {
        quizz.write().questions.push(q);
    };

    rsx! {
        div { style: "
                display: flex;
                justify-content: center;
                align-items: baseline;
                min-height: 100vh;
                background: linear-gradient(135deg, #1e293b, #0f172a);
                font-family: Inter, sans-serif;
                color: white
            ",
            div {
                background_color: "rgba(255,255,255,0.08)",
                padding: "2em",
                border_radius: "1em",
                backdrop_filter: "blur(12px)",
                width: "100%",
                box_shadow: "0 10px 30px rgba(0,0,0,0.35)",

                h1 { "New Quizz" }
                for (i , question) in quizz.read().questions.iter().enumerate() {
                    ExistingQuestion {
                        question: question.clone(),
                        ondelete: move || {
                            quizz.write().questions.remove(i);
                        },
                    }
                }
                NewQuestion { on_add: add_question }
                div {
                    display: "flex",
                    align_items: "center",
                    justify_content: "center",
                    Create { quizz }
                }
            }
        }
    }
}

#[component]
pub fn ExistingQuestion(question: Question, ondelete: EventHandler<()>) -> Element {
    rsx! {
        div { display: "flex", width: "100%", margin: "1em",
            b { width: "40%", "{question.question}" }
            Divider {}
            span { style: "width: 40%", "{question.answer}" }
            Divider {}
            button {
                border: "none",
                background: "transparent",
                color: "inherit",
                cursor: "pointer",
                onclick: move |_| { ondelete.call(()) },
                Icon { icon: FaTrash }
            }
        }
    }
}

#[component]
pub fn NewQuestion(on_add: EventHandler<Question>) -> Element {
    let mut question_text = use_signal(|| String::new());
    let mut question_answer = use_signal(|| 0);

    let submit = move |_| {
        let q = Question {
            question: question_text(),
            answer: question_answer(),
        };
        on_add.call(q);
    };

    rsx! {
        div { display: "flex", padding: "1em 0", width: "100%",
            input {
                style: "width: 40%",
                r#type: "text",
                placeholder: "Enter question text...",
                value: "{question_text}",
                oninput: move |evt| question_text.set(evt.value()),
            }
            Divider {}
            input {
                style: "width: 40%",
                r#type: "text",
                placeholder: "Enter correct answer...",
                value: "{question_answer}",
                oninput: move |evt| question_answer.set(evt.value().parse::<i32>().unwrap()),
            }
            Divider {}
            button {
                style: "
                    background-color: #4a6bff;
                    color: white;
                    border: none;
                    border-radius: 4px;
                    padding: 0.5rem;
                    cursor: pointer;
                ",
                onclick: submit,
                Icon { icon: FaPlus }
            }
        }
    }
}


#[component]
fn Divider() -> Element {
    rsx! {
        div { margin_right: "1em" }
    }
}

#[component]
fn Create(quizz: Signal<Quizz>) -> Element {
    let navigator = navigator();
    rsx! {
        button {
            padding: "1em 3em",
            background_color: "#4a6bff",
            style: "
                    background-color: #4a6bff;
                    color: white;
                    border: none;
                    border-radius: 4px;
                    cursor: pointer;
                ",
            onclick: move |_| {
                tracing::info!("Creating quizz");
                async move {
                    let quizz_id = create_quizz(quizz.read().clone()).await.unwrap();
                    navigator.push(Route::Lobby { quizz_id });
                }
            },
            "Create"
        }
    }
}