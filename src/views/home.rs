use crate::Route;
use dioxus::{
    core::ElementId,
    html::{button::value, u::flex_direction},
    prelude::*,
};
use dioxus_free_icons::Icon;
use dioxus_free_icons::icons::fa_solid_icons::{FaPeopleGroup, FaPlus};

/// The Home page component that will be rendered when the current route is `[Route::Home]`
#[component]
pub fn Home() -> Element {
    rsx! {
        h1 { "Welcome" }
        p {
            "This website is here to illustrate the \"Crowd Wisdom\", or how 
            a group of people can be smarter than even the smartest among them."
        }
        p {
            "It's mostly inspired by Fouloscopie's book : "
            em { "Do we need a boss ?" }
        }

        p {
            "To discover that yourself, you can either create a new Quizz, or join one that already exists!"
        }
        div {
            display: "flex",
            justify_content: "center",
            gap: "4em",
            margin_top: "2em",

            button {

                Link {
                    display: "flex",
                    align_items: "space-around",
                    gap: "10px",
                    text_decoration: "none",
                    color: "inherit",
                    to: Route::CreateQuizz {},

                    Icon { icon: FaPlus }
                    "Create new quizz"
                }
            }
            JoinGame {}
        
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
        div { display: "flex", align_content: "center", gap: "1em",
            input {
                r#type: "number",
                width: "5em",
                placeholder: "Game ID",
                style: "width: 8em",
                value: "{game_id}",
                oninput: fill_game_id,
            }
            button {
                Link {
                    display: "flex",
                    align_items: "center",
                    text_decoration: "none",
                    color: "inherit",
                    gap: "10px",
                    to: Route::Play {
                        quizz_id: game_id.read().parse().unwrap_or(0),
                    },

                    Icon { icon: FaPeopleGroup }
                    "Join game"
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