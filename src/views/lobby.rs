use dioxus::{fullstack::WebSocketOptions, logger::tracing, prelude::*};

use crate::backend::{get_lobby_state, User};

/// Adjectives and animals used to build deterministic fake names from a UUID.
/// 10 × 12 = 120 combinations — enough for a fun demo.
const ADJECTIVES: [&str; 10] = [
    "Swift", "Brave", "Clever", "Wise", "Bold", "Quick", "Bright", "Calm", "Daring", "Eager",
];
const ANIMALS: [&str; 12] = [
    "Fox", "Bear", "Eagle", "Wolf", "Lion", "Tiger", "Hawk", "Deer", "Crane", "Lynx", "Panda",
    "Otter",
];

#[component]
pub fn Lobby(quizz_id: u32) -> Element {
    // Current player list received from the server via WebSocket.
    let mut players: Signal<Vec<User>> = use_signal(Vec::new);

    // The local user's display name (set once we read localStorage).
    let mut my_name: Signal<String> = use_signal(String::new);

    // use_effect runs only on the client (after hydration), which is exactly
    // when we can access localStorage and open a WebSocket.
    use_effect(move || {
        spawn(async move {
            // -----------------------------------------------------------------
            // Step 1 – get or create the user's identity in localStorage.
            // -----------------------------------------------------------------
            // We derive the fake name deterministically from the UUID so the
            // user always gets the same name on the same device without needing
            // an account.
            let script = format!(
                r#"
                let id = localStorage.getItem('crowd_wisdom_user_id');
                if (!id) {{
                    id = crypto.randomUUID();
                    localStorage.setItem('crowd_wisdom_user_id', id);
                    // Clear any stale name so it regenerates below.
                    localStorage.removeItem('crowd_wisdom_user_name');
                }}
                let name = localStorage.getItem('crowd_wisdom_user_name');
                if (!name) {{
                    const adj  = [{adj}];
                    const ani  = [{ani}];
                    const h    = parseInt(id.replace(/-/g, '').slice(0, 8), 16);
                    name = adj[h % adj.length] + ani[Math.floor(h / adj.length) % ani.length];
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

            let name = val["name"].as_str().unwrap_or("Player").to_string();
            my_name.set(name.clone());

            // -----------------------------------------------------------------
            // Step 2 – open the lobby WebSocket and stream player updates.
            // -----------------------------------------------------------------
            match get_lobby_state(quizz_id, name, WebSocketOptions::new()).await {
                Ok(ws) => {
                    while let Ok(users) = ws.recv().await {
                        players.set(users);
                    }
                }
                Err(e) => tracing::error!("Lobby WebSocket error: {e:?}"),
            }
        });
    });

    let name_snapshot = my_name.read().clone();

    rsx! {
        div { display: "flex", flex_direction: "column", align_items: "center",
            padding: "2rem", gap: "1.5rem",

            h2 { font_size: "1.8rem", "Lobby #{quizz_id}" }

            // Player list
            div {
                background: "rgba(255,255,255,0.06)",
                border_radius: "16px",
                padding: "1.5rem 2rem",
                min_width: "260px",

                h3 { margin_bottom: "1rem",
                    "Players ({players.read().len()})"
                }

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

                        if player.name == name_snapshot {
                            span { "⭐" }
                            span { font_weight: "700", "{player.name}" }
                            span { color: "rgba(255,255,255,0.4)", font_size: "0.85rem", "(you)" }
                        } else {
                            span { "👤" }
                            span { "{player.name}" }
                        }
                    }
                }
            }

            button { font_size: "1rem", padding: "0.8rem 2rem",
                "Start"
            }
        }
    }
}
