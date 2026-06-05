use dioxus::{fullstack::WebSocketOptions, logger::tracing, prelude::*};

use crate::backend::{
    admin_next, admin_reveal, check_is_admin, get_play_state, submit_answer, PlayPhase, PlayState,
};

// ─────────────────────────────────────────────────────────────────────────────
// Top-level route component
// ─────────────────────────────────────────────────────────────────────────────

#[component]
pub fn Play(quizz_id: u32) -> Element {
    let mut play_state: Signal<Option<PlayState>> = use_signal(|| None);
    // The answer the current user submitted for the active question (reset on next).
    let mut my_answer: Signal<Option<i32>> = use_signal(|| None);
    let mut is_admin: Signal<bool> = use_signal(|| false);
    let mut user_id: Signal<String> = use_signal(String::new);

    use_effect(move || {
        spawn(async move {
            // Read the persistent user UUID written by the lobby page.
            let mut eval =
                document::eval("dioxus.send(localStorage.getItem('crowd_wisdom_user_id') || '');");
            let uid = eval.recv::<String>().await.unwrap_or_default();

            let admin = check_is_admin(quizz_id, uid.clone()).await.unwrap_or(false);
            is_admin.set(admin);
            user_id.set(uid);

            match get_play_state(quizz_id, WebSocketOptions::new()).await {
                Ok(ws) => {
                    while let Ok(state) = ws.recv().await {
                        // Clear submitted answer whenever the question position changes.
                        // Use play_state() (clones the value) to avoid holding a Ref across awaits.
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
        // Game hasn't sent its first state yet.
        None => rsx! {
            div { style: CARD_OUTER,
                div { style: CARD_INNER,
                    div { style: "font-size:3rem;margin-bottom:1rem", "⏳" }
                    h2 { style: "font-size:1.4rem;font-weight:700;", "Waiting for the quiz to start…" }
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
                }
            } else {
                match state.phase.clone() {
                    PlayPhase::Answering => rsx! {
                        // key forces hook reset when a new question arrives
                        PlayerAnswering { key: "{state.position}", state, quizz_id, my_answer }
                    },
                    PlayPhase::Revealed { .. } => rsx! {
                        PlayerRevealed { state, my_answer: my_answer() }
                    },
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared style constants
// ─────────────────────────────────────────────────────────────────────────────

const CARD_OUTER: &str = "
    display:flex; justify-content:center; align-items:center;
    min-height:100vh;
    background:linear-gradient(135deg,#1e293b,#0f172a);
    font-family:Inter,sans-serif; padding:2rem;
";
const CARD_INNER: &str = "
    background:rgba(255,255,255,0.08); backdrop-filter:blur(12px);
    border:1px solid rgba(255,255,255,0.12); border-radius:24px;
    padding:2rem; width:100%; max-width:440px;
    box-shadow:0 10px 30px rgba(0,0,0,0.35); color:white; text-align:center;
";
const BTN_PRIMARY: &str = "
    margin-top:1.5rem; width:100%; padding:0.9rem; border:none;
    border-radius:14px; background:linear-gradient(135deg,#3b82f6,#8b5cf6);
    color:white; font-size:1rem; font-weight:600; cursor:pointer;
";
const BTN_ORANGE: &str = "
    margin-top:1.5rem; width:100%; padding:0.9rem; border:none;
    border-radius:14px; background:linear-gradient(135deg,#f97316,#ef4444);
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

                // Live count badge
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
    let PlayPhase::Revealed { correct_answer } = state.phase else {
        return rsx! {};
    };
    let is_last = state.position + 1 >= state.total;

    rsx! {
        div { style: CARD_OUTER,
            div { style: CARD_INNER,
                p { style: "font-size:0.85rem;color:rgba(255,255,255,0.5);margin-bottom:0.25rem;",
                    "Question {state.position + 1} / {state.total}"
                }
                h2 { style: "font-size:1.5rem;font-weight:700;margin-bottom:1rem;",
                    "{state.question}"
                }

                p { style: "color:rgba(255,255,255,0.6);margin-bottom:0.25rem;",
                    "Correct answer:"
                }
                div { style: "
                        font-size:3rem; font-weight:800; margin-bottom:1rem;
                        background:linear-gradient(135deg,#34d399,#059669);
                        -webkit-background-clip:text; -webkit-text-fill-color:transparent;
                    ",
                    "{correct_answer}"
                }

                div { style: "
                        display:inline-flex; align-items:center; gap:0.5rem;
                        padding:0.5rem 1rem; border-radius:999px;
                        background:rgba(255,255,255,0.08);
                        color:rgba(255,255,255,0.7); font-size:0.9rem; margin-bottom:1.5rem;
                    ",
                    "📊 {state.answer_count} answer(s) received"
                }

                if is_last {
                    p { style: "color:#34d399;font-weight:700;font-size:1.1rem;",
                        "🎉 Quiz complete!"
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
fn PlayerAnswering(state: PlayState, quizz_id: u32, mut my_answer: Signal<Option<i32>>) -> Element {
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
                    // ── Input form ──────────────────────────────────────────
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
                            async move {
                                submit_answer(quizz_id, pos, answer).await.ok();
                                my_answer.set(Some(answer));
                            }
                        },
                        "Submit"
                    }
                } else {
                    // ── Waiting state ───────────────────────────────────────
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
fn PlayerRevealed(state: PlayState, my_answer: Option<i32>) -> Element {
    let PlayPhase::Revealed { correct_answer } = state.phase else {
        return rsx! {};
    };

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

                // Verdict
                div { style: "font-size:3rem;margin-bottom:0.25rem;", "{icon}" }
                p { style: "font-size:1.1rem;font-weight:700;margin-bottom:1rem;", "{verdict}" }

                // Their answer vs correct
                div { style: "
                        display:flex; gap:1.5rem; justify-content:center;
                        margin-bottom:1.5rem;
                    ",
                    div {
                        p { style: "color:rgba(255,255,255,0.5);font-size:0.85rem;", "Your answer" }
                        div { style: "font-size:2rem;font-weight:800;",
                            match my_answer {
                                Some(a) => rsx! { "{a}" },
                                None => rsx! { span { style: "color:rgba(255,255,255,0.3);", "—" } },
                            }
                        }
                    }
                    div { style: "width:1px;background:rgba(255,255,255,0.15);" }
                    div {
                        p { style: "color:rgba(255,255,255,0.5);font-size:0.85rem;", "Correct" }
                        div { style: "
                                font-size:2rem; font-weight:800;
                                background:linear-gradient(135deg,#34d399,#059669);
                                -webkit-background-clip:text; -webkit-text-fill-color:transparent;
                            ",
                            "{correct_answer}"
                        }
                    }
                }

                p { style: "color:rgba(255,255,255,0.4);font-size:0.85rem;",
                    "Waiting for the admin to continue…"
                }
            }
        }
    }
}
