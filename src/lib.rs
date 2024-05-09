use application::Application;
use jandering_engine::core::{engine::EngineBuilder, window::WindowBuilder};

mod application;
mod camera_controller;
mod color_obj;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
async fn main() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Info).expect("Coultn init");

    let mut engine = EngineBuilder::default()
        .with_window(
            WindowBuilder::default()
                .with_cursor(true)
                .with_auto_resolution()
                .with_title("heyy")
                .with_cursor(true),
        )
        .build()
        .await;

    let application = Application::new(&mut engine).await;

    engine.run(application);
}
