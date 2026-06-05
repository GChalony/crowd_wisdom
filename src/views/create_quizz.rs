use dioxus::prelude::*;
use dioxus_free_icons::{
    icons::fa_solid_icons::{FaPlus, FaTrash},
    Icon,
};

use crate::backend::{create_quizz, Question, QuestionKind, Quizz};
use crate::Route;

// ─────────────────────────────────────────────────────────────────────────────
// Page
// ─────────────────────────────────────────────────────────────────────────────

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

                // List of already-added questions
                for (i, question) in quizz.read().questions.iter().enumerate() {
                    ExistingQuestion {
                        question: question.clone(),
                        ondelete: move || { quizz.write().questions.remove(i); },
                    }
                }

                NewQuestion { on_add: add_question }

                div { display: "flex", align_items: "center", justify_content: "center",
                    Create { quizz }
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Existing question row
// ─────────────────────────────────────────────────────────────────────────────

#[component]
pub fn ExistingQuestion(question: Question, ondelete: EventHandler<()>) -> Element {
    rsx! {
        div { class: "question-row",
            // Question text
            span { style: "flex:2.5;", "{question.question}" }

            // Kind + answer display
            {
                match &question.kind {
                    QuestionKind::Numeric => rsx! {
                        span { style: "flex:1;color:rgba(241,245,249,0.6);", "= {question.answer}" }
                        span { style: "font-size:0.75rem;color:rgba(241,245,249,0.35);padding:0 0.5rem;", "numeric" }
                    },
                    QuestionKind::Choice { options } => rsx! {
                        div { style: "flex:1;display:flex;flex-direction:column;gap:0.2rem;",
                            for (i, opt) in options.iter().enumerate() {
                                span {
                                    key: "{i}",
                                    style: if i as i32 == question.answer {
                                        "font-size:0.8rem;color:#34d399;"
                                    } else {
                                        "font-size:0.8rem;color:rgba(241,245,249,0.5);"
                                    },
                                    if i as i32 == question.answer { "✓ {opt}" } else { "  {opt}" }
                                }
                            }
                        }
                        span { style: "font-size:0.75rem;color:rgba(241,245,249,0.35);padding:0 0.5rem;", "choice" }
                    },
                }
            }

            // Delete button
            button {
                onclick: move |_| { ondelete.call(()) },
                Icon { icon: FaTrash }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// New question form
// ─────────────────────────────────────────────────────────────────────────────

#[component]
pub fn NewQuestion(on_add: EventHandler<Question>) -> Element {
    let mut question_text = use_signal(String::new);
    let mut is_choice = use_signal(|| false);
    let mut numeric_answer = use_signal(|| 0i32);
    let mut choices = use_signal(|| vec!["Option A".to_string(), "Option B".to_string()]);
    let mut correct_choice = use_signal(|| 0usize);

    let submit = move |_| {
        let q = if is_choice() {
            Question {
                question: question_text(),
                answer: correct_choice() as i32,
                kind: QuestionKind::Choice { options: choices() },
            }
        } else {
            Question {
                question: question_text(),
                answer: numeric_answer(),
                kind: QuestionKind::Numeric,
            }
        };
        on_add.call(q);
        // Reset form
        question_text.set(String::new());
        numeric_answer.set(0);
        is_choice.set(false);
        choices.set(vec!["Option A".to_string(), "Option B".to_string()]);
        correct_choice.set(0);
    };

    rsx! {
        div { style: "padding:1rem 0;border-top:1px solid rgba(255,255,255,0.08);margin-top:0.5rem;",

            // Question text input
            input {
                style: "width:100%;margin-bottom:0.75rem;",
                r#type: "text",
                placeholder: "Question…",
                value: "{question_text}",
                oninput: move |e| question_text.set(e.value()),
            }

            // Type toggle
            div { style: "display:flex;gap:0.5rem;margin-bottom:0.75rem;",
                button {
                    style: if !is_choice() {
                        "padding:0.4rem 1rem;border-radius:8px;background:rgba(59,130,246,0.3);border:1px solid #3b82f6;color:white;cursor:pointer;font-size:0.85rem;"
                    } else {
                        "padding:0.4rem 1rem;border-radius:8px;background:rgba(255,255,255,0.06);border:1px solid rgba(255,255,255,0.12);color:rgba(255,255,255,0.6);cursor:pointer;font-size:0.85rem;"
                    },
                    onclick: move |_| is_choice.set(false),
                    "123  Numeric"
                }
                button {
                    style: if is_choice() {
                        "padding:0.4rem 1rem;border-radius:8px;background:rgba(139,92,246,0.3);border:1px solid #8b5cf6;color:white;cursor:pointer;font-size:0.85rem;"
                    } else {
                        "padding:0.4rem 1rem;border-radius:8px;background:rgba(255,255,255,0.06);border:1px solid rgba(255,255,255,0.12);color:rgba(255,255,255,0.6);cursor:pointer;font-size:0.85rem;"
                    },
                    onclick: move |_| is_choice.set(true),
                    "☰  Multiple Choice"
                }
            }

            // Kind-specific inputs
            if !is_choice() {
                // ── Numeric ──────────────────────────────────────────────────
                div { style: "display:flex;gap:0.75rem;align-items:center;",
                    input {
                        style: "flex:1;",
                        r#type: "number",
                        placeholder: "Correct answer",
                        value: "{numeric_answer}",
                        oninput: move |e| {
                            numeric_answer.set(e.value().parse::<i32>().unwrap_or(0));
                        },
                    }
                    button {
                        class: "btn-add",
                        onclick: submit,
                        Icon { icon: FaPlus }
                        " Add"
                    }
                }
            } else {
                // ── Multiple choice ───────────────────────────────────────────
                div { style: "display:flex;flex-direction:column;gap:0.4rem;margin-bottom:0.5rem;",
                    for i in 0..choices.read().len() {
                        div { key: "{i}", style: "display:flex;align-items:center;gap:0.5rem;",

                            // Radio — click to mark as correct
                            button {
                                style: if correct_choice() == i {
                                    "width:1.4rem;height:1.4rem;border-radius:50%;border:2px solid #34d399;background:#34d399;flex-shrink:0;cursor:pointer;"
                                } else {
                                    "width:1.4rem;height:1.4rem;border-radius:50%;border:2px solid rgba(255,255,255,0.3);background:transparent;flex-shrink:0;cursor:pointer;"
                                },
                                onclick: move |_| correct_choice.set(i),
                                ""
                            }

                            // Option text
                            input {
                                style: "flex:1;",
                                r#type: "text",
                                placeholder: "Option {i + 1}",
                                value: "{choices.read()[i]}",
                                oninput: move |e| {
                                    choices.write()[i] = e.value();
                                },
                            }

                            // Remove option (only if > 2)
                            if choices.read().len() > 2 {
                                button {
                                    style: "padding:0.3rem 0.5rem;background:rgba(239,68,68,0.15);border:1px solid rgba(239,68,68,0.3);border-radius:6px;color:#f87171;cursor:pointer;",
                                    onclick: move |_| {
                                        let len = choices.read().len();
                                        choices.write().remove(i);
                                        // Fix correct_choice if it pointed at removed or beyond
                                        if correct_choice() >= len - 1 {
                                            correct_choice.set(0);
                                        }
                                    },
                                    "✕"
                                }
                            }
                        }
                    }
                }

                // Add option + submit row
                div { style: "display:flex;gap:0.5rem;",
                    button {
                        style: "flex:1;padding:0.4rem;border-radius:8px;border:1px dashed rgba(255,255,255,0.2);background:transparent;color:rgba(255,255,255,0.5);cursor:pointer;font-size:0.85rem;",
                        onclick: move |_| {
                            let n = choices.read().len() + 1;
                            choices.write().push(format!("Option {n}"));
                        },
                        "＋ Add option"
                    }
                    button {
                        class: "btn-add",
                        onclick: submit,
                        Icon { icon: FaPlus }
                        " Add"
                    }
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Create button
// ─────────────────────────────────────────────────────────────────────────────

#[component]
fn Create(quizz: Signal<Quizz>) -> Element {
    let navigator = navigator();
    rsx! {
        button { class: "btn-submit-create",
            onclick: move |_| {
                async move {
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
            "Create Quiz"
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Kept from original (used by other views)
// ─────────────────────────────────────────────────────────────────────────────

#[component]
fn Divider() -> Element {
    rsx! { div { margin_right: "1em" } }
}
