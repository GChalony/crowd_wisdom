use dioxus::{fullstack::WebSocketOptions, logger::tracing, prelude::*};
use qrcode::{Color, QrCode};

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

    // Full URL of this lobby page — populated client-side (window.location is browser-only).
    let mut lobby_url: Signal<String> = use_signal(String::new);
    let mut copied: Signal<bool> = use_signal(|| false);

    use_effect(move || {
        spawn(async move {
            let mut eval = document::eval("dioxus.send(window.location.origin);");
            if let Ok(origin) = eval.recv::<String>().await {
                lobby_url.set(format!("{origin}/lobby/{quizz_id}"));
            }
        });
    });

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
        div { class: "lobby-page",

            h2 { class: "lobby-title", "Lobby #{quizz_id}" }

            div { class: "lobby-card",
                h3 { "Players ({players.read().len()})" }

                if players.read().is_empty() {
                    p { color: "rgba(255,255,255,0.5)", "Waiting for others to join…" }
                }

                for player in players.read().iter() {
                    div { class: "player-row", key: "{player.id}",
                        span { if player.is_admin { "👑" } else { "👤" } }
                        span {
                            class: if player.name == *my_name.read() { "player-name-me" } else { "" },
                            "{player.name}"
                        }
                        if player.name == *my_name.read() {
                            span { class: "player-badge-you", "(you)" }
                        }
                        if player.is_admin {
                            span { class: "player-badge-admin", "admin" }
                        }
                    }
                }
            }

            // ── Share card (visible to everyone) ──────────────────────────────
            if !lobby_url.read().is_empty() {
                div { class: "share-card",
                    p { class: "share-title", "🔗 Invite players" }

                    // URL row with copy button
                    div { class: "share-url-row",
                        span { class: "share-url-text", "{lobby_url}" }
                        button {
                            class: "btn-copy",
                            onclick: move |_| {
                                let url = lobby_url();
                                async move {
                                    document::eval(&format!("navigator.clipboard.writeText(\"{url}\");"));
                                    copied.set(true);
                                }
                            },
                            if copied() { "✓ Copied!" } else { "Copy" }
                        }
                    }

                    // Inline SVG QR code — no external service, fully offline
                    QrCodeSvg { data: lobby_url() }
                }
            }

            // ── Start button — admin only ────────────────────────────────────
            // Clicking calls admin_start_game; the server broadcasts GameStarted
            // to all lobby WebSocket connections, which navigate everyone to /play.
            if is_me_admin() {
                button { class: "btn-start",
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

// ── QR code component ────────────────────────────────────────────────────────
// Renders the QR matrix as inline SVG <rect> elements.
// No external service, no dangerous_inner_html, works fully offline.
#[component]
fn QrCodeSvg(data: String) -> Element {
    let Ok(code) = QrCode::new(data.as_bytes()) else {
        return rsx! {};
    };

    let width = code.width();
    let cells: Vec<bool> = code
        .into_colors()
        .into_iter()
        .map(|c| c == Color::Dark)
        .collect();

    // Each module is 7 px; 14 px quiet-zone margin on every side.
    let cell: usize = 7;
    let margin: usize = 14;
    let total = width * cell + 2 * margin;

    // Collect dark-cell (col, row) pairs up-front so the iterator is
    // a plain Vec — trivial for the RSX map to consume.
    let dark: Vec<(usize, usize)> = cells
        .iter()
        .enumerate()
        .filter_map(|(i, &d)| d.then_some((i % width, i / width)))
        .collect();

    rsx! {
        svg {
            width: "{total}",
            height: "{total}",
            style: "background:white;border-radius:10px;display:block;",
            rect { x: "0", y: "0", width: "{total}", height: "{total}", fill: "white" }
            {dark.into_iter().map(|(cx, cy)| {
                let x = cx * cell + margin;
                let y = cy * cell + margin;
                rsx! {
                    rect {
                        key: "{cx}-{cy}",
                        x: "{x}",
                        y: "{y}",
                        width: "{cell}",
                        height: "{cell}",
                        fill: "#0f172a",
                    }
                }
            })}
        }
    }
}
