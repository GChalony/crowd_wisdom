use dioxus::{fullstack::WebSocketOptions, logger::tracing, prelude::*};

use crate::{
    backend::{
        admin_finish, admin_next, admin_reveal, check_is_admin, get_play_state, submit_answer,
        IndividualAnswer, PlayPhase, PlayState,
    },
    Route,
};

// ─────────────────────────────────────────────────────────────────────────────
// Top-level route component
// ─────────────────────────────────────────────────────────────────────────────

#[component]
pub fn Play(quizz_id: u32) -> Element {
    let mut play_state: Signal<Option<PlayState>> = use_signal(|| None);
    let mut my_answer: Signal<Option<i32>> = use_signal(|| None);
    let mut is_admin: Signal<bool> = use_signal(|| false);
    let mut user_id: Signal<String> = use_signal(String::new);
    let mut user_name: Signal<String> = use_signal(String::new);
    let navigator = use_navigator();

    use_effect(move || {
        spawn(async move {
            // Read both the UUID and the display name written by the lobby page.
            let mut eval = document::eval(
                r#"dioxus.send({
                    id:   localStorage.getItem('crowd_wisdom_user_id')   || '',
                    name: localStorage.getItem('crowd_wisdom_user_name') || 'Player'
                });"#,
            );
            let val = eval.recv::<serde_json::Value>().await.unwrap_or_default();
            let uid = val["id"].as_str().unwrap_or("").to_string();
            let name = val["name"].as_str().unwrap_or("Player").to_string();

            let admin = check_is_admin(quizz_id, uid.clone()).await.unwrap_or(false);
            is_admin.set(admin);
            user_id.set(uid);
            user_name.set(name);

            match get_play_state(quizz_id, WebSocketOptions::new()).await {
                Ok(ws) => {
                    while let Ok(state) = ws.recv().await {
                        // Finished is a signal to navigate home — don't update state.
                        if matches!(state.phase, PlayPhase::Finished) {
                            navigator.push(Route::Home {});
                            break;
                        }
                        let prev_pos = play_state().map(|s| s.position);
                        if prev_pos != Some(state.position) {
                            my_answer.set(None);
                        }
                        play_state.set(Some(state));
                    }
                }
                Err(e) => tracing::error!("Play WebSocket error: {e:?}"),
            }
        });
    });

    match play_state() {
        None => rsx! {
            div { style: CARD_OUTER,
                div { style: CARD_INNER,
                    div { style: "font-size:3rem;margin-bottom:1rem", "⏳" }
                    h2 { style: "font-size:1.4rem;font-weight:700;",
                        "Waiting for the quiz to start…"
                    }
                }
            }
        },
        Some(state) => {
            if is_admin() {
                match state.phase.clone() {
                    PlayPhase::Answering => rsx! {
                        AdminAnswering { state, quizz_id, user_id: user_id() }
                    },
                    PlayPhase::Revealed { .. } => rsx! {
                        AdminRevealed { state, quizz_id, user_id: user_id() }
                    },
                    PlayPhase::Finished => rsx! {}, // navigation already triggered in WS loop
                }
            } else {
                match state.phase.clone() {
                    PlayPhase::Answering => rsx! {
                        PlayerAnswering {
                            key: "{state.position}",
                            state,
                            quizz_id,
                            my_answer,
                            user_name: user_name(),
                        }
                    },
                    PlayPhase::Revealed { .. } => rsx! {
                        PlayerRevealed {
                            state,
                            my_answer: my_answer(),
                            my_name: user_name(),
                        }
                    },
                    PlayPhase::Finished => rsx! {}, // navigation already triggered in WS loop
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared style constants
// ─────────────────────────────────────────────────────────────────────────────

const CARD_OUTER: &str = "
    display:flex; justify-content:center; align-items:flex-start;
    min-height:100vh;
    background:linear-gradient(135deg,#1e293b,#0f172a);
    font-family:Inter,sans-serif; padding:2rem;
";
const CARD_INNER: &str = "
    background:rgba(255,255,255,0.08); backdrop-filter:blur(12px);
    border:1px solid rgba(255,255,255,0.12); border-radius:24px;
    padding:2rem; width:100%; max-width:520px;
    box-shadow:0 10px 30px rgba(0,0,0,0.35); color:white; text-align:center;
";
const BTN_PRIMARY: &str = "
    margin-top:1.5rem; width:100%; padding:0.9rem; border:none;
    border-radius:14px; background:linear-gradient(135deg,#3b82f6,#8b5cf6);
    color:white; font-size:1rem; font-weight:600; cursor:pointer;
";
const BTN_ORANGE: &str = "
    margin-top:1rem; width:100%; padding:0.9rem; border:none;
    border-radius:14px; background:linear-gradient(135deg,#f97316,#ef4444);
    color:white; font-size:1rem; font-weight:600; cursor:pointer;
";
const BTN_GREEN: &str = "
    margin-top:1rem; width:100%; padding:0.9rem; border:none;
    border-radius:14px; background:linear-gradient(135deg,#34d399,#059669);
    color:white; font-size:1rem; font-weight:600; cursor:pointer;
";

// ─────────────────────────────────────────────────────────────────────────────
// Admin: answering phase
// ─────────────────────────────────────────────────────────────────────────────

#[component]
fn AdminAnswering(state: PlayState, quizz_id: u32, user_id: String) -> Element {
    rsx! {
        div { style: CARD_OUTER,
            div { style: CARD_INNER,
                p { style: "font-size:0.85rem;color:rgba(255,255,255,0.5);margin-bottom:0.25rem;",
                    "Question {state.position + 1} / {state.total}"
                }
                h2 { style: "font-size:1.5rem;font-weight:700;margin-bottom:1.5rem;",
                    "{state.question}"
                }
                div { style: "
                        display:inline-flex; align-items:center; gap:0.5rem;
                        padding:0.6rem 1.2rem; border-radius:999px;
                        background:rgba(255,255,255,0.08);
                        color:rgba(255,255,255,0.8); font-size:0.95rem; margin-bottom:1.5rem;
                    ",
                    span { "⏳" }
                    span { "{state.answer_count} answer(s) received" }
                }
                button {
                    style: BTN_PRIMARY,
                    onclick: move |_| {
                        let uid = user_id.clone();
                        async move { admin_reveal(quizz_id, uid).await.ok(); }
                    },
                    "🔍 Reveal Answers"
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Admin: revealed phase
// ─────────────────────────────────────────────────────────────────────────────

#[component]
fn AdminRevealed(state: PlayState, quizz_id: u32, user_id: String) -> Element {
    let PlayPhase::Revealed {
        correct_answer,
        ref answers,
        average,
    } = state.phase
    else {
        return rsx! {};
    };
    let is_last = state.position + 1 >= state.total;
    let avg_str = format!("{average:.1}");
    let crowd_err = crowd_error_pct(average, correct_answer);
    let crowd_err_str = format!("{crowd_err:.1}%");

    rsx! {
        div { style: CARD_OUTER,
            div { style: CARD_INNER,
                p { style: "font-size:0.85rem;color:rgba(255,255,255,0.5);margin-bottom:0.25rem;",
                    "Question {state.position + 1} / {state.total}"
                }
                h2 { style: "font-size:1.4rem;font-weight:700;margin-bottom:1.25rem;",
                    "{state.question}"
                }

                // ── Correct answer ─────────────────────────────────────────
                div { class: "reveal-answers-grid",
                    div { class: "reveal-col",
                        p { class: "reveal-label", "✓ Correct" }
                        div { class: "reveal-value correct", "{correct_answer}" }
                    }
                    div { class: "reveal-divider" }
                    div { class: "reveal-col",
                        p { class: "reveal-label", "👥 Crowd avg" }
                        div { class: "reveal-value crowd", "{avg_str}" }
                        p { class: "reveal-sublabel",
                            if crowd_err < 1.0 { "🎯 {crowd_err_str} off!" } else { "{crowd_err_str} off" }
                        }
                    }
                }

                // ── Individual answers ─────────────────────────────────────
                if !answers.is_empty() {
                    div { class: "answers-panel",
                        p { class: "answers-heading",
                            "Individual answers — {answers.len()} submitted"
                        }
                        div { class: "answers-list",
                            {answers.iter().enumerate().map(|(i, a)| {
                                let delta = a.value - correct_answer;
                                let delta_str = if delta >= 0 {
                                    format!("+{delta}")
                                } else {
                                    format!("{delta}")
                                };
                                let is_closest = i == 0;
                                rsx! {
                                    div { class: "answer-row", key: "{i}",
                                        span { class: "answer-name",
                                            if is_closest { "🏆 {a.name}" } else { "{a.name}" }
                                        }
                                        span { class: "answer-value-cell", "{a.value}" }
                                        span {
                                            class: if is_closest { "answer-delta best" } else { "answer-delta" },
                                            "({delta_str})"
                                        }
                                    }
                                }
                            })}
                        }
                    }
                } else {
                    p { style: "color:rgba(255,255,255,0.4);font-size:0.85rem;margin-top:0.5rem;",
                        "No answers were submitted."
                    }
                }

                if is_last {
                    button {
                        style: BTN_GREEN,
                        onclick: move |_| {
                            let uid = user_id.clone();
                            async move { admin_finish(quizz_id, uid).await.ok(); }
                        },
                        "🏁 End Quiz — Go Home"
                    }
                } else {
                    button {
                        style: BTN_ORANGE,
                        onclick: move |_| {
                            let uid = user_id.clone();
                            async move { admin_next(quizz_id, uid).await.ok(); }
                        },
                        "Next Question →"
                    }
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Player: answering phase
// ─────────────────────────────────────────────────────────────────────────────

#[component]
fn PlayerAnswering(
    state: PlayState,
    quizz_id: u32,
    mut my_answer: Signal<Option<i32>>,
    user_name: String,
) -> Element {
    let mut pending = use_signal(|| 0i32);
    let submitted = my_answer.read().is_some();

    rsx! {
        div { style: CARD_OUTER,
            div { style: CARD_INNER,
                p { style: "font-size:0.85rem;color:rgba(255,255,255,0.5);margin-bottom:0.25rem;",
                    "Question {state.position + 1} / {state.total}"
                }
                h2 { style: "font-size:1.5rem;font-weight:700;margin-bottom:1.5rem;",
                    "{state.question}"
                }

                if !submitted {
                    input {
                        style: "
                            width:100%; padding:0.9rem 1rem; border-radius:14px;
                            border:none; outline:none; font-size:1rem;
                            background:rgba(255,255,255,0.12); color:white;
                            box-sizing:border-box;
                        ",
                        r#type: "number",
                        placeholder: "Your answer…",
                        value: "{pending}",
                        oninput: move |e| {
                            if let Ok(n) = e.value().parse::<i32>() {
                                pending.set(n);
                            }
                        },
                    }
                    button {
                        style: BTN_PRIMARY,
                        onclick: move |_| {
                            let answer = pending();
                            let pos = state.position;
                            let name = user_name.clone();
                            async move {
                                submit_answer(quizz_id, pos, answer, name).await.ok();
                                my_answer.set(Some(answer));
                            }
                        },
                        "Submit"
                    }
                } else {
                    div { style: "font-size:3rem;margin-bottom:0.5rem;", "✅" }
                    p { style: "font-weight:700;font-size:1.1rem;margin-bottom:0.25rem;",
                        "Answer submitted!"
                    }
                    div { style: "
                            font-size:2.5rem; font-weight:800; margin-bottom:1rem;
                            background:linear-gradient(135deg,#60a5fa,#a78bfa);
                            -webkit-background-clip:text; -webkit-text-fill-color:transparent;
                        ",
                        "{my_answer().unwrap_or(0)}"
                    }
                    div { style: "
                            display:inline-flex; align-items:center; gap:0.5rem;
                            padding:0.75rem 1rem; border-radius:999px;
                            background:rgba(255,255,255,0.08);
                            color:rgba(255,255,255,0.8); font-size:0.95rem;
                        ",
                        span { "⏳" }
                        span { "Waiting for others… ({state.answer_count} answered)" }
                    }
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Player: revealed phase
// ─────────────────────────────────────────────────────────────────────────────

#[component]
fn PlayerRevealed(state: PlayState, my_answer: Option<i32>, my_name: String) -> Element {
    let PlayPhase::Revealed {
        correct_answer,
        ref answers,
        average,
    } = state.phase
    else {
        return rsx! {};
    };
    let avg_str = format!("{average:.1}");
    let crowd_err = crowd_error_pct(average, correct_answer);
    let crowd_err_str = format!("{crowd_err:.1}%");

    let (icon, verdict) = match my_answer {
        Some(a) if a == correct_answer => ("✅", "Correct!"),
        Some(_) => ("❌", "Wrong!"),
        None => ("😶", "No answer submitted"),
    };

    rsx! {
        div { style: CARD_OUTER,
            div { style: CARD_INNER,
                p { style: "font-size:0.85rem;color:rgba(255,255,255,0.5);margin-bottom:0.25rem;",
                    "Question {state.position + 1} / {state.total}"
                }
                h2 { style: "font-size:1.3rem;font-weight:700;margin-bottom:1rem;",
                    "{state.question}"
                }

                // ── Verdict + comparison ───────────────────────────────────
                div { style: "font-size:2.5rem;margin-bottom:0.2rem;", "{icon}" }
                p { style: "font-size:1rem;font-weight:700;margin-bottom:1rem;", "{verdict}" }

                div { class: "reveal-answers-grid",
                    div { class: "reveal-col",
                        p { class: "reveal-label", "Your answer" }
                        div { class: "reveal-value",
                            match my_answer {
                                Some(a) => rsx! { "{a}" },
                                None => rsx! { span { style: "color:rgba(255,255,255,0.3);", "—" } },
                            }
                        }
                    }
                    div { class: "reveal-divider" }
                    div { class: "reveal-col",
                        p { class: "reveal-label", "✓ Correct" }
                        div { class: "reveal-value correct", "{correct_answer}" }
                    }
                    div { class: "reveal-divider" }
                    div { class: "reveal-col",
                        p { class: "reveal-label", "👥 Crowd avg" }
                        div { class: "reveal-value crowd", "{avg_str}" }
                        p { class: "reveal-sublabel",
                            if crowd_err < 1.0 { "🎯 {crowd_err_str} off!" } else { "{crowd_err_str} off" }
                        }
                    }
                }

                // ── Individual answers ─────────────────────────────────────
                if !answers.is_empty() {
                    div { class: "answers-panel",
                        p { class: "answers-heading",
                            "All answers ({answers.len()})"
                        }
                        div { class: "answers-list",
                            {answers.iter().enumerate().map(|(i, a)| {
                                let is_me = a.name == my_name;
                                let is_closest = i == 0;
                                let delta = a.value - correct_answer;
                                let delta_str = if delta >= 0 {
                                    format!("+{delta}")
                                } else {
                                    format!("{delta}")
                                };
                                rsx! {
                                    div {
                                        class: if is_me { "answer-row me" } else { "answer-row" },
                                        key: "{i}",
                                        span { class: "answer-name",
                                            if is_closest && !is_me { "🏆 {a.name}" }
                                            else if is_me { "⭐ {a.name}" }
                                            else { "{a.name}" }
                                        }
                                        span { class: "answer-value-cell", "{a.value}" }
                                        span {
                                            class: if is_closest { "answer-delta best" } else { "answer-delta" },
                                            "({delta_str})"
                                        }
                                    }
                                }
                            })}
                        }
                    }
                }

                p { style: "color:rgba(255,255,255,0.35);font-size:0.82rem;margin-top:0.75rem;",
                    "Waiting for the admin to continue…"
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Percentage error of the crowd average relative to the correct answer.
fn crowd_error_pct(average: f64, correct_answer: i32) -> f64 {
    if correct_answer == 0 {
        return 0.0;
    }
    ((average - correct_answer as f64) / correct_answer as f64 * 100.0).abs()
}
