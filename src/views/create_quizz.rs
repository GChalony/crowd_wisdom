use dioxus::{core::ElementId, html::div, logger::tracing, prelude::*};
use dioxus_free_icons::{
    icons::fa_solid_icons::{FaPlus, FaTrash},
    Icon,
};

use crate::backend::{Question, Quizz};
use crate::{
    backend::create_quizz,
    views::home::{Column, Row},
    Route,
};

#[component]
pub fn CreateQuizz() -> Element {
    let mut quizz = use_signal(|| Quizz::new());

    let add_question = move |q: Question| {
        quizz.write().questions.push(q);
    };

    rsx! {
        div { class: "create-page",
            div { class: "create-card",
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
        div { class: "question-row",
            span { style: "flex:2.5;", "{question.question}" }
            span { style: "flex:1;color:rgba(241,245,249,0.6);", "{question.answer}" }
            button {
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
        div { class: "new-question-row",
            input {
                r#type: "text",
                placeholder: "Question…",
                value: "{question_text}",
                oninput: move |evt| question_text.set(evt.value()),
            }
            input {
                r#type: "text",
                placeholder: "Answer",
                value: "{question_answer}",
                oninput: move |evt| question_answer.set(evt.value().parse::<i32>().unwrap_or_default()),
            }
            button { class: "btn-add", onclick: submit, Icon { icon: FaPlus } }
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
        button { class: "btn-submit-create",
            onclick: move |_| {
                async move {
                    // Grab (or create) the user's persistent UUID from localStorage.
                    // This runs after hydration so browser APIs are available.
                    let mut eval = document::eval(
                        r#"let id = localStorage.getItem('crowd_wisdom_user_id');
                        if (!id) {
                            id = crypto.randomUUID();
                            localStorage.setItem('crowd_wisdom_user_id', id);
                        }
                        dioxus.send(id);"#,
                    );
                    let creator_id = eval.recv::<String>().await.unwrap_or_default();
                    let quizz_id = create_quizz(quizz.read().clone(), creator_id).await.unwrap();
                    navigator.push(Route::Lobby { quizz_id });
                }
            },
            "Create"
        }
    }
}
