use dioxus::{fullstack::WebSocketOptions, logger::tracing, prelude::*};

use crate::{
    backend::{admin_start_game, get_lobby_state, LobbyUpdate, User},
    Route,
};

const ADJECTIVES: [&str; 10] = [
    "Swift", "Brave", "Clever", "Wise", "Bold", "Quick", "Bright", "Calm", "Daring", "Eager",
];
const ANIMALS: [&str; 12] = [
    "Fox", "Bear", "Eagle", "Wolf", "Lion", "Tiger", "Hawk", "Deer", "Crane", "Lynx", "Panda",
    "Otter",
];

#[component]
pub fn Lobby(quizz_id: u32) -> Element {
    let mut players: Signal<Vec<User>> = use_signal(Vec::new);
    let mut my_name: Signal<String> = use_signal(String::new);
    // Stored so the Start button can pass it to admin_start_game.
    let mut my_user_id: Signal<String> = use_signal(String::new);

    let is_me_admin = use_memo(move || {
        let name = my_name.read();
        !name.is_empty() && players.read().iter().any(|p| p.is_admin && p.name == *name)
    });

    let navigator = use_navigator();

    use_effect(move || {
        spawn(async move {
            // ── Step 1: identity ──────────────────────────────────────────────
            let script = format!(
                r#"
                let id = localStorage.getItem('crowd_wisdom_user_id');
                if (!id) {{
                    id = crypto.randomUUID();
                    localStorage.setItem('crowd_wisdom_user_id', id);
                    localStorage.removeItem('crowd_wisdom_user_name');
                }}
                let name = localStorage.getItem('crowd_wisdom_user_name');
                if (!name) {{
                    const hex  = id.replace(/-/g, '');
                    const adj  = [{adj}];
                    const ani  = [{ani}];
                    const h1   = parseInt(hex.slice(0, 8),  16);
                    const h2   = parseInt(hex.slice(8, 16), 16);
                    name = adj[h1 % adj.length] + ani[h2 % ani.length];
                    localStorage.setItem('crowd_wisdom_user_name', name);
                }}
                dioxus.send({{ id, name }});
                "#,
                adj = ADJECTIVES
                    .iter()
                    .map(|s| format!("'{s}'"))
                    .collect::<Vec<_>>()
                    .join(","),
                ani = ANIMALS
                    .iter()
                    .map(|s| format!("'{s}'"))
                    .collect::<Vec<_>>()
                    .join(","),
            );

            let mut eval = document::eval(&script);
            let Ok(val) = eval.recv::<serde_json::Value>().await else {
                tracing::error!("Failed to receive user identity from localStorage");
                return;
            };

            let user_id = val["id"].as_str().unwrap_or("").to_string();
            let name = val["name"].as_str().unwrap_or("Player").to_string();
            my_name.set(name.clone());
            my_user_id.set(user_id.clone());

            // ── Step 2: lobby WebSocket ───────────────────────────────────────
            match get_lobby_state(quizz_id, user_id, name, WebSocketOptions::new()).await {
                Ok(ws) => {
                    while let Ok(update) = ws.recv().await {
                        match update {
                            LobbyUpdate::Players(users) => players.set(users),
                            LobbyUpdate::GameStarted => {
                                navigator.push(Route::Play { quizz_id });
                                break;
                            }
                        }
                    }
                }
                Err(e) => tracing::error!("Lobby WebSocket error: {e:?}"),
            }
        });
    });

    rsx! {
        div {
            display: "flex",
            flex_direction: "column",
            align_items: "center",
            padding: "2rem",
            gap: "1.5rem",

            h2 { font_size: "1.8rem", "Lobby #{quizz_id}" }

            // ── Player list ──────────────────────────────────────────────────
            div {
                background: "rgba(255,255,255,0.06)",
                border_radius: "16px",
                padding: "1.5rem 2rem",
                min_width: "260px",

                h3 { margin_bottom: "1rem", "Players ({players.read().len()})" }

                if players.read().is_empty() {
                    p { color: "rgba(255,255,255,0.5)", "Waiting for others to join…" }
                }

                for player in players.read().iter() {
                    div {
                        key: "{player.id}",
                        display: "flex",
                        align_items: "center",
                        gap: "0.5rem",
                        padding: "0.4rem 0",

                        span { if player.is_admin { "👑" } else { "👤" } }
                        span {
                            font_weight: if player.name == *my_name.read() { "700" } else { "400" },
                            "{player.name}"
                        }
                        if player.name == *my_name.read() {
                            span { color: "rgba(255,255,255,0.4)", font_size: "0.8rem", "(you)" }
                        }
                        if player.is_admin {
                            span { color: "#facc15", font_size: "0.8rem", "admin" }
                        }
                    }
                }
            }

            // ── Start button — admin only ────────────────────────────────────
            // Clicking calls admin_start_game; the server broadcasts GameStarted
            // to all lobby WebSocket connections, which navigate everyone to /play.
            if is_me_admin() {
                button {
                    font_size: "1rem",
                    padding: "0.8rem 2rem",
                    background: "linear-gradient(135deg, #3b82f6, #8b5cf6)",
                    border: "none",
                    border_radius: "12px",
                    color: "white",
                    cursor: "pointer",
                    onclick: move |_| {
                        let uid = my_user_id();
                        async move {
                            admin_start_game(quizz_id, uid).await.ok();
                        }
                    },
                    "▶ Start Quiz"
                }
            }
        }
    }
}
