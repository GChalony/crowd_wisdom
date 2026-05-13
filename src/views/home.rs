use crate::components::{Echo, Hero};
use dioxus::prelude::*;

/// The Home page component that will be rendered when the current route is `[Route::Home]`
#[component]
pub fn Home() -> Element {
    rsx! {
        SayHi {}
        Hero {}
        Echo {}
    }
}

#[component]
pub fn SayHi() -> Element {
    rsx! { "Hi" }
}