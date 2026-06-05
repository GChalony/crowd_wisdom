use crate::Route;
use dioxus::{
    html::{button::value, u::flex_direction},
    prelude::*,
};
use dioxus_free_icons::icons::fa_solid_icons::{FaPeopleGroup, FaPlus};
use dioxus_free_icons::Icon;

#[component]
pub fn Home() -> Element {
    rsx! {
        div { class: "home-wrap",

            // ── Hero ─────────────────────────────────────────────────────────
            div { class: "hero",
                p { class: "hero-eyebrow", "🧠 A live social experiment" }
                h1 { class: "hero-title", "Crowd Wisdom" }
                p { class: "hero-tagline",
                    "Are 100 strangers smarter than one expert?"
                }
            }

            // ── The Galton story ─────────────────────────────────────────────
            div { class: "story-card",
                p { class: "story-year", "📅 Plymouth, England · 1906" }
                p { class: "story-text",
                    "Statistician "
                    strong { "Francis Galton" }
                    " asked 800 fairgoers to guess the weight of an ox.
                     He expected the crowd to be wildly wrong.
                     Instead, the average of all 800 guesses was "
                    strong { "1,207 lbs" }
                    " — just "
                    strong { "0.8% off" }
                    " the actual weight of 1,198 lbs.
                     Not a single individual came that close."
                }
                div { class: "story-highlight",
                    "✨ The crowd beat every individual expert"
                }
            }

            // ── Explanation ──────────────────────────────────────────────────
            p {
                style: "color:rgba(241,245,249,0.7);max-width:520px;font-size:1.05rem;",
                "This quiz lets you experience that effect live.
                 Everyone answers the same numerical questions — then we reveal
                 how close the crowd's average was to the truth."
            }

            // ── Book reference ───────────────────────────────────────────────
            p { class: "book-ref",
                "Inspired by Fouloscopie's book "
                em { "« Does the crowd need a boss? »" }
            }

            // ── Actions ──────────────────────────────────────────────────────
            div { class: "home-actions",

                button { class: "btn-create",
                    Link { to: Route::CreateQuizz {},
                        Icon { icon: FaPlus }
                        "Create a quiz"
                    }
                }

                div { class: "home-actions-divider", "or join one" }

                JoinGame {}
            }

            p { class: "home-footnote",
                "No account needed · Completely free · Works on any device"
            }
        }
    }
}

#[component]
pub fn JoinGame() -> Element {
    let mut game_id: Signal<String> = use_signal(|| "".to_string());
    let fill_game_id = move |text: Event<FormData>| {
        game_id.set(text.value());
    };

    rsx! {
        div { class: "join-row",
            input {
                r#type: "number",
                placeholder: "Game code",
                value: "{game_id}",
                oninput: fill_game_id,
            }
            button {
                Link {
                    display: "flex",
                    align_items: "center",
                    text_decoration: "none",
                    color: "inherit",
                    gap: "6px",
                    to: Route::Play {
                        quizz_id: game_id.read().parse().unwrap_or(0),
                    },
                    Icon { icon: FaPeopleGroup }
                    "Join"
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
