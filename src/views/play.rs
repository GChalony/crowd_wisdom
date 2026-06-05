use dioxus::{fullstack::WebSocketOptions, logger::tracing, prelude::*};

use crate::{
    backend::{
        admin_finish, admin_next, admin_reveal, check_is_admin, get_play_state, submit_answer,
        CrowdResult, IndividualAnswer, PlayPhase, PlayState, QuestionKind,
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
                    PlayPhase::Finished => rsx! {},
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
                    PlayPhase::Finished => rsx! {},
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared styles
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
    margin-top:1rem; width:100%; padding:0.9rem; border:none;
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

                // Preview choices for the admin
                if let QuestionKind::Choice { options } = &state.kind {
                    div { style: "display:flex;flex-direction:column;gap:0.4rem;margin-bottom:1.5rem;text-align:left;",
                        for (i, opt) in options.iter().enumerate() {
                            div {
                                key: "{i}",
                                style: "padding:0.5rem 0.75rem;border-radius:10px;border:1px solid rgba(255,255,255,0.1);background:rgba(255,255,255,0.04);font-size:0.9rem;",
                                "{opt}"
                            }
                        }
                    }
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
        ref crowd_result,
    } = state.phase
    else {
        return rsx! {};
    };
    let is_last = state.position + 1 >= state.total;

    rsx! {
        div { style: CARD_OUTER,
            div { style: CARD_INNER,
                p { style: "font-size:0.85rem;color:rgba(255,255,255,0.5);margin-bottom:0.25rem;",
                    "Question {state.position + 1} / {state.total}"
                }
                h2 { style: "font-size:1.4rem;font-weight:700;margin-bottom:1.25rem;",
                    "{state.question}"
                }

                // ── Stats grid ──────────────────────────────────────────────
                {reveal_stats_grid(correct_answer, &state.kind, crowd_result)}

                // ── Individual answers ───────────────────────────────────────
                {answers_panel(answers, correct_answer, "", &state.kind)}

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
                    match state.kind.clone() {
                        QuestionKind::Numeric => rsx! {
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
                                    if let Ok(n) = e.value().parse::<i32>() { pending.set(n); }
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
                        },
                        QuestionKind::Choice { options } => rsx! {
                            div { style: "display:flex;flex-direction:column;gap:0.6rem;margin-bottom:1rem;",
                                for (i, opt) in options.iter().enumerate() {
                                    button {
                                        key: "{i}",
                                        style: if pending() == i as i32 {
                                            "padding:0.8rem 1rem;border-radius:12px;border:2px solid #3b82f6;background:rgba(59,130,246,0.2);color:white;font-size:1rem;cursor:pointer;text-align:left;width:100%;"
                                        } else {
                                            "padding:0.8rem 1rem;border-radius:12px;border:1px solid rgba(255,255,255,0.15);background:rgba(255,255,255,0.06);color:rgba(255,255,255,0.85);font-size:1rem;cursor:pointer;text-align:left;width:100%;"
                                        },
                                        onclick: move |_| pending.set(i as i32),
                                        "{opt}"
                                    }
                                }
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
                                "Confirm"
                            }
                        },
                    }
                } else {
                    // ── Submitted / waiting ─────────────────────────────────
                    div { style: "font-size:3rem;margin-bottom:0.5rem;", "✅" }
                    p { style: "font-weight:700;font-size:1.1rem;margin-bottom:0.25rem;",
                        "Answer submitted!"
                    }
                    // Show what was submitted
                    div { style: "
                            font-size:1.5rem; font-weight:800; margin-bottom:1rem;
                            background:linear-gradient(135deg,#60a5fa,#a78bfa);
                            -webkit-background-clip:text; -webkit-text-fill-color:transparent;
                        ",
                        match state.kind.clone() {
                            QuestionKind::Numeric => rsx! { "{my_answer().unwrap_or(0)}" },
                            QuestionKind::Choice { options } => {
                                let idx = my_answer().unwrap_or(0) as usize;
                                let label = options.get(idx).map(|s| s.as_str()).unwrap_or("?");
                                rsx! { "{label}" }
                            }
                        }
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
        ref crowd_result,
    } = state.phase
    else {
        return rsx! {};
    };

    let (icon, verdict) = match my_answer {
        Some(a) if a == correct_answer => ("✅", "Correct!"),
        Some(_) => ("❌", "Wrong!"),
        None => ("😶", "No answer submitted"),
    };

    // For display: what did the current player answer?
    let my_answer_label = match (my_answer, &state.kind) {
        (None, _) => "—".to_string(),
        (Some(a), QuestionKind::Numeric) => a.to_string(),
        (Some(a), QuestionKind::Choice { options }) => options
            .get(a as usize)
            .cloned()
            .unwrap_or_else(|| a.to_string()),
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

                div { style: "font-size:2.5rem;margin-bottom:0.2rem;", "{icon}" }
                p { style: "font-size:1rem;font-weight:700;margin-bottom:1rem;", "{verdict}" }

                // Your answer vs correct, plus crowd result
                div { class: "reveal-answers-grid",
                    div { class: "reveal-col",
                        p { class: "reveal-label", "Your answer" }
                        div { class: "reveal-value",
                            style: "font-size:1.3rem;",
                            "{my_answer_label}"
                        }
                    }
                    div { class: "reveal-divider" }
                    {correct_col(&state.kind, correct_answer)}
                    div { class: "reveal-divider" }
                    {crowd_col(crowd_result, &state.kind)}
                }

                {answers_panel(answers, correct_answer, &my_name, &state.kind)}

                p { style: "color:rgba(255,255,255,0.35);font-size:0.82rem;margin-top:0.75rem;",
                    "Waiting for the admin to continue…"
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared render helpers
// ─────────────────────────────────────────────────────────────────────────────

/// The "Correct answer" column, rendered differently per question kind.
fn correct_col(kind: &QuestionKind, correct_answer: i32) -> Element {
    let label = match kind {
        QuestionKind::Numeric => correct_answer.to_string(),
        QuestionKind::Choice { options } => options
            .get(correct_answer as usize)
            .cloned()
            .unwrap_or_else(|| correct_answer.to_string()),
    };
    rsx! {
        div { class: "reveal-col",
            p { class: "reveal-label", "✓ Correct" }
            div { class: "reveal-value correct", style: "font-size:1.3rem;", "{label}" }
        }
    }
}

/// The "Crowd answer" column, rendered differently per question kind.
fn crowd_col(crowd_result: &CrowdResult, kind: &QuestionKind) -> Element {
    match crowd_result {
        CrowdResult::Mean { average } => {
            let avg_str = format!("{average:.1}");
            rsx! {
                div { class: "reveal-col",
                    p { class: "reveal-label", "👥 Crowd avg" }
                    div { class: "reveal-value crowd", style: "font-size:1.3rem;", "{avg_str}" }
                }
            }
        }
        CrowdResult::Votes { counts, winners } => {
            let total_votes: usize = counts.iter().sum();
            let winner_labels: Vec<String> = match kind {
                QuestionKind::Choice { options } => winners
                    .iter()
                    .filter_map(|&i| options.get(i).cloned())
                    .collect(),
                _ => winners.iter().map(|i| i.to_string()).collect(),
            };
            let crowd_pick = if winner_labels.is_empty() {
                "—".to_string()
            } else {
                winner_labels.join(" / ")
            };
            let tie_note = if winners.len() > 1 { " (tie!)" } else { "" };
            rsx! {
                div { class: "reveal-col",
                    p { class: "reveal-label", "👥 Crowd pick" }
                    div { class: "reveal-value crowd", style: "font-size:1.1rem;word-break:break-word;",
                        "{crowd_pick}{tie_note}"
                    }
                    p { class: "reveal-sublabel", "{total_votes} votes" }
                }
            }
        }
    }
}

/// Stats grid shown in both admin and player revealed views.
fn reveal_stats_grid(
    correct_answer: i32,
    kind: &QuestionKind,
    crowd_result: &CrowdResult,
) -> Element {
    rsx! {
        div { class: "reveal-answers-grid",
            {correct_col(kind, correct_answer)}
            div { class: "reveal-divider" }
            {crowd_col_with_error(crowd_result, kind, correct_answer)}
        }
    }
}

/// Crowd column variant that includes % error for numeric questions (admin view).
fn crowd_col_with_error(
    crowd_result: &CrowdResult,
    kind: &QuestionKind,
    correct_answer: i32,
) -> Element {
    match crowd_result {
        CrowdResult::Mean { average } => {
            let avg_str = format!("{average:.1}");
            let crowd_err = crowd_error_pct(*average, correct_answer);
            let crowd_err_str = format!("{crowd_err:.1}%");
            rsx! {
                div { class: "reveal-col",
                    p { class: "reveal-label", "👥 Crowd avg" }
                    div { class: "reveal-value crowd", style: "font-size:1.3rem;", "{avg_str}" }
                    p { class: "reveal-sublabel",
                        if crowd_err < 1.0 { "🎯 {crowd_err_str} off!" } else { "{crowd_err_str} off" }
                    }
                }
            }
        }
        other => crowd_col(other, kind),
    }
}

/// Answer list panel — numeric shows delta, choice shows option text + correct/wrong indicator.
fn answers_panel(
    answers: &[IndividualAnswer],
    correct_answer: i32,
    my_name: &str,
    kind: &QuestionKind,
) -> Element {
    if answers.is_empty() {
        return rsx! {
            p { style: "color:rgba(255,255,255,0.4);font-size:0.85rem;margin-top:0.5rem;",
                "No answers were submitted."
            }
        };
    }

    rsx! {
        div { class: "answers-panel",
            p { class: "answers-heading",
                "{answers.len()} answer(s)"
            }
            div { class: "answers-list",
                {answers.iter().enumerate().map(|(i, a)| {
                    let is_me = a.name == my_name && !my_name.is_empty();
                    let is_closest = i == 0 && matches!(kind, QuestionKind::Numeric);

                    let value_label = match kind {
                        QuestionKind::Numeric => a.value.to_string(),
                        QuestionKind::Choice { options } => options
                            .get(a.value as usize)
                            .cloned()
                            .unwrap_or_else(|| a.value.to_string()),
                    };
                    let delta_label = match kind {
                        QuestionKind::Numeric => {
                            let d = a.value - correct_answer;
                            if d >= 0 { format!("+{d}") } else { format!("{d}") }
                        }
                        QuestionKind::Choice { .. } => {
                            if a.value == correct_answer { "✓".to_string() } else { "✗".to_string() }
                        }
                    };

                    rsx! {
                        div {
                            key: "{i}",
                            class: if is_me { "answer-row me" } else { "answer-row" },
                            span { class: "answer-name",
                                if is_closest && !is_me { "🏆 {a.name}" }
                                else if is_me { "⭐ {a.name}" }
                                else { "{a.name}" }
                            }
                            span { class: "answer-value-cell", "{value_label}" }
                            span {
                                class: if is_closest || (matches!(kind, QuestionKind::Choice { .. }) && a.value == correct_answer) {
                                    "answer-delta best"
                                } else {
                                    "answer-delta"
                                },
                                "{delta_label}"
                            }
                        }
                    }
                })}
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn crowd_error_pct(average: f64, correct_answer: i32) -> f64 {
    if correct_answer == 0 {
        return 0.0;
    }
    ((average - correct_answer as f64) / correct_answer as f64 * 100.0).abs()
}
