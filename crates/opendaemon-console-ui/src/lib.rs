use leptos::prelude::*;
use wasm_bindgen::prelude::*;

mod app;
mod routes;
mod shell;
pub mod state;

#[wasm_bindgen(start)]
pub fn mount() {
    console_error_panic_hook::set_once();
    mount_to_body(app::App);
}
